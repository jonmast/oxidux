use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use dirs;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{
    de::{self, Unexpected},
    Deserialize, Deserializer,
};
use toml;

use crate::procfile;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub general: ProxyConfig,
    pub apps: Vec<App>,
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    pub proxy_port: u16,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum CommandConfig {
    Command(String),
    Commands(HashMap<String, String>),
    #[serde(deserialize_with = "true_to_unit")]
    Procfile,
}

impl CommandConfig {
    pub fn commands(&self, directory: String) -> HashMap<String, String> {
        match self {
            CommandConfig::Command(command) => [("app".to_string(), command.clone())]
                .iter()
                .cloned()
                .collect(),
            CommandConfig::Commands(map) => map.clone(),
            CommandConfig::Procfile => procfile::parse_procfile_in_dir(&directory),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub directory: String,
    pub port: Option<u16>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(flatten)]
    pub command_config: CommandConfig,
}

impl App {
    pub fn parsed_headers(&self) -> HeaderMap {
        self.headers
            .iter()
            .map(|(key, value)| {
                let header_name = HeaderName::from_bytes(key.as_bytes()).unwrap();
                let header_value = HeaderValue::from_bytes(value.as_bytes()).unwrap();

                (header_name, header_value)
            })
            .collect()
    }

    pub fn full_path(&self) -> String {
        match shellexpand::full(&self.directory) {
            Ok(expanded_path) => expanded_path.to_string(),
            Err(_) => self.directory.clone(),
        }
    }

    pub fn commands(&self) -> HashMap<String, String> {
        self.command_config.commands(self.full_path())
    }
}

pub fn read_config(file_name: &str) -> Config {
    let mut contents = String::new();

    let mut file = File::open(file_name).expect("No config file found");
    file.read_to_string(&mut contents)
        .expect("Failed to read config file");

    toml::from_str(&contents).expect("Config file is invalid")
}

pub fn config_dir() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Couldn't determine home directory");

    home_dir.join(".oxidux")
}

pub fn socket_path() -> PathBuf {
    config_dir().join("oxidux.sock")
}

// This needs to be dynamic to support multiple servers (as does the socket above)
pub fn tmux_socket() -> String {
    "oxidux".to_string()
}

fn true_to_unit<'a, D>(deserializer: D) -> Result<(), D::Error>
where
    D: Deserializer<'a>,
{
    if bool::deserialize(deserializer)? {
        Ok(())
    } else {
        Err(de::Error::invalid_value(Unexpected::Bool(false), &"true"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::header::HOST;

    #[test]
    fn test_header_deserialization() {
        let data = "
            directory = '/home/jon'
            name = 'bar'
            command = 'echo hello'
            headers = {host = 'test', foo='bar'}
        ";

        let app: App = toml::from_str(data).unwrap();

        assert!(app.parsed_headers().contains_key(HOST));
    }
}
