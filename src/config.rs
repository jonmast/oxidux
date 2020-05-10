use failure::ResultExt;
use std::collections::HashMap;
use std::fs::{create_dir, File};
use std::io::prelude::*;
use std::path::PathBuf;
use tokio::fs::{read_dir as async_read_dir, File as AsyncFile};
use tokio::io::AsyncReadExt;
use tokio::stream::StreamExt;

use dirs;
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{
    de::{self, Unexpected},
    Deserialize, Deserializer,
};
use toml;

use crate::procfile;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
    pub general: ProxyConfig,
    #[deprecated]
    pub apps: Vec<App>,
}

impl Config {
    /// Retrieve app config for a hostname
    ///
    /// Searches through configs on disk to find the best match
    pub async fn find_app_by_host(&self, hostname: &str) -> Option<App> {
        // TODO: unify with process_manager logic and make more robust with subdomains, etc
        let parts = hostname.split('.');
        let app_name = parts.rev().nth(1).unwrap_or(hostname);

        for app in self.app_configs().await {
            if app.name == app_name {
                return Some(app);
            }
        }
        None
    }

    async fn app_configs(&self) -> Vec<App> {
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

async fn read_app_config(
    entry: tokio::io::Result<tokio::fs::DirEntry>,
) -> Result<App, failure::Error> {
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

fn default_dns_port() -> u16 {
    6153
}

fn default_domain() -> String {
    "test".to_string()
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ProxyConfig {
    pub proxy_port: u16,
    #[serde(default = "default_dns_port")]
    pub dns_port: u16,
    #[serde(default = "default_domain")]
    pub domain: String,
    #[serde(default = "config_dir")]
    pub config_dir: PathBuf,
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

    let dir = home_dir.join(".oxidux");

    if !dir.is_dir() {
        create_dir(&dir).expect("Error creating config directory");
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
    async fn find_app_by_host_test() {
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
            apps: vec![],
        };

        let found_app = config.find_app_by_host("testapp.test").await.unwrap();
        assert_eq!(
            CommandConfig::Command("/bin/true".to_string()),
            found_app.command_config
        )
    }
}
