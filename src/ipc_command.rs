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
}
