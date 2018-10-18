#[derive(Serialize, Deserialize, Debug)]
pub struct IPCCommand {
    pub command: String,
    pub args: Vec<String>,
}

impl IPCCommand {
    pub fn restart_command(process_name: String) -> Self {
        Self {
            command: "restart".to_string(),
            args: vec![process_name],
        }
    }
}
