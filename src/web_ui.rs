use axum::{
    extract::Json,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Serialize;
use crate::state::{get_active_connections, get_active_users};
use crate::config::Config;
use std::net::SocketAddr;
use axum_server::Server;
use crate::db::get_db_conn;

#[derive(Serialize)]
struct ServerInfo {
    total_messages: usize,
    total_time_online: usize,
    uptime: usize,
}

#[derive(Serialize)]
struct User {
    username: String,
    status: String,
    last_online: String,
    messages_sent: usize,
    total_time_online: String,
    permission: String,
}

#[derive(Serialize)]
struct ActiveConnectionCount {
    count: usize,
}

#[derive(Debug)]
struct DatabaseError;

impl IntoResponse for DatabaseError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
    }
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn info_handler() -> Result<Json<ServerInfo>, DatabaseError> {
    let conn = get_db_conn().map_err(|_| DatabaseError)?;

    let total_messages = {
        let mut stmt = conn.prepare("SELECT value FROM server_data WHERE key = 'messages_sent'")
            .map_err(|_| DatabaseError)?;
        stmt.query_row([], |row| row.get(0))
            .unwrap_or(0)
    };

    let total_time_online = {
        let mut stmt = conn.prepare("SELECT value FROM server_data WHERE key = 'total_time_online'")
            .map_err(|_| DatabaseError)?;
        stmt.query_row([], |row| row.get(0))
            .unwrap_or(0)
    };

    let uptime = 0;

    Ok(Json(ServerInfo {
        total_messages,
        total_time_online,
        uptime,
    }))
}

async fn users_handler() -> Result<Json<Vec<User>>, DatabaseError> {
    let conn = get_db_conn().map_err(|_| DatabaseError)?;
    let mut stmt = conn.prepare("SELECT username, status, last_online, messages_sent, total_time_online, permission FROM users")
        .map_err(|_| DatabaseError)?;
    let user_iter = stmt.query_map([], |row| {
        Ok(User {
            username: row.get(0)?,
            status: row.get(1)?,
            last_online: row.get(2)?,
            messages_sent: row.get(3)?,
            total_time_online: row.get(4)?,
            permission: row.get(5)?,
        })
    }).map_err(|_| DatabaseError)?;

    let users: Vec<User> = user_iter.filter_map(Result::ok).collect();
    Ok(Json(users))
}

async fn active_connections_handler() -> Json<ActiveConnectionCount> {
    let active_connections = get_active_connections();
    let connections = active_connections.read().await;
    let count = connections.len();
    Json(ActiveConnectionCount { count })
}

async fn active_users_handler() -> Json<Vec<String>> {
    let active_users = get_active_users();
    let users = active_users.read().await;
    let active_users_list: Vec<String> = users.keys().cloned().collect();
    Json(active_users_list)
}

pub async fn run_web_ui(
    config: Config,
) {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/info", get(info_handler))
        .route("/api/users", get(users_handler))
        .route("/api/active-connections", get(active_connections_handler))
        .route("/api/active-users", get(active_users_handler));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.web.port));

    tracing::info!(target: "server", "Starting web server on {}:{}", config.web.host, config.web.port);
    Server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
