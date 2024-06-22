use tokio::net::TcpListener;
use std::error::Error;
use tracing_subscriber::fmt;
use std::sync::Arc;
use tokio::signal;
use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use std::fs;
use reqwest;

mod config;
mod auth;
mod conn_handler;
mod web_ui;
mod db;
mod validators;
mod commands;
mod state;
mod textutils;

use config::Config;
use conn_handler::handle_connection;
use db::init_db;
use crate::commands::get_commands;
use crate::state::get_active_users;

const CONFIG_URL: &str = "https://raw.githubusercontent.com/tkbstudios/netchat-server-rust/master/config.toml.example";
const CONFIG_PATH: &str = "config.toml";
const DB_PATH: &str = if cfg!(test) {
    "netchat-test.db"
} else {
    "netchat.db"
};

fn init_tracing() {
    fmt::init();
}

async fn fetch_and_save_config() -> Result<(), Box<dyn Error + Send + Sync>> {
    if !DB_PATH.ends_with(".db") {
        return Err("DB path must end with .db".into());
    }
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
    let config_clone_for_web = config.clone();

    init_db().expect("Failed to initialize database");

    tracing::info!(target: "tcpserver", "Starting server with online mode: {} on {}:{}", config.server.online_mode, config.server.host, config.server.port);

    let listener = TcpListener::bind(format!("{}:{}", config.server.host, config.server.port)).await?;
    let active_connections = state::get_active_connections();
    let chat_rooms = state::get_chat_rooms();

    {
        let mut chat_rooms = chat_rooms.write().await;
        chat_rooms.insert("global".to_string(), Vec::new());
    }

    if config.web.enable {
        tokio::spawn(async move {
            web_ui::run_web_ui(config_clone_for_web).await;
        });
    }

    tokio::spawn(remove_non_authenticated_connections());

    loop {
        tokio::select! {
            Ok((socket, _)) = listener.accept() => {
                tracing::info!(target: "tcpserver", "New connection accepted");

                let socket = Arc::new(tokio::sync::Mutex::new(socket));
                let commands_clone = get_commands();

                {
                    let mut conns = active_connections.write().await;
                    conns.push(socket.clone());
                }

                let config_clone = config.clone();
                tokio::spawn(handle_connection(
                    socket,
                    config_clone,
                    commands_clone
                ));
            },

            _ = signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, notifying all clients...");

                let active_connections = state::get_active_connections();
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

async fn remove_non_authenticated_connections() {
    loop {
        sleep(Duration::from_secs(60)).await;
        let mut connections_to_remove = Vec::new();
        let active_users = get_active_users();
        let active_connections = state::get_active_connections();
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
            let active_connections = state::get_active_connections();
            let mut conns = active_connections.write().await;
            let conn = conns.remove(*index);
            {
                let mut conn = conn.lock().await;
                let _ = conn.shutdown().await;
            }
        }
    }
}
