use crate::config;
use crate::process::Process;

use failure::Error;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum IPCResponse {
    NotFound(String),
    ConnectionDetails {
        app_name: String,
        tmux_socket: String,
        tmux_session: String,
    },
}

impl IPCResponse {
    pub fn for_process(process: Result<&Process, Error>) -> Self {
        match process {
            Ok(process) => IPCResponse::ConnectionDetails {
                app_name: process.app_name(),
                tmux_socket: config::tmux_socket(),
                tmux_session: process.tmux_session(),
            },

            Err(error) => IPCResponse::NotFound(error.to_string()),
        }
    }
}