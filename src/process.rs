use std::env;
use std::io::BufRead;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;
use std::time::Duration;

use nix::sys::signal::{self, Signal};
use nix::sys::stat;
use nix::unistd::{self, Pid};

use async_stream::stream;
use eyre::{bail, eyre, Context};
use futures::Stream;
use tokio::{
    fs::{self, File},
    sync::{
        broadcast::{self, error::RecvError},
        RwLock, RwLockReadGuard, RwLockWriteGuard,
    },
    time::timeout,
};

use crate::config;
use crate::output::Output;
use crate::tmux;

#[derive(Clone, Debug)]
pub struct Process {
    inner: Arc<RwLock<Inner>>,
}

const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
const PID_STOP_TIMEOUT: Duration = Duration::from_secs(20);
const WATCH_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub(crate) enum RunState {
    /// Process is not running - initial state
    Stopped,
    /// Start request received, but PID not yet set
    Starting,
    /// Process is running with a known PID
    Running(Pid),
    /// Killing process, but haven't yet confirmed
    Terminating(Pid),
    /// Same as `Terminating`, but with the intention to restart
    Restarting(Pid),
}

#[derive(Debug)]
struct Inner {
    app_name: String,
    process_name: String,
    port: u16,
    command: String,
    directory: String,
    state: RunState,
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
            state: RunState::Stopped,
            output_channel,
        };

        Process {
            inner: Arc::new(RwLock::new(data)),
        }
    }

    async fn inner(&self) -> RwLockReadGuard<'_, Inner> {
        let result: color_eyre::Result<_> = timeout(LOCK_TIMEOUT, self.inner.read())
            .await
            .context("Timed out waiting on process read lock");

        result.unwrap()
    }

    async fn inner_mut(&self) -> RwLockWriteGuard<'_, Inner> {
        let result: color_eyre::Result<_> = timeout(LOCK_TIMEOUT, self.inner.write())
            .await
            .context("Timed out waiting on process write lock");

        result.unwrap()
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

    async fn respawn_tmux_session(&self) -> color_eyre::Result<()> {
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
            .ok_or_else(|| eyre!("Failed to find PID for session"))?;

        self.set_pid(
            pid.parse()
                .with_context(|| format!("\"{}\" is not a valid pid", pid))?,
        )
        .await;

        self.watch_for_exit();
        self.pipe_output()
            .await
            .unwrap_or_else(|e| println!("{}", e));

        Ok(())
    }

    pub async fn start(&self) -> Result<(), String> {
        if !matches!(self.run_state().await, RunState::Stopped) {
            return Err("Ignoring start request - process is not stopped".to_string());
        }

        self.set_run_state(RunState::Starting).await;

        if self.respawn_tmux_session().await.is_ok() {
            eprintln!("Respawned existing session");
            // Bail out if respawning worked
            return Ok(());
        }
        eprintln!("Starting new session");

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
        let fifo_path = self.setup_fifo().await.map_err(|e| e.to_string())?;

        tmux::pipe_pane(&fifo_path)
            .await
            .map_err(|_| "Failed to set up tmux output pipe")?;

        let fifo = File::open(&fifo_path)
            .await
            .map_err(|e| format!("Couldn't open FIFO, got {}", e))?;
        Output::for_stream(fifo, self.clone());

        Ok(())
    }

    async fn setup_fifo(&self) -> color_eyre::Result<PathBuf> {
        let pipe_name = {
            let inner = self.inner().await;
            format!("{}_{}.pipe", inner.app_name, inner.process_name)
        };

        let path = config::config_dir().join(pipe_name);
        fs::remove_file(&path).await.ok();

        unistd::mkfifo(&path, stat::Mode::S_IRWXU)
            .wrap_err("Failed to set up process output fifo")?;

        Ok(path)
    }

    pub async fn restart(&self) {
        eprintln!("restarting");

        match self.run_state().await {
            RunState::Restarting(_) | RunState::Starting => {
                eprintln!("Ignoring restart request, process is in invalid state");
            }
            RunState::Stopped => self.start().await.unwrap_or_else(|e| eprintln!("{}", e)),
            RunState::Running(pid) | RunState::Terminating(pid) => {
                self.set_run_state(RunState::Restarting(pid)).await;
                self.kill_after_timout(pid);

                signal_pid(pid, Signal::SIGINT).unwrap_or_else(|e| eprintln!("{}", e));
            }
        }
    }

    /// Force kill the process if it doesn't respond to our earlier SIGINT
    fn kill_after_timout(&self, pid: Pid) {
        let process = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(PID_STOP_TIMEOUT).await;

            match process.run_state().await {
                RunState::Restarting(newpid) | RunState::Terminating(newpid) if pid == newpid => {
                    signal_pid(pid, Signal::SIGKILL).unwrap_or_else(|e| eprintln!("{}", e));
                }
                _ => {
                    // If process is in new state or pid has changed don't try to kill
                }
            };
        });
    }

    pub async fn process_died(&self) {
        let previous_state = self.run_state().await;
        self.set_run_state(RunState::Stopped).await;

        if let RunState::Restarting(_) = previous_state {
            self.start().await.unwrap_or_else(|e| eprintln!("{}", e));
        }
    }

    pub async fn stop(&self) {
        match self.run_state().await {
            RunState::Starting | RunState::Stopped => {
                eprintln!("Ignoring stop request, process is in invalid state");
            }
            RunState::Running(pid) | RunState::Terminating(pid) | RunState::Restarting(pid) => {
                self.set_run_state(RunState::Terminating(pid)).await;
                signal_pid(pid, Signal::SIGINT).unwrap_or_else(|e| eprintln!("{}", e));
            }
        }
    }

    pub async fn is_running(&self) -> bool {
        if let RunState::Running(_) = self.run_state().await {
            true
        } else {
            false
        }
    }

    pub(crate) async fn run_state(&self) -> RunState {
        self.inner().await.state.clone()
    }

    async fn set_run_state(&self, new_state: RunState) {
        self.inner_mut().await.state = new_state;
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

    async fn pid(&self) -> Option<Pid> {
        match self.run_state().await {
            RunState::Running(pid) | RunState::Restarting(pid) | RunState::Terminating(pid) => {
                Some(pid)
            }
            _ => None,
        }
    }

    async fn set_pid(&self, pid: u32) {
        eprintln!("Setting pid for {} to {}", self.name().await, pid);
        let pid = Pid::from_raw(pid as i32);

        self.set_run_state(RunState::Running(pid)).await
    }

    fn watch_for_exit(&self) {
        let process = self.clone();

        let watcher = async move {
            let mut interval = tokio::time::interval(WATCH_INTERVAL);

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
        let mut channel = self.inner().await.output_channel.subscribe();

        let raw_stream = stream! {
            loop {
                match channel.recv().await {
                Ok(val) => yield val,
                Err(RecvError::Closed) => break,
                Err(e) => {
                    eprintln!("Got error {} in output listener", e);
                }
                }
            }
        };

        Box::pin(raw_stream)
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

fn signal_pid(pid: Pid, signal: Signal) -> Result<(), &'static str> {
    let group_pid = unistd::getpgid(Some(pid)).map_err(|_| "Couldn't find group for PID")?;

    eprintln!("Sending {:?} to process group {}", signal, group_pid);
    signal::kill(negate_pid(group_pid), signal).map_err(|_| "Failed to signal pid")
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
        use std::path;

        let home_dir = dirs::home_dir().unwrap();

        let result = expand_path("~/foo/bar");

        assert_eq!(path::PathBuf::from(result), home_dir.join("foo/bar"))
    }
}
