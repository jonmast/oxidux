use std::{io::prelude::*, os::unix::net::UnixStream, time::Duration};

use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum IpcCommand {
    Restart {
        process_name: Option<String>,
        directory: String,
    },
    Connect {
        process_name: Option<String>,
        directory: String,
    },
    Stop {
        app_name: Option<String>,
        directory: String,
    },
    Ping,
}

impl IpcCommand {
    pub fn restart_command(process_name: Option<String>, directory: String) -> Self {
        Self::Restart {
            process_name,
            directory,
        }
    }

    pub fn connect_command(process_name: Option<String>, directory: String) -> Self {
        Self::Connect {
            process_name,
            directory,
        }
    }

    pub fn stop_command(app_name: Option<String>, directory: String) -> Self {
        Self::Stop {
            app_name,
            directory,
        }
    }

    pub fn heartbeat_command() -> Self {
        Self::Ping
    }
}

pub fn ping_server() -> color_eyre::Result<String> {
    let mut socket = UnixStream::connect(config::socket_path())?;

    // Ensure we don't hang indefinitely
    let timeout = Some(Duration::from_secs(1));
    socket.set_read_timeout(timeout)?;
    socket.set_write_timeout(timeout)?;

    let command = IpcCommand::heartbeat_command();
    serde_json::to_writer(&socket, &command)?;
    socket.write_all(b"\n")?;
    socket.flush()?;

    let mut response = String::new();
    socket.read_to_string(&mut response)?;

    Ok(response)
}
