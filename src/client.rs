use serde_json;
use std::env;
use std::io::{self, Write};
use std::os::unix::net::UnixStream;

use config;
use ipc_command::IPCCommand;

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
            println!("Wrote command, waiting for response");
            io::copy(&mut socket, &mut io::stdout()).unwrap();
        }
        Err(e) => {
            eprintln!("Couldn't connect to socket, got err {}", e);
            eprintln!("Is the server running?")
        }
    }
}

fn current_dir() -> String {
    let current_dir_path = env::current_dir().expect("Can't determine working directory");

    current_dir_path.to_str().unwrap().to_string()
}
