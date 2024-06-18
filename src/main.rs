use tokio::net::TcpListener;
use std::error::Error;
use tracing_subscriber::fmt;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::signal;

mod auth;
mod conn_handler;

use auth::Config;
use conn_handler::handle_connection;

fn init_tracing() {
    fmt::init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    init_tracing();

    let config = Config::load_config();

    tracing::info!(target: "server", "Starting server with online mode: {} on {}:{}", config.online_mode, config.host, config.port);

    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;
    let active_connections = Arc::new(RwLock::new(Vec::new()));

    loop {
        tokio::select! {
            Ok((socket, _)) = listener.accept() => {
                tracing::info!(target: "server", "New connection accepted");

                let active_connections = active_connections.clone();
                let config_clone = config.clone();
                let socket = Arc::new(tokio::sync::Mutex::new(socket));

                {
                    let mut conns = active_connections.write().await;
                    conns.push(socket.clone());
                }

                tokio::spawn(handle_connection(socket, active_connections, config_clone));
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

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                tracing::info!("Server shutting down after 5 seconds.");
                break;
            },
        }
    }

    Ok(())
}
