use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{RwLock, Mutex};
use std::sync::Arc;
use tracing::{info, warn, error, debug};

use crate::auth::{Config, verify_session};

pub async fn handle_connection(
    socket: Arc<Mutex<tokio::net::TcpStream>>,
    active_connections: Arc<RwLock<Vec<Arc<Mutex<tokio::net::TcpStream>>>>>,
    config: Config
) {
    let mut buf = vec![0; 1024];

    {
        let mut socket = socket.lock().await;
        match socket.read(&mut buf).await {
            Ok(n) => {
                if n == 0 {
                    return;
                }
                let message = String::from_utf8_lossy(&buf[..n]);
                debug!(message="Received message", msg=%message);

                if message.starts_with("AUTH:") {
                    let auth_parts: Vec<&str> = message.splitn(3, ':').collect();
                    if auth_parts.len() == 3 {
                        let username = auth_parts[1];
                        let session_token = auth_parts[2].trim();

                        if config.online_mode {
                            info!(target: "auth", "Authenticating user: {}", username);

                            match verify_session(&config, username, session_token).await {
                                Ok(is_valid_session) => {
                                    if !is_valid_session {
                                        let _ = socket.write_all(b"AUTH_FAILED\n").await;
                                    } else {
                                        let _ = socket.write_all(b"AUTH_SUCCESS\n").await;
                                    }
                                },
                                Err(e) => {
                                    let error_message = format!("AUTH_ERROR:{}\n", e);
                                    let _ = socket.write_all(error_message.as_bytes()).await;
                                },
                            }
                        } else {
                            info!(target: "auth", "Server not in online mode, marking user: {} as authenticated", username);
                            let _ = socket.write_all(b"AUTH_SUCCESS\n").await;
                        }
                    } else {
                        warn!(target: "auth", "Invalid AUTH message");
                        let _ = socket.write_all(b"AUTH_INVALID\n").await;
                    }
                } else {
                    info!(target: "server", "Received non-AUTH message or server not in online mode");
                    let _ = socket.write_all(b"INVALID_MESSAGE\n").await;
                }
            }
            Err(e) => error!(target: "server", "Failed to read from socket: {}", e),
        }

        info!(target: "server", "User marked as logged in.");
        socket.shutdown().await.unwrap_or_else(|_| {
            error!(target: "server", "Failed to shutdown socket");
        });
    }

    {
        let mut conns = active_connections.write().await;
        if let Some(pos) = conns.iter().position(|x| Arc::ptr_eq(x, &socket)) {
            conns.remove(pos);
        }
    }
}
