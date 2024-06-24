use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use std::sync::Arc;
use tracing::{info, warn, error};
use std::collections::HashMap;
use chrono::Utc;
use crate::auth::verify_session;
use crate::config::Config;
use crate::validators;
use crate::commands::{Command};
use crate::db::{add_message_to_db, add_or_update_user, get_messages, set_user_status, update_user_time_online};
use crate::state::{get_active_connections, get_active_users};
use crate::textutils::format_outgoing_message;

pub async fn handle_connection(
    socket: Arc<Mutex<tokio::net::TcpStream>>,
    config: Config,
    commands: HashMap<&str, Box<dyn Command>>,
) {
    let mut buf = vec![0; 4 * 1024];
    let mut authenticated = false;
    let mut username = String::new();
    let mut server_password_correct = if config.server.protect_server { false } else { true };
    let start_time = Utc::now().timestamp();

    loop {
        let mut socket_guard = socket.lock().await;
        let read_result = time::timeout(Duration::from_millis(100), socket_guard.read(&mut buf)).await;

        match read_result {
            Ok(Ok(n)) => {
                if n == 0 {
                    break;
                }
                let message = String::from_utf8_lossy(&buf[..n]).to_string();

                if message.trim() == "DISCONNECT" {
                    socket_guard.write_all(b"DISCONNECTED\n").await.unwrap();
                    socket_guard.flush().await.unwrap();
                    break;
                }
                if server_password_correct {
                    if message.starts_with("AUTH:") {
                        if authenticated {
                            socket_guard.write_all(b"ALREADY_AUTHENTICATED\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                        } else {
                            let auth_parts: Vec<&str> = message.splitn(3, ':').collect();
                            if auth_parts.len() == 3 {
                                username = auth_parts[1].to_string();
                                let session_token = auth_parts[2].trim();

                                if !validators::validate_username(&username) {
                                    socket_guard.write_all(b"INVALID_USERNAME\n").await.unwrap();
                                    socket_guard.flush().await.unwrap();
                                    continue;
                                }
                                if !validators::validate_session_token(session_token) {
                                    socket_guard.write_all(b"INVALID_SESSION_TOKEN\n").await.unwrap();
                                    socket_guard.flush().await.unwrap();
                                    continue;
                                }

                                if config.server.online_mode {
                                    info!(target: "auth", "Authenticating user: {}", username);

                                    match verify_session(&config, &username, session_token).await {
                                        Ok(is_valid_session) => {
                                            if !is_valid_session {
                                                socket_guard.write_all(b"AUTH_FAILED\n").await.unwrap();
                                                socket_guard.flush().await.unwrap();
                                            } else {
                                                authenticated = true;
                                                add_or_update_user(&username);
                                                {
                                                    let active_users = get_active_users();
                                                    let mut users = active_users.write().await;
                                                    users.insert(username.clone(), Arc::clone(&socket));
                                                }
                                                socket_guard.write_all(b"AUTH_SUCCESS\n").await.unwrap();
                                                socket_guard.flush().await.unwrap();
                                            }
                                        },
                                        Err(e) => {
                                            let error_message = format!("AUTH_ERROR:{}\n", e);
                                            socket_guard.write_all(error_message.as_bytes()).await.unwrap();
                                            socket_guard.flush().await.unwrap();
                                        },
                                    }
                                } else {
                                    info!(target: "auth", "Server not in online mode, marking user: {} as authenticated", username);
                                    authenticated = true;
                                    add_or_update_user(&username);
                                    {
                                        let active_users = get_active_users();
                                        let mut users = active_users.write().await;
                                        users.insert(username.clone(), Arc::clone(&socket));
                                    }
                                    socket_guard.write_all(b"AUTH_SUCCESS\n").await.unwrap();
                                    socket_guard.flush().await.unwrap();
                                }
                            } else {
                                warn!(target: "auth", "Invalid AUTH message");
                                socket_guard.write_all(b"AUTH_INVALID\n").await.unwrap();
                                socket_guard.flush().await.unwrap();
                            }
                        }
                    } else if authenticated {
                        if message.len() > 256 {
                            socket_guard.write_all(b"MESSAGE_TOO_LONG\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                            continue;
                        }

                        if message.is_empty() {
                            socket_guard.write_all(b"EMPTY_MESSAGE\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                            continue;
                        }

                        if message.starts_with("GET_MESSAGES:") {
                            let recipient = message.trim_start_matches("GET_MESSAGES:").trim();
                            let messages = get_messages(recipient, 100).unwrap();
                            info!(target: "tcpserver", "Sending messages: {:?}", messages);
                            for message in messages {
                                info!(target: "tcpserver", "Sending message: {}", message);
                                socket_guard.write_all(message.as_bytes()).await.unwrap();
                                socket_guard.flush().await.unwrap();
                            }
                            continue
                        }

                        if let Some((recipient, command_message)) = message.split_once(':') {
                            if command_message.starts_with('!') {
                                let command_name = command_message.split_whitespace().next().unwrap();
                                let args: Vec<&str> = command_message.split_whitespace().skip(1).collect();
                                if let Some(command) = commands.get(command_name) {
                                    let response = command.execute(&args).await;
                                    socket_guard.write_all(&response).await.unwrap();
                                    socket_guard.flush().await.unwrap();
                                    continue;
                                }
                            }

                            let timestamp = Utc::now().timestamp();
                            let full_message = format_outgoing_message(&username, recipient, &command_message, timestamp);
                            if recipient == "global" {
                                broadcast_message(&full_message).await;
                            } else {
                                send_direct_message(recipient, &full_message).await;
                            }
                            add_message_to_db(timestamp, &username, recipient, &command_message).unwrap();
                        } else {
                            socket_guard.write_all(b"INVALID_MESSAGE_FORMAT\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                        }
                    } else {
                        socket_guard.write_all(b"NOT_AUTHENTICATED\n").await.unwrap();
                        socket_guard.flush().await.unwrap();
                    }
                } else if !server_password_correct && config.server.protect_server {
                    if message.starts_with("SERVER_PASS:") {
                        let server_password = message.trim_start_matches("SERVER_PASS:").trim();
                        if server_password == config.server.server_password {
                            server_password_correct = true;
                            socket_guard.write_all(b"SERVER_PASS_CORRECT\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                        } else {
                            socket_guard.write_all(b"SERVER_PASS_INCORRECT\n").await.unwrap();
                            socket_guard.flush().await.unwrap();
                        }
                    } else {
                        socket_guard.write_all(b"SERVER_PASS_REQUIRED\n").await.unwrap();
                        socket_guard.flush().await.unwrap();
                    }
                }
            }
            Ok(Err(e)) => {
                error!(target: "server", "Failed to read from socket: {}", e);
                break;
            }
            Err(_) => {
                continue;
            }
        }
    }

    info!(target: "server", "Closing connection.");
    socket.lock().await.shutdown().await.unwrap_or_else(|_| {
        error!(target: "server", "Failed to shutdown socket");
    });

    {
        let active_connections = get_active_connections();
        let mut conns = active_connections.write().await;
        if let Some(pos) = conns.iter().position(|x| Arc::ptr_eq(x, &socket)) {
            conns.remove(pos);
        }
    }
    {
        let active_users = get_active_users();
        let mut users = active_users.write().await;
        users.remove(&username);
    }

    let _ = set_user_status(&username, "offline");
    let _ = update_user_time_online(&username, Utc::now().timestamp() - start_time);
}

async fn broadcast_message(message: &str) {
    info!(target: "server", "Broadcasting message: {}", message);

    let connections = get_active_connections();
    let connections = connections.read().await;
    let active_users = get_active_users();
    let active_users = active_users.read().await;

    for client in connections.iter() {
        if active_users.values().any(|user| Arc::ptr_eq(user, client)) {
            let client = client.clone();
            let message = message.to_string();
            info!(target: "server", "Sending to client: {:?}", client);
            tokio::spawn(async move {
                let mut client = client.lock().await;
                if let Err(e) = client.write_all(message.as_bytes()).await {
                    error!(target: "server", "Failed to send message: {}", e);
                } else {
                    let _ = client.flush().await;
                    info!(target: "server", "Broadcasted message: {}", message);
                }
            });
        }
    }
}

async fn send_direct_message(target: &str, message: &str) {
    let active_users = get_active_users();
    let active_users = active_users.read().await;
    if let Some(client) = active_users.get(target) {
        let client = client.clone();
        let message = message.to_string();
        info!(target: "server", "Sending direct message: {}", message);
        tokio::spawn(async move {
            let mut client = client.lock().await;
            if let Err(e) = client.write_all(message.as_bytes()).await {
                error!(target: "server", "Failed to send direct message: {}", e);
            } else {
                let _ = client.flush().await;
                info!(target: "server", "Sent direct message: {}", message);
            }
        });
    }
}
