use std::env;
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{self, Command};
use std::str;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time;

use nix::sys::signal::{self, Signal};
use nix::sys::stat;
use nix::unistd::{self, Pid};

use failure::{bail, err_msg, format_err, ResultExt};
use shellexpand;
use tokio::fs::File;

use crate::config;
use crate::output::Output;

#[derive(Clone)]
pub struct Process {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    app_name: String,
    process_name: String,
    port: u16,
    command: String,
    directory: String,
    pid: Option<Pid>,
    restart_pending: bool,
}

impl Process {
    pub fn from_config(
        app_config: &config::App,
        process_name: String,
        command: String,
        port: u16,
    ) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            process_name,
            port,
            command: command.to_string(),
            directory: expand_path(&app_config.directory),
            pid: None,
            restart_pending: false,
        };

        Process {
            inner: Arc::new(Mutex::new(data)),
        }
    }

    fn inner(&self) -> MutexGuard<Inner> {
        self.inner.lock().unwrap()
    }

    pub fn app_name(&self) -> String {
        self.inner().app_name.clone()
    }

    fn base_tmux_command(&self) -> Command {
        let mut command = Command::new("tmux");
        command.args(&["-L", &config::tmux_socket()]);
        command.args(&["-f", "/dev/null"]);

        command
    }

    fn kill_tmux_session(&self) -> Result<process::Output, String> {
        self.base_tmux_command()
            .args(&["kill-session", "-t", &self.tmux_session()])
            .output()
            .map_err(|e| format!("Cleaning up old tmux session failed with error {}", e))
    }

    fn respawn_tmux_session(&self) -> Result<(), failure::Error> {
        let mut cmd = self.base_tmux_command();

        let result = cmd
            .args(&["respawn-window", "-t", &self.tmux_session()])
            .args(&self.shell_args())
            .status()
            .context("Error trying to run respawn command")?;

        if !result.success() {
            bail!("Non-zero return status from respawn-session");
        }

        let pids = self
            .base_tmux_command()
            .args(&["list-sessions", "-F", "#{session_name}|#{pane_pid}"])
            .output()?
            .stdout;

        let pid = BufRead::lines(&pids[..])
            .find_map(|line| match line {
                Err(_) => None,
                Ok(line) => {
                    let parts: Vec<&str> = line.splitn(2, '|').collect();

                    match parts[..] {
                        [session, pid] if session == self.tmux_session() => Some(String::from(pid)),
                        _ => None,
                    }
                }
            })
            .ok_or_else(|| err_msg("Failed to find PID for session"))?;

        self.set_pid(
            pid.parse()
                .map_err(|_| format_err!("\"{}\" is not a valid pid", pid))?,
        );

        self.watch_for_exit();

        Ok(())
    }

    pub fn start(&self) -> Result<(), String> {
        self.set_restart_pending(false);

        if self.respawn_tmux_session().is_ok() {
            // Bail out if respawning worked
            return Ok(());
        }

        // Clean up any existing tmux sessions with conflicting names
        self.kill_tmux_session()?;

        let child_pid = self
            .build_command()
            .output()
            .map_err(|_| "Failed to start app process")?
            .stdout;

        // Set pid using output from Tmux custom format
        let child_pid = str::from_utf8(&child_pid)
            .map_err(|e| format!("{}", e))?
            .trim();

        self.set_pid(
            child_pid
                .parse()
                .map_err(|_| format!("\"{}\" is not a valid pid", child_pid))?,
        );

        self.watch_for_exit();

        let fifo_path = self.setup_fifo();
        let catpipe = format!("cat >> {}", fifo_path.to_string_lossy());

        self.base_tmux_command()
            .args(&["pipe-pane", &catpipe])
            .status()
            .map_err(|_| "Failed to set up tmux output pipe")?;

        let fifo =
            fs::File::open(&fifo_path).map_err(|e| format!("Couldn't open FIFO, got {}", e))?;
        Output::for_stream(File::from_std(fifo), self.clone());
        Ok(())
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
            self.start().unwrap_or_else(|e| eprintln!("{}", e));
        }
    }

    pub fn process_died(&self) {
        self.set_stopped();

        if self.restart_pending() {
            self.start().unwrap_or_else(|e| eprintln!("{}", e));
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

    pub fn is_running(&self) -> bool {
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

    fn shell_args(&self) -> [String; 5] {
        let full_command = format!(
            "exec bash -c 'cd {directory}; export PORT={port}; exec {command}'",
            directory = self.directory(),
            command = self.command(),
            port = self.port()
        );

        eprintln!("Starting command {}", full_command);

        let shell = env::var("SHELL").unwrap_or("/bin/sh".into());

        [shell, "-l".into(), "-i".into(), "-c".into(), full_command]
    }

    fn build_command(&self) -> Command {
        let mut cmd = self.base_tmux_command();

        cmd.args(&["new-session", "-s", &self.tmux_session()])
            .args(&["-d", "-P", "-F", "#{pane_pid}"])
            .args(&self.shell_args())
            .args(&[";", "set", "remain-on-exit", "on"])
            .args(&[";", "set", "mouse", "on"])
            .args(&[";", "set", "status-right", "Press C-x to disconnect"])
            .args(&[";", "bind-key", "-n", "C-x", "detach-client"]);

        cmd
    }

    pub fn tmux_session(&self) -> String {
        self.name()
    }

    pub fn port(&self) -> u16 {
        self.inner().port
    }

    pub fn directory(&self) -> String {
        self.inner().directory.clone()
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

    fn watch_for_exit(&self) {
        let process = self.clone();

        let watcher = async move {
            let mut interval =
                tokio::time::interval(time::Duration::from_millis(WATCH_INTERVAL_MS));

            loop {
                interval.tick().await;

                if let Some(pid) = process.pid() {
                    if signal::kill(pid, None).is_err() {
                        println!("Process died");
                        process.process_died();

                        return;
                    }
                }
            }
        };

        tokio::spawn(watcher);
    }

    pub fn name(&self) -> String {
        let inner = self.inner();

        format!("{}/{}", inner.app_name, inner.process_name)
    }

    pub fn process_name(&self) -> String {
        self.inner().process_name.clone()
    }
}

const WATCH_INTERVAL_MS: u64 = 100;

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
