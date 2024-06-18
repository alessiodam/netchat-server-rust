use std::error::Error;
use tracing::{error, info};
use crate::config::Config;

pub async fn verify_session(config: &Config, username: &str, session_token: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let trimmed_session_token = session_token.trim();
    info!(target: "auth", "Verifying session token for user: {}", username);
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
