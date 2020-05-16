use crate::config;
use std::path::Path;
use tokio::process::Command;

type CmdResult<T> = Result<T, std::io::Error>;
type OutputResult = CmdResult<std::process::Output>;
type StatusResult = CmdResult<std::process::ExitStatus>;

pub(crate) async fn respawn_window(session_name: &str, shell_args: &[String]) -> StatusResult {
    base_command()
        .args(&["respawn-window", "-t", &session_name])
        .args(shell_args)
        .status()
        .await
}

pub(crate) async fn kill_session(session_name: &str) -> OutputResult {
    base_command()
        .args(&["kill-session", "-t", session_name])
        .output()
        .await
}

pub(crate) async fn list_sessions() -> OutputResult {
    base_command()
        .args(&["list-sessions", "-F", "#{session_name}|#{pane_pid}"])
        .output()
        .await
}

pub(crate) async fn pipe_pane(fifo_path: &Path) -> StatusResult {
    let catpipe = format!("cat >> {}", fifo_path.to_string_lossy());

    base_command().args(&["pipe-pane", &catpipe]).status().await
}

pub(crate) async fn new_session(session_name: &str, shell_args: &[String]) -> OutputResult {
    base_command()
        .args(&["new-session", "-s", session_name])
        .args(&["-d", "-P", "-F", "#{pane_pid}"])
        .args(shell_args)
        .args(&[";", "set", "remain-on-exit", "on"])
        .args(&[";", "set", "mouse", "on"])
        .args(&[";", "set", "status-right", "Press C-x to disconnect"])
        .args(&[";", "bind-key", "-n", "C-x", "detach-client"])
        .output()
        .await
}

pub(crate) async fn kill_server() -> StatusResult {
    base_command().arg("kill-server").status().await
}

fn base_command() -> Command {
    let mut command = Command::new("tmux");
    command.args(&["-L", &config::tmux_socket()]);
    command.args(&["-f", "/dev/null"]);

    command
}
