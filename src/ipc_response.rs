use crate::config;
use crate::process::Process;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum IpcResponse {
    NotFound(String),
    ConnectionDetails {
        app_name: String,
        tmux_socket: String,
        tmux_session: String,
    },
    Status(String),
}

impl IpcResponse {
    pub async fn for_process(process: &color_eyre::Result<Process>) -> Self {
        match process {
            Ok(process) => {
                let app_name = process.app_name().await;
                IpcResponse::ConnectionDetails {
                    app_name,
                    tmux_socket: config::tmux_socket(),
                    tmux_session: process.tmux_session().await,
                }
            }

            Err(error) => IpcResponse::NotFound(error.to_string()),
        }
    }
}
