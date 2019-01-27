use serde_json;
use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::Command;

use crate::config;
use crate::ipc_command::IPCCommand;
use crate::ipc_response::IPCResponse;

pub fn restart_process(process_name: &str) {
    let command = IPCCommand::restart_command(process_name.to_string(), current_dir());
    send_command(&command);
}

pub fn connect_to_process(process_name: &str) {
    let command = IPCCommand::connect_command(process_name.to_string(), current_dir());
    send_command(&command);
}

fn send_command(command: &IPCCommand) {
    match UnixStream::connect(config::socket_path()) {
        Ok(mut socket) => {
            serde_json::to_writer(&socket, &command).unwrap();
            socket.write_all(b"\n").unwrap();
            socket.flush().unwrap();
            let response: IPCResponse = serde_json::from_reader(socket).unwrap();
            println!("Connecting tmux");
            Command::new("tmux")
                .args(&["-L", &response.tmux_socket])
                .args(&["attach-session", "-t", &response.tmux_session])
                .status()
                .expect("Tmux attach command failed");
        }
        Err(e) => {
            eprintln!("Couldn't connect to socket, got error \"{}\"", e);
            eprintln!("Is the server running?")
        }
    }
}

fn current_dir() -> String {
    let current_dir_path = env::current_dir().expect("Can't determine working directory");

    current_dir_path.to_str().unwrap().to_string()
}
