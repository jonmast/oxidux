use serde_json;
use std::os::unix::net::UnixStream;

use config;
use ipc_command::IPCCommand;

pub fn restart_process(process_name: &str) {
    match UnixStream::connect(config::socket_path()) {
        Ok(socket) => {
            eprintln!("Restarting process {}", process_name);
            let command = IPCCommand::restart_command(process_name.to_string());

            serde_json::to_writer(socket, &command).unwrap()
        }
        Err(e) => {
            eprintln!("Couldn't connect to socket, got err {}", e);
            eprintln!("Is the server running?")
        }
    }
}
