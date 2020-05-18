use eyre::{bail, eyre, Context};
use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::Command;

use crate::config;
use crate::ipc_command::IPCCommand;
use crate::ipc_response::IPCResponse;

type ClientResult<T> = color_eyre::Result<T>;
type EmptyResult = ClientResult<()>;

pub fn restart_process(process_name: Option<&str>) -> EmptyResult {
    let command = IPCCommand::restart_command(process_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

pub fn connect_to_process(process_name: Option<&str>) -> EmptyResult {
    let command = IPCCommand::connect_command(process_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

pub fn stop_app(app_name: Option<&str>) -> EmptyResult {
    let command = IPCCommand::stop_command(app_name.map(str::to_string), current_dir()?);
    send_command(&command)?;
    Ok(())
}

fn send_command(command: &IPCCommand) -> EmptyResult {
    let mut socket = UnixStream::connect(config::socket_path())
        .context("Failed to open socket. Is the server running?")?;

    serde_json::to_writer(&socket, &command)?;
    socket.write_all(b"\n")?;
    socket.flush()?;
    let response: IPCResponse =
        serde_json::from_reader(socket).context("Failed to parse response from server")?;

    match response {
        IPCResponse::ConnectionDetails {
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
        IPCResponse::Status(message) => println!("{}", message),
        IPCResponse::NotFound(message) => eprintln!("Server returned error: {}", message),
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
