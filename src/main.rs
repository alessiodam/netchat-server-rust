/*
 * NETCHAT Server, in Rust.
 *
 * Dear Rust developers/community
 * Please don't get mad at me if the code is bd
 * This is only my second project with Rust :(
 *
 * Therefore, do NOT try to optimize this code.
 * When I wrote it, only God and I knew what we wrote.
 * Increase this counter as a warning to the next one:
 * wasted hours = 6 hours 10 Minutes
*/

use tokio::net::TcpListener;
use std::error::Error;
use tracing_subscriber::fmt;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::signal;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use std::fs;
use reqwest;
use std::sync::Mutex;

mod config;
mod auth;
mod conn_handler;
mod web_ui;
mod db;

use config::Config;
use conn_handler::{handle_connection, ActiveUsers, ChatRooms};
use db::init_db;

const CONFIG_URL: &str = "https://raw.githubusercontent.com/tkbstudios/netchat-server-rust/master/config.toml.example";
const CONFIG_PATH: &str = "config.toml";
const DB_PATH: &str = "netchat.db";

fn init_tracing() {
    fmt::init();
}

async fn fetch_and_save_config() -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = reqwest::get(CONFIG_URL).await?;
    let content = response.text().await?;
    fs::write(CONFIG_PATH, content)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    init_tracing();

    if !std::path::Path::new(CONFIG_PATH).exists() {
        tracing::warn!("Config file not found, fetching from remote...");
        fetch_and_save_config().await.expect("Failed to fetch config file");
        tracing::info!("Config file fetched and saved as config.toml. Please edit it carefully before restarting the server.");
        return Ok(());
    }

    let config = Config::load_config().expect("Failed to load config");

    let db_conn = Arc::new(Mutex::new(init_db(DB_PATH)?)); // Use Arc<Mutex<Connection>>

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
        let db_conn_clone = Arc::clone(&db_conn);
        tokio::spawn(async move {
            web_ui::run_web_ui(web_host, web_port, active_connections, active_users, config_clone, db_conn_clone).await;
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
                let db_conn = Arc::new(tokio::sync::Mutex::new(init_db(DB_PATH)?));

                {
                    let mut conns = active_connections.write().await;
                    conns.push(socket.clone());
                }

                tokio::spawn(handle_connection(socket, active_connections, active_users, config_clone, db_conn));
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
