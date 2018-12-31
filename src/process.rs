use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::str;
use std::sync::{Arc, Mutex, MutexGuard};

use nix::sys::signal::{self, Signal};
use nix::sys::stat;
use nix::unistd::{self, Pid};

use futures::sync::mpsc;
use futures::Stream;
use hyper::Uri;
use shellexpand;
use tokio;
use url::Url;

use crate::config;
use crate::output::Output;

#[derive(Clone)]
pub struct Process {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    app_name: String,
    port: u16,
    command: String,
    directory: String,
    pid: Option<Pid>,
    restart_pending: bool,
    watchers: Vec<mpsc::Sender<Vec<u8>>>,
}

impl Process {
    pub fn from_config(app_config: &config::App) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            port: app_config.port,
            command: app_config.command.clone(),
            directory: expand_path(&app_config.directory),
            pid: None,
            restart_pending: false,
            watchers: vec![],
        };

        Process {
            inner: Arc::new(Mutex::new(data)),
        }
    }

    pub fn url(&self, request_path: &Uri) -> Uri {
        let base_url = Url::parse("http://localhost/").unwrap();

        let mut destination_url = base_url
            .join(request_path.path_and_query().unwrap().as_str())
            .expect("Invalid request URL");

        destination_url.set_port(Some(self.port())).unwrap();

        eprintln!("Starting request to backend {}", destination_url);

        destination_url.as_str().parse().unwrap()
    }

    fn inner(&self) -> MutexGuard<Inner> {
        self.inner.lock().unwrap()
    }

    pub fn app_name(&self) -> String {
        self.inner().app_name.clone()
    }

    pub fn start(&self) {
        self.set_restart_pending(false);

        let child_pid = self
            .build_command()
            .output()
            .expect("Failed to start app process")
            .stdout;

        // Set pid using output from Tmux custom format
        let child_pid = str::from_utf8(&child_pid).unwrap().trim();

        self.set_pid(child_pid.parse().unwrap());

        let fifo_path = self.setup_fifo();
        let catpipe = format!("cat >> {}", fifo_path.to_string_lossy());

        Command::new("tmux")
            .args(&["-L", "oxidux", "pipe-pane", &catpipe])
            .status()
            .expect("Pipe-pane failed :(");

        let fifo = fs::File::open(&fifo_path).unwrap();
        Output::for_stream(fifo, self.clone());
    }

    fn setup_fifo(&self) -> PathBuf {
        let path = config::config_dir().join(self.app_name() + ".pipe");
        fs::remove_file(&path).ok();

        unistd::mkfifo(&path, stat::Mode::S_IRWXU).unwrap();

        path
    }

    pub fn restart(&self) {
        eprintln!("restarting");
        self.stop();

        self.set_restart_pending(true);

        if !self.is_running() {
            self.start();
        }
    }

    pub fn process_died(&self) {
        self.set_stopped();

        if self.restart_pending() {
            self.start();
        }
    }

    pub fn stop(&self) {
        eprintln!("Stopping process {}", self.app_name());

        match self.send_signal(Signal::SIGINT) {
            Ok(_) => {
                eprintln!("Successfully sent stop signal");
            }
            Err(msg) => eprintln!("Couldn't SIGINT, got err {}", msg),
        }
    }

    fn is_running(&self) -> bool {
        self.pid().is_some()
    }

    fn restart_pending(&self) -> bool {
        self.inner().restart_pending
    }

    fn set_restart_pending(&self, state: bool) {
        let mut inner = self.inner();
        inner.restart_pending = state
    }

    fn send_signal(&self, signal: Signal) -> Result<(), &str> {
        let pid = self.pid().ok_or("Pid is empty")?;

        let group_pid = unistd::getpgid(Some(pid)).map_err(|_| "Couldn't find group for PID")?;

        eprintln!("Sending {:?} to process group {}", signal, group_pid);
        signal::kill(negate_pid(group_pid), signal).map_err(|_| "Failed to signal pid")
    }

    fn build_command(&self) -> Command {
        let full_command = format!(
            "cd {directory}; {command}",
            directory = self.directory(),
            command = self.command(),
        );
        eprintln!("Starting command {}", full_command);

        let shell = "/bin/sh";

        let mut cmd = Command::new("tmux");

        cmd.env("PORT", self.port().to_string())
            .args(&[
                "-L",
                "oxidux",
                "new-session",
                "-d",
                "-P",
                "-F",
                "#{pane_pid}",
            ])
            .arg(shell)
            .arg("-c")
            .arg(full_command);

        cmd
    }

    pub fn port(&self) -> u16 {
        self.inner().port
    }

    pub fn directory(&self) -> String {
        self.inner().directory.clone()
    }

    pub fn add_watcher(&self) -> impl Stream<Item = Vec<u8>, Error = ()> {
        let (tx, rx) = mpsc::channel(5);

        self.inner().watchers.push(tx);

        rx
    }

    fn command(&self) -> String {
        self.inner().command.clone()
    }

    fn pid(&self) -> Option<Pid> {
        self.inner().pid
    }

    fn set_pid(&self, pid: u32) {
        eprintln!("Setting pid for {} to {}", self.app_name(), pid);
        let mut inner = self.inner();
        let pid = Pid::from_raw(pid as i32);
        inner.pid = Some(pid);
    }

    fn set_stopped(&self) {
        let mut inner = self.inner();
        inner.pid = None;
    }
}

fn negate_pid(pid: Pid) -> Pid {
    let pid_id: i32 = pid.into();

    Pid::from_raw(-pid_id)
}

fn expand_path(input_path: &str) -> String {
    match shellexpand::full(input_path) {
        Ok(expanded_path) => expanded_path.to_string(),
        Err(_) => input_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_negation() {
        let pid = Pid::from_raw(1);

        let negated_pid = negate_pid(pid);

        assert_eq!(negated_pid, Pid::from_raw(-1));
    }

    #[test]
    fn expand_path_replaces_tilde() {
        use dirs;
        use std::path;

        let home_dir = dirs::home_dir().unwrap();

        let result = expand_path("~/foo/bar");

        assert_eq!(path::PathBuf::from(result), home_dir.join("foo/bar"))
    }
}
