use eyre::{bail, eyre, Context};
use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use crate::config;
use crate::ipc_command::IpcCommand;
use crate::ipc_response::IpcResponse;

type ClientResult<T> = color_eyre::Result<T>;
type EmptyResult = ClientResult<()>;

pub fn restart_process(process_name: Option<&str>) -> EmptyResult {
    let command = IpcCommand::restart_command(process_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

pub fn connect_to_process(process_name: Option<&str>) -> EmptyResult {
    let command = IpcCommand::connect_command(process_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

pub fn stop_app(app_name: Option<&str>) -> EmptyResult {
    let command = IpcCommand::stop_command(app_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

fn send_command(command: &IpcCommand) -> EmptyResult {
    let mut socket = UnixStream::connect(config::socket_path())
        .context("Failed to open socket. Is the server running?")?;

    // Ensure we don't hang indefinitely if server is unresponsive
    let timeout = Some(Duration::from_secs(5));
    socket.set_read_timeout(timeout)?;
    socket.set_write_timeout(timeout)?;

    serde_json::to_writer(&socket, &command)?;
    socket.write_all(b"\n")?;
    socket.flush()?;
    let response: IpcResponse =
        serde_json::from_reader(socket).context("Failed to parse response from server")?;

    match response {
        IpcResponse::ConnectionDetails {
            tmux_socket,
            tmux_session,
            ..
        } => {
            println!("Connecting tmux");
            let status = Command::new("tmux")
                .env_remove("TMUX")
                .args(&["-L", &tmux_socket])
                .args(&["attach-session", "-t", &tmux_session])
                .status()?;

            if !status.success() {
                bail!("Tmux reported Error");
            }
        }
        IpcResponse::Status(message) => println!("{}", message),
        IpcResponse::NotFound(message) => eprintln!("Server returned error: {}", message),
    }

    Ok(())
}

fn current_dir() -> ClientResult<String> {
    let current_dir_path = env::current_dir()?;

    Ok(current_dir_path
        .to_str()
        .ok_or_else(|| eyre!("Current directory is an invalid string"))?
        .to_string())
}
