use std::io::prelude::*;
use std::fs::File;
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
    pub port: u16,
}

pub fn read_config() -> Config {
    let mut contents = String::new();

    let mut file = File::open("apps.toml").expect("No config file found");
    file.read_to_string(&mut contents)
        .expect("Failed to read config file");

    toml::from_str(&contents).expect("Config file is invalid")
}
