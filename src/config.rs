use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
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
    pub port: u16,
    pub command: String,
}

pub fn read_config(file_name: &str) -> Config {
    let mut contents = String::new();

    let mut file = File::open(file_name).expect("No config file found");
    file.read_to_string(&mut contents)
        .expect("Failed to read config file");

    toml::from_str(&contents).expect("Config file is invalid")
}

pub fn config_dir() -> PathBuf {
    let home_dir = env::home_dir().expect("Couldn't determine home directory");

    home_dir.join(".oxidux")
}
