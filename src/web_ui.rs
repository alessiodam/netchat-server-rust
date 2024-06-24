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
use std::str::FromStr;
use axum_server::Server;
use rusqlite::Connection;
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

fn get_value_from_db<T: FromStr>(conn: &Connection, key: &str) -> Result<T, DatabaseError> {
    let mut stmt = conn.prepare("SELECT value FROM server_data WHERE key = ?1").unwrap();
    let result: Result<String, rusqlite::Error> = stmt.query_row([key], |row| row.get(0));

    match result {
        Ok(value) => {
            value.parse::<T>().map_err(|_| DatabaseError)
        },
        Err(_) => Err(DatabaseError),
    }
}

// handlers
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn info_handler() -> Result<Json<ServerInfo>, DatabaseError> {
    let conn = get_db_conn().map_err(|_| DatabaseError)?;

    let total_messages = get_value_from_db(&conn, "messages_sent").unwrap();
    let total_time_online = get_value_from_db(&conn, "total_time_online").unwrap();
    let uptime = 0; // TODO: implement this

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

    tracing::info!(target: "webserver", "Starting web server on {}:{}", config.web.host, config.web.port);
    Server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}


#[cfg(test)]
mod tests {
    use crate::db::init_db;
    use super::*;

    #[test]
    fn test_get_value_from_db() {
        init_db().unwrap();
        let conn = get_db_conn().unwrap();



        let result = get_value_from_db::<i32>(&conn, "total_time_online").unwrap();
        assert_eq!(result, 0);
        let result = get_value_from_db::<i32>(&conn, "messages_sent").unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_get_value_from_db_error() {
        init_db().unwrap();
        let conn = get_db_conn().unwrap();
        let result = get_value_from_db::<i32>(&conn, "invalid_key");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_value_from_db_default() {
        init_db().unwrap();
        let conn = get_db_conn().unwrap();
        let result = get_value_from_db::<i32>(&conn, "invalid_key").unwrap_or_default();
        assert_eq!(result, 0);
    }
}
