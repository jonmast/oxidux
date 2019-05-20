use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use dirs;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use toml;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub general: ProxyConfig,
    pub apps: Vec<App>,
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    pub proxy_port: u16,
}

#[derive(Deserialize, Debug)]
pub struct App {
    pub name: String,
    pub directory: String,
    pub port: Option<u16>,
    pub command: String,
    pub headers: Option<HashMap<String, String>>,
}

impl App {
    pub fn parsed_headers(&self) -> HeaderMap {
        match &self.headers {
            Some(headers) => headers
                .iter()
                .map(|(key, value)| {
                    let header_name = HeaderName::from_bytes(key.as_bytes()).unwrap();
                    let header_value = HeaderValue::from_bytes(value.as_bytes()).unwrap();

                    (header_name, header_value)
                })
                .collect(),
            None => HeaderMap::default(),
        }
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
