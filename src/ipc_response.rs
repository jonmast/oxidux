use crate::config;
use crate::process::Process;

#[derive(Serialize, Deserialize, Debug)]
pub struct IPCResponse {
    pub app_name: String,
    pub tmux_socket: String,
    pub tmux_session: String,
}

impl IPCResponse {
    pub fn for_process(process: &Process) -> Self {
        Self {
            app_name: process.app_name(),
            tmux_socket: config::tmux_socket(),
            tmux_session: process.tmux_session(),
        }
    }
}
