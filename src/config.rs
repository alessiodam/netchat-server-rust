use std::error::Error;
use serde::Deserialize;
use crate::CONFIG_PATH;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub web: WebConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub online_mode: bool,
    pub api_key: String,
    pub protect_server: bool,
    pub server_password: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebConfig {
    pub enable: bool,
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn load_config() -> Result<Self, Box<dyn Error>> {
        let contents = std::fs::read_to_string(CONFIG_PATH)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        toml::from_str(&contents).map_err(|e| format!("Error deserializing config: {}", e).into())
    }
}
