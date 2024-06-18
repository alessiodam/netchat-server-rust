use std::error::Error;
use serde::Deserialize;
use tracing::{error, info};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub online_mode: bool,
    pub host: String,
    pub port: u16,
    pub api_key: String,
}

impl Config {
    pub fn load_config() -> Self {
        let contents = std::fs::read_to_string("config.toml")
            .expect("Something went wrong reading the file");

        toml::from_str(&contents).unwrap_or_else(|e| panic!("Error deserializing config: {}", e))
    }
}

pub async fn verify_session(config: &Config, username: &str, session_token: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let trimmed_session_token = session_token.trim();
    info!(target: "auth", "Verifying session token: {}", trimmed_session_token);
    let request_json = serde_json::json!({
        "username": username,
        "session_token": trimmed_session_token,
    });

    let url = "https://tinet.tkbstudios.com/api/v1/user/sessions/validity-check";
    let client = reqwest::Client::new();
    let response = client.post(url)
        .json(&request_json)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Api-Key", &config.api_key)
        .send()
        .await?
        .text()
        .await?;

    let result: serde_json::Value = serde_json::from_str(&response)?;
    println!("Response: {:?}", result);

    if let Some(error) = result["error"].as_str() {
        error!(target: "auth", "Error verifying session: {}", error);
        return Err(format!("AUTH_ERROR:{}", error).into());
    }

    if result["valid"].as_bool().unwrap_or(false) {
        info!(target: "auth", "Session verified successfully for user: {}", username);
        return Ok(true);
    }

    Ok(false)
}
