use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::error::Error;
use serde::Deserialize;
use toml::de::from_str;
use tracing_subscriber::fmt;

#[derive(Debug, Deserialize, Clone)]
struct Config {
    online_mode: bool,
    host: String,
    port: u16,
    api_key: String,
}

impl Config {
    fn load_config() -> Self {
        let contents = std::fs::read_to_string("config.toml")
            .expect("Something went wrong reading the file");

        from_str(&contents).unwrap_or_else(|e| panic!("Error deserializing config: {}", e))
    }
}

fn init_tracing() {
    fmt::init();
}

async fn verify_session(config: &Config, username: &str, session_token: &str) -> Result<bool, Box<dyn Error>> {
    let url = "https://tinet.tkbstudios.com/api/v1/user/sessions/auth";
    let client = reqwest::Client::new();
    let response = client.post(url)
        .json(&serde_json::json!({
            "username": username,
            "session_token": session_token,
        }))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Api-Key", &config.api_key)
        .send()
        .await?
        .text()
        .await?;

    let result: serde_json::Value = serde_json::from_str(&response)?;

    if let Some(error) = result["error"].as_str() {
        if error == "User has not granted access to the app" {
            return Ok(false);
        }
    }

    Ok(true)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing();

    let config = Config::load_config();

    tracing::info!(target: "server", "Starting server with online mode: {} on {}:{}", config.online_mode, config.host, config.port);

    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        tracing::info!(target: "server", "New connection accepted");

        let config_clone = config.clone();
        tokio::spawn(async move {
            let mut buf = vec![0; 1024];

            match socket.read(&mut buf).await {
                Ok(n) => {
                    if n == 0 {
                        return;
                    }
                    let message = String::from_utf8_lossy(&buf[..n]);
                    tracing::debug!(message="Received message", msg=%message);

                    if message.starts_with("AUTH:") && config_clone.online_mode {
                        let auth_parts: Vec<&str> = message.splitn(3, ':').collect();
                        if auth_parts.len() == 3 {
                            let username = auth_parts[1];
                            let session_token = auth_parts[2];
                            tracing::info!(target: "auth", "Authenticated user: {} with session token: {}", username, session_token);

                            match verify_session(&config_clone, username, session_token).await {
                                Ok(is_valid_session) => {
                                    if !is_valid_session {
                                        tracing::warn!(target: "auth", "Session verification failed for user: {}", username);
                                    } else {
                                        tracing::info!(target: "auth", "Session verified for user: {}", username);
                                    }
                                },
                                Err(e) => tracing::error!(target: "auth", "Failed to verify session: {}", e),
                            }
                        } else {
                            tracing::warn!(target: "auth", "Invalid AUTH message");
                        }
                    } else {
                        tracing::info!(target: "server", "Received non-AUTH message or server not in online mode");
                    }
                }
                Err(e) => tracing::error!(target: "server", "Failed to read from socket: {}", e),
            }

            tracing::info!(target: "server", "User marked as logged in.");
            socket.shutdown().await.unwrap_or_else(|_| {
                tracing::error!(target: "server", "Failed to shutdown socket");
            });
        });
    }
}
