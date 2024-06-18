use tokio::net::TcpListener;
use std::error::Error;
use tracing_subscriber::fmt;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::signal;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};

mod config;
mod auth;
mod conn_handler;
mod web_ui;

use config::Config;
use conn_handler::{handle_connection, ActiveUsers, ChatRooms};

fn init_tracing() {
    fmt::init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    init_tracing();

    let config = Config::load_config().expect("Failed to load config");

    tracing::info!(target: "server", "Starting server with online mode: {} on {}:{}", config.server.online_mode, config.server.host, config.server.port);

    let listener = TcpListener::bind(format!("{}:{}", config.server.host, config.server.port)).await?;
    let active_connections = Arc::new(RwLock::new(Vec::new()));
    let chat_rooms: ChatRooms = Arc::new(RwLock::new(HashMap::new()));
    let active_users: ActiveUsers = Arc::new(RwLock::new(HashMap::new()));

    {
        let mut chat_rooms = chat_rooms.write().await;
        chat_rooms.insert("global".to_string(), Vec::new());
    }

    if config.web.enable {
        let web_host = config.web.host.clone();
        let web_port = config.web.port;
        let active_connections = Arc::clone(&active_connections);
        let active_users = Arc::clone(&active_users);
        let config_clone = config.clone();
        tokio::spawn(async move {
            web_ui::run_web_ui(web_host, web_port, active_connections, active_users, config_clone).await;
        });
    }

    tokio::spawn(remove_non_authenticated_connections(active_connections.clone(), active_users.clone()));

    loop {
        tokio::select! {
            Ok((socket, _)) = listener.accept() => {
                tracing::info!(target: "server", "New connection accepted");

                let active_connections = active_connections.clone();
                let active_users = active_users.clone();
                let config_clone = config.clone();
                let socket = Arc::new(tokio::sync::Mutex::new(socket));

                {
                    let mut conns = active_connections.write().await;
                    conns.push(socket.clone());
                }

                tokio::spawn(handle_connection(socket, active_connections, active_users, config_clone));
            },
            _ = signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, notifying all clients...");

                let conns = active_connections.read().await;
                for conn in conns.iter() {
                    let conn = conn.clone();
                    tokio::spawn(async move {
                        let mut conn = conn.lock().await;
                        let _ = conn.write_all(b"SERVER_SHUTDOWN\n").await;
                    });
                }

                sleep(Duration::from_secs(5)).await;

                tracing::info!("Server shutting down after 5 seconds.");
                break;
            },
        }
    }

    Ok(())
}

async fn remove_non_authenticated_connections(
    active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>,
    active_users: ActiveUsers,
) {
    loop {
        sleep(Duration::from_secs(60)).await;
        let mut connections_to_remove = Vec::new();
        {
            let active_users = active_users.read().await;
            let active_connections = active_connections.read().await;
            for (index, conn) in active_connections.iter().enumerate() {
                if !active_users.values().any(|user_conn| Arc::ptr_eq(user_conn, conn)) {
                    connections_to_remove.push(index);
                }
            }
        }
        for index in connections_to_remove.iter().rev() {
            let mut conns = active_connections.write().await;
            let conn = conns.remove(*index);
            {
                let mut conn = conn.lock().await;
                let _ = conn.shutdown().await;
            }
        }
    }
}
