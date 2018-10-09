use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};

use nix::sys::signal::{self, Signal};
use nix::unistd::{self, Pid};

use futures::Future;
use hyper::Uri;
use tokio;
use tokio_pty_process::{self, CommandExt};
use url::Url;

use config;
use output::Output;

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
}

impl Process {
    pub fn from_config(app_config: &config::App) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            port: app_config.port,
            command: app_config.command.clone(),
            directory: app_config.directory.clone(),
            pid: None,
            restart_pending: false,
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

        println!("Starting request to backend {}", destination_url);

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
        let pty_master =
            tokio_pty_process::AsyncPtyMaster::open().expect("Failed to open PTY master");

        let child_process = self
            .build_command()
            .spawn_pty_async(&pty_master)
            .expect("Failed to start app process");

        let app_name = self.app_name();
        let output = Output::new(app_name.clone(), pty_master);
        output.setup_writer();

        self.set_pid(child_process.id());

        let listener = self.clone();

        let child_future = child_process
            .map(move |status| listener.process_died(status))
            .map_err(|e| panic!("failed to wait for exit: {}", e));

        tokio::spawn(child_future);
    }

    pub fn restart(&self) {
        println!("restarting");
        self.stop();

        self.set_restart_pending(true);

        if !self.is_running() {
            self.start();
        }
    }

    fn process_died(&self, status: ExitStatus) {
        println!("Process {} exited with {}", self.app_name(), status);

        self.set_stopped();

        if self.restart_pending() {
            self.start();
        }
    }

    pub fn stop(&self) {
        println!("Stopping process {}", self.app_name());

        match self.send_signal(Signal::SIGINT) {
            Ok(_) => {
                println!("Successfully sent stop signal");
            }
            Err(msg) => println!("Couldn't SIGINT, got err {}", msg),
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

        println!("Sending {:?} to process group {}", signal, group_pid);
        signal::kill(negate_pid(group_pid), signal).map_err(|_| "Failed to signal pid")
    }

    fn build_command(&self) -> Command {
        let full_command = format!(
            "cd {directory}; {command}",
            directory = self.directory(),
            command = self.command(),
        );
        println!("Starting command {}", full_command);

        let shell = "/bin/sh";

        let mut cmd = Command::new(shell);

        cmd.env("PORT", self.port().to_string())
            .arg("-c")
            .arg(full_command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd
    }

    fn port(&self) -> u16 {
        self.inner().port
    }

    fn directory(&self) -> String {
        self.inner().directory.clone()
    }

    fn command(&self) -> String {
        self.inner().command.clone()
    }

    fn pid(&self) -> Option<Pid> {
        self.inner().pid
    }

    fn set_pid(&self, pid: u32) {
        println!("Setting pid for {} to {}", self.app_name(), pid);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        let pid = Pid::from_raw(1);

        let negated_pid = negate_pid(pid);

        assert_eq!(negated_pid, Pid::from_raw(-1));
    }
}
