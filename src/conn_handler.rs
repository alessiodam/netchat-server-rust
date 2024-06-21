use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{self, Duration};
use std::sync::Arc;
use tracing::{info, warn, error};
use std::collections::HashMap;
use chrono::Utc;
use rusqlite::Connection;
use crate::auth::verify_session;
use crate::config::Config;
use crate::validators;
use crate::commands::{Command};
use crate::state::get_active_users;

pub async fn handle_connection(
    socket: Arc<Mutex<tokio::net::TcpStream>>,
    active_connections: Arc<RwLock<Vec<Arc<Mutex<tokio::net::TcpStream>>>>>,
    config: Config,
    db_conn: Arc<Mutex<Connection>>,
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

                                if !validators::validate_username(&username).await {
                                    socket_guard.write_all(b"INVALID_USERNAME\n").await.unwrap();
                                    socket_guard.flush().await.unwrap();
                                    continue;
                                }
                                if !validators::validate_session_token(session_token).await {
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
                                                add_or_update_user(&db_conn, &username).await;
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
                                    add_or_update_user(&db_conn, &username).await;
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
                            let full_message = format!("{}:{}:{}:{}", timestamp, username, recipient, command_message);
                            if recipient == "global" {
                                broadcast_message(&active_connections, &full_message).await;
                            } else {
                                send_direct_message(recipient, &full_message).await;
                            }
                            let _ = increment_user_sent_messages(&db_conn, &username).await;
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

    set_user_status(&db_conn, &username, "offline").await;
    let _ = update_user_time_online(&db_conn, &username, Utc::now().timestamp() - start_time).await;
}

async fn broadcast_message(
    active_connections: &Arc<RwLock<Vec<Arc<Mutex<tokio::net::TcpStream>>>>>,
    message: &str,
) {
    info!(target: "server", "Broadcasting message: {}", message);

    let connections = active_connections.read().await;
    for client in connections.iter() {
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

async fn add_or_update_user(db_conn: &Arc<Mutex<Connection>>, username: &str) {
    let conn = db_conn.lock().await;
    let mut stmt = conn.prepare("SELECT username FROM users WHERE username = ?1").unwrap();
    let user_exists = stmt.exists([username]).unwrap();

    if !user_exists {
        let mut stmt = conn.prepare("INSERT INTO users (username, status, last_online, messages_sent, total_time_online, permission) VALUES (?1, 'online', ?2, 0, 0, 'user')").unwrap();
        stmt.execute([username.to_string(), Utc::now().timestamp_millis().to_string()]).unwrap();
    } else {
        let mut stmt = conn.prepare("UPDATE users SET status = 'online', last_online = ?1 WHERE username = ?2").unwrap();
        stmt.execute([Utc::now().timestamp_millis().to_string(), username.to_string()]).unwrap();
    }
}

async fn set_user_status(db_conn: &Arc<Mutex<Connection>>, username: &str, status: &str) {
    let conn = db_conn.lock().await;
    let mut stmt = conn.prepare("UPDATE users SET status = ?1 WHERE username = ?2").unwrap();
    stmt.execute([status.to_string(), username.to_string()]).unwrap();
}

async fn increment_user_sent_messages(db_conn: &Arc<Mutex<Connection>>, username: &str) -> Result<(), rusqlite::Error> {
    let conn = db_conn.lock().await;

    {
        let mut stmt = conn.prepare("UPDATE users SET messages_sent = messages_sent + 1 WHERE username = ?1")?;
        stmt.execute([username.to_string()])?;
    }

    {
        let mut stmt2 = conn.prepare("
            INSERT INTO server_data (key, value)
            VALUES ('messages_sent', '1')
            ON CONFLICT(key) DO UPDATE SET value = value + 1
        ")?;
        stmt2.execute([])?;
    }

    Ok(())
}

async fn update_user_time_online(db_conn: &Arc<Mutex<Connection>>, username: &str, time_online: i64) -> Result<(), rusqlite::Error> {
    let conn = db_conn.lock().await;
    {
        let mut stmt = conn.prepare("UPDATE users SET total_time_online = total_time_online + ?1 WHERE username = ?2")?;
        stmt.execute([&time_online.to_string(), &username.to_string()])?;
    }

    {
        let mut stmt2 = conn.prepare("
            INSERT INTO server_data (key, value)
            VALUES ('total_time_online', ?1)
            ON CONFLICT(key) DO UPDATE SET value = value + ?1
        ")?;
        stmt2.execute([&time_online.to_string(), &time_online.to_string()])?;
    }
    Ok(())
}
