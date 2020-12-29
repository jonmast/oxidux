use std::collections::HashMap;
use std::fs::{create_dir, File};
use std::io::prelude::*;
use std::path::PathBuf;

use eyre::Context;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{
    de::{self, Unexpected},
    Deserialize, Deserializer,
};
use tokio::fs::{read_dir as async_read_dir, File as AsyncFile};
use tokio::io::AsyncReadExt;
use tokio::stream::StreamExt;

use crate::procfile;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub general: ProxyConfig,
}

impl Config {
    /// Read app configs from disk and return them as a Vec
    pub(crate) async fn app_configs(&self) -> Vec<App> {
        let app_config_dir = self.general.config_dir.join("apps");
        let mut results = Vec::new();
        match async_read_dir(app_config_dir).await {
            Ok(mut entries) => {
                while let Some(entry) = entries.next().await {
                    match read_app_config(entry).await {
                        Ok(app) => results.push(app),
                        Err(e) => {
                            eprintln!("Skipping app config due to error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from app config directory: {}", e);
                return vec![];
            }
        };

        results
    }
}

async fn read_app_config(entry: tokio::io::Result<tokio::fs::DirEntry>) -> color_eyre::Result<App> {
    let path = entry.context("Error reading directory entry")?.path();
    let mut file = AsyncFile::open(&path)
        .await
        .context("Error reading app file")?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .await
        .context("Error reading config file")?;
    let app = toml::from_str(&contents).context("Invalid config file")?;
    Ok(app)
}

fn default_proxy_port() -> u16 {
    0
}

fn default_dns_port() -> u16 {
    6153
}

pub(crate) fn default_domain() -> String {
    "test".to_string()
}

fn default_idle_timeout_secs() -> u64 {
    3600
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProxyConfig {
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    #[serde(default = "default_dns_port")]
    pub dns_port: u16,
    #[serde(default = "default_domain")]
    pub domain: String,
    #[serde(default = "config_dir")]
    pub config_dir: PathBuf,
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            proxy_port: 0,
            dns_port: default_dns_port(),
            domain: default_domain(),
            config_dir: config_dir(),
            idle_timeout_secs: default_idle_timeout_secs(),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CommandConfig {
    Command(String),
    Commands(HashMap<String, String>),
    #[serde(deserialize_with = "true_to_unit")]
    Procfile,
}

impl Default for CommandConfig {
    fn default() -> Self {
        CommandConfig::Procfile
    }
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

#[derive(Deserialize, Debug, Clone, Eq, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub directory: String,
    pub port: Option<u16>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(flatten)]
    pub command_config: CommandConfig,
    #[serde(default)]
    pub aliases: Vec<String>,
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

    pub(crate) fn domains(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.name).chain(self.aliases.iter())
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

    let dir = home_dir.join(".oxidux");

    if !dir.is_dir() {
        let result = create_dir(&dir);
        if result.is_err() && !dir.is_dir() {
            result.expect("Error creating config directory");
        }
    }

    dir
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
    use crate::test_utils;
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

    #[tokio::test]
    async fn load_app_config() {
        let tmp = test_utils::temp_dir();
        let app_dir = tmp.join("apps");
        create_dir(&app_dir).unwrap();

        let mut app_file = File::create(&app_dir.join("testapp.toml")).unwrap();

        app_file
            .write_all(
                b"
name = 'testapp'
directory = '~'
command = '/bin/true'
",
            )
            .unwrap();

        let proxy_config = ProxyConfig {
            config_dir: tmp.to_path_buf(),
            ..ProxyConfig::default()
        };

        let config = Config {
            general: proxy_config,
        };

        let configs = config.app_configs().await;
        assert_eq!(1, configs.len());
        let found_app = &configs[0];
        assert_eq!(
            CommandConfig::Command("/bin/true".to_string()),
            found_app.command_config
        )
    }
}
