use std::env;
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;
use std::time;

use nix::sys::signal::{self, Signal};
use nix::sys::stat;
use nix::unistd::{self, Pid};

use failure::{bail, err_msg, format_err, ResultExt};
use tokio::{
    fs::File,
    stream::{Stream, StreamExt},
    sync::{broadcast, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crate::config;
use crate::output::Output;
use crate::tmux;

#[derive(Clone, Debug)]
pub struct Process {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug)]
struct Inner {
    app_name: String,
    process_name: String,
    port: u16,
    command: String,
    directory: String,
    pid: Option<Pid>,
    restart_pending: bool,
    output_channel: broadcast::Sender<(Process, String)>,
}

impl Process {
    pub fn from_config(
        app_config: &config::App,
        process_name: String,
        command: String,
        port: u16,
    ) -> Self {
        let (output_channel, _output_receiver) = broadcast::channel(50);
        let data = Inner {
            app_name: app_config.name.clone(),
            process_name,
            port,
            command,
            directory: expand_path(&app_config.directory),
            pid: None,
            restart_pending: false,
            output_channel,
        };

        Process {
            inner: Arc::new(RwLock::new(data)),
        }
    }

    async fn inner(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().await
    }

    async fn inner_mut(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().await
    }

    pub async fn app_name(&self) -> String {
        self.inner().await.app_name.clone()
    }

    async fn kill_tmux_session(&self) -> Result<(), String> {
        tmux::kill_session(&self.tmux_session().await)
            .await
            .map(drop)
            .map_err(|e| format!("Cleaning up old tmux session failed with error {}", e))
    }

    async fn respawn_tmux_session(&self) -> Result<(), failure::Error> {
        let session_name = self.tmux_session().await;
        let shell_args = self.shell_args();

        let result = tmux::respawn_window(&session_name, &shell_args.await)
            .await
            .context("Error trying to run respawn command")?;

        if !result.success() {
            bail!("Non-zero return status from respawn-session");
        }

        let pids = tmux::list_sessions().await?.stdout;

        let pid = BufRead::lines(&pids[..])
            .find_map(|line| match line {
                Err(_) => None,
                Ok(line) => {
                    let parts: Vec<&str> = line.splitn(2, '|').collect();

                    match parts[..] {
                        [session, pid] if session == session_name => Some(String::from(pid)),
                        _ => None,
                    }
                }
            })
            .ok_or_else(|| err_msg("Failed to find PID for session"))?;

        self.set_pid(
            pid.parse()
                .map_err(|_| format_err!("\"{}\" is not a valid pid", pid))?,
        )
        .await;

        self.watch_for_exit();
        self.pipe_output()
            .await
            .unwrap_or_else(|e| println!("{}", e));

        Ok(())
    }

    pub async fn start(&self) -> Result<(), String> {
        self.set_restart_pending(false).await;

        if self.respawn_tmux_session().await.is_ok() {
            println!("Respawned existing session");
            // Bail out if respawning worked
            return Ok(());
        }
        println!("Starting new session");

        // Clean up any existing tmux sessions with conflicting names
        self.kill_tmux_session().await?;

        let args = self.shell_args().await;
        eprintln!("Starting command {}", args[4]);

        let child_pid = tmux::new_session(&self.tmux_session().await, &args)
            .await
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
        )
        .await;

        self.watch_for_exit();
        self.pipe_output().await?;

        Ok(())
    }

    /// Capture process output for our logging system
    async fn pipe_output(&self) -> Result<(), String> {
        let fifo_path = self.setup_fifo().await;

        tmux::pipe_pane(&fifo_path)
            .await
            .map_err(|_| "Failed to set up tmux output pipe")?;

        let fifo =
            fs::File::open(&fifo_path).map_err(|e| format!("Couldn't open FIFO, got {}", e))?;
        Output::for_stream(File::from_std(fifo), self.clone());

        Ok(())
    }

    async fn setup_fifo(&self) -> PathBuf {
        let path = config::config_dir().join(self.app_name().await + ".pipe");
        fs::remove_file(&path).ok();

        unistd::mkfifo(&path, stat::Mode::S_IRWXU).unwrap();

        path
    }

    pub async fn restart(&self) {
        eprintln!("restarting");
        self.stop().await;

        self.set_restart_pending(true).await;

        if !self.is_running().await {
            self.start().await.unwrap_or_else(|e| eprintln!("{}", e));
        }
    }

    pub async fn process_died(&self) {
        self.set_stopped().await;

        if self.restart_pending().await {
            self.start().await.unwrap_or_else(|e| eprintln!("{}", e));
        }
    }

    pub async fn stop(&self) {
        eprintln!("Stopping process {}", self.app_name().await);

        match self.send_signal(Signal::SIGINT).await {
            Ok(_) => {
                eprintln!("Successfully sent stop signal");
            }
            Err(msg) => eprintln!("Couldn't SIGINT, got err {}", msg),
        }
    }

    pub async fn is_running(&self) -> bool {
        self.pid().await.is_some()
    }

    async fn restart_pending(&self) -> bool {
        self.inner().await.restart_pending
    }

    async fn set_restart_pending(&self, state: bool) {
        let mut inner = self.inner_mut().await;
        inner.restart_pending = state
    }

    async fn send_signal(&self, signal: Signal) -> Result<(), &str> {
        let pid = self.pid().await.ok_or("Pid is empty")?;

        let group_pid = unistd::getpgid(Some(pid)).map_err(|_| "Couldn't find group for PID")?;

        eprintln!("Sending {:?} to process group {}", signal, group_pid);
        signal::kill(negate_pid(group_pid), signal).map_err(|_| "Failed to signal pid")
    }

    async fn shell_args(&self) -> [String; 5] {
        let full_command = format!(
            "exec bash -c 'cd {directory}; export PORT={port}; {command}'",
            directory = self.directory().await,
            command = self.command().await,
            port = self.port().await
        );

        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());

        [shell, "-l".into(), "-i".into(), "-c".into(), full_command]
    }

    pub async fn tmux_session(&self) -> String {
        self.name().await
    }

    pub async fn port(&self) -> u16 {
        self.inner().await.port
    }

    pub async fn directory(&self) -> String {
        self.inner().await.directory.clone()
    }

    async fn command(&self) -> String {
        self.inner().await.command.clone()
    }

    pub async fn pid(&self) -> Option<Pid> {
        self.inner().await.pid
    }

    async fn set_pid(&self, pid: u32) {
        eprintln!("Setting pid for {} to {}", self.app_name().await, pid);
        let pid = Pid::from_raw(pid as i32);

        let mut inner = self.inner_mut().await;
        inner.pid = Some(pid);
    }

    async fn set_stopped(&self) {
        let mut inner = self.inner_mut().await;
        inner.pid = None;
    }

    fn watch_for_exit(&self) {
        let process = self.clone();

        let watcher = async move {
            let mut interval =
                tokio::time::interval(time::Duration::from_millis(WATCH_INTERVAL_MS));

            loop {
                interval.tick().await;

                if let Some(pid) = process.pid().await {
                    if signal::kill(pid, None).is_err() {
                        eprintln!("Process died");
                        process.process_died().await;

                        return;
                    }
                }
            }
        };

        tokio::spawn(watcher);
    }

    pub async fn name(&self) -> String {
        let inner = self.inner().await;

        format!("{}/{}", inner.app_name, inner.process_name)
    }

    pub async fn process_name(&self) -> String {
        self.inner().await.process_name.clone()
    }

    pub async fn register_output_watcher(&self) -> impl Stream<Item = (Process, String)> {
        self.inner()
            .await
            .output_channel
            .subscribe()
            .filter_map(|result| match result {
                Ok(val) => Some(val),
                Err(e) => {
                    eprintln!("Got error {} in output listener", e);
                    None
                }
            })
    }

    /// Forwards stdout from process to any registered watchers
    pub fn output_line(&self, line: String) {
        let process = self.clone();
        tokio::spawn(async move {
            let output_channel = &process.inner().await.output_channel;

            output_channel.send((process.clone(), line))
        });
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
        use std::path;

        let home_dir = dirs::home_dir().unwrap();

        let result = expand_path("~/foo/bar");

        assert_eq!(path::PathBuf::from(result), home_dir.join("foo/bar"))
    }
}
