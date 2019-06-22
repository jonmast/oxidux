use std::{io::prelude::*, os::unix::net::UnixStream};

use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Serialize, Deserialize, Debug)]
pub struct IPCCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl IPCCommand {
    pub fn restart_command(process_name: String, directory: String) -> Self {
        Self {
            command: "restart".to_string(),
            args: vec![process_name, directory],
        }
    }

    pub fn connect_command(process_name: String, directory: String) -> Self {
        Self {
            command: "connect".to_string(),
            args: vec![process_name, directory],
        }
    }

    pub fn heartbeat_command() -> Self {
        Self {
            command: "ping".to_string(),
            args: Vec::new(),
        }
    }
}

pub fn ping_server() -> Result<String, failure::Error> {
    let mut socket = UnixStream::connect(config::socket_path())?;
    let command = IPCCommand::heartbeat_command();
    serde_json::to_writer(&socket, &command)?;
    socket.write_all(b"\n")?;
    socket.flush()?;

    let mut response = String::new();
    socket.read_to_string(&mut response)?;

    Ok(response)
}
