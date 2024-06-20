use warp::{Filter, Rejection, reject::custom};
use std::net::IpAddr;
use tokio::sync::RwLock;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use rusqlite::Connection;
use crate::conn_handler::ActiveUsers;
use crate::config::Config;
use chrono::Utc;
use crate::validators;

const HTML_CONTENT: &str = include_str!("../web/index.html");

#[derive(Serialize)]
struct ServerInfo {
    total_messages: usize,
    total_time_online: String,
    uptime: String,
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

#[derive(Serialize)]
struct ResponseMessage {
    message: String,
}

#[derive(Deserialize)]
struct UserBanRequest {
    username: String,
}

#[derive(Deserialize)]
struct TempBanRequest {
    username: String,
    ban_duration: u32,
}

#[derive(Debug)]
struct DatabaseError;

#[derive(Debug)]
struct ValidationError;

impl warp::reject::Reject for DatabaseError {}
impl warp::reject::Reject for ValidationError {}

pub async fn run_web_ui(
    host: String,
    port: u16,
    active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>,
    active_users: ActiveUsers,
    _config: Config,
    db_conn: Arc<Mutex<Connection>>,
) {
    let index_route = warp::path::end().map(move || warp::reply::html(HTML_CONTENT));

    let db_conn_info = Arc::clone(&db_conn);
    let info_route = warp::path("api")
        .and(warp::path("info"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&db_conn_info)))
        .and_then(move |db_conn: Arc<Mutex<Connection>>| async move {
            let result: Result<ServerInfo, Rejection> = {
                let conn = db_conn.lock().map_err(|_| custom(DatabaseError))?;
                let mut stmt = conn.prepare("SELECT data FROM server_data WHERE key = 'messages_sent'")
                    .map_err(|_| custom(DatabaseError))?;
                let total_messages: usize = stmt.query_row([], |row| row.get(0))
                    .map_err(|_| custom(DatabaseError))?;

                let mut stmt = conn.prepare("SELECT data FROM server_data WHERE key = 'total_time_online'")
                    .map_err(|_| custom(DatabaseError))?;
                let total_time_online: String = stmt.query_row([], |row| row.get(0))
                    .map_err(|_| custom(DatabaseError))?;

                Ok(ServerInfo {
                    total_messages,
                    total_time_online,
                    uptime: Utc::now().to_rfc3339(),
                })
            };

            result.map(|server_info| warp::reply::json(&server_info))
        });

    let db_conn_users = Arc::clone(&db_conn);
    let users_route = warp::path("api")
        .and(warp::path("users"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&db_conn_users)))
        .and_then(move |db_conn: Arc<Mutex<Connection>>| async move {
            let result: Result<Vec<User>, Rejection> = {
                let conn = db_conn.lock().map_err(|_| custom(DatabaseError))?;
                let mut stmt = conn.prepare("SELECT username, status, last_online, messages_sent, total_time_online, permission FROM users")
                    .map_err(|_| custom(DatabaseError))?;
                let user_iter = stmt.query_map([], |row| {
                    Ok(User {
                        username: row.get(0)?,
                        status: row.get(1)?,
                        last_online: row.get(2)?,
                        messages_sent: row.get(3)?,
                        total_time_online: row.get(4)?,
                        permission: row.get(5)?,
                    })
                }).map_err(|_| custom(DatabaseError))?;

                let users: Vec<User> = user_iter.filter_map(Result::ok).collect();
                Ok(users)
            };

            result.map(|users| warp::reply::json(&users))
        });

    let active_connections_route = warp::path("api")
        .and(warp::path("active-connections"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&active_connections)))
        .and_then(|active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>| async move {
            let conns = active_connections.read().await;
            let count = conns.len();

            Ok::<_, Rejection>(warp::reply::json(&ActiveConnectionCount { count }))
        });

    let active_users_route = warp::path("api")
        .and(warp::path("active-users"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&active_users)))
        .and_then(|active_users: ActiveUsers| async move {
            let users = active_users.read().await;
            let active_users_list: Vec<String> = users.keys().cloned().collect();

            Ok::<_, Rejection>(warp::reply::json(&active_users_list))
        });

    let db_conn_ban = Arc::clone(&db_conn);
    let ban_user_route = warp::path("api")
        .and(warp::path("ban"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || Arc::clone(&db_conn_ban)))
        .and_then(|body: UserBanRequest, db_conn: Arc<Mutex<Connection>>| async move {
            let result: Result<(), Rejection> = {
                let username = &body.username;
                if !validators::validate_username(&username).await {
                    return Err(custom(ValidationError));
                }
                let conn = db_conn.lock().map_err(|_| custom(DatabaseError))?;
                let mut stmt = conn.prepare("UPDATE users SET status = 'banned:indefinitely' WHERE username = ?1")
                    .map_err(|_| custom(DatabaseError))?;
                stmt.execute(&[username]).map_err(|_| custom(DatabaseError))?;
                Ok(())
            };

            result.map(|_| warp::reply::json(&ResponseMessage {
                message: format!("User {} has been banned indefinitely", body.username),
            }))
        });

    let db_conn_temp_ban = Arc::clone(&db_conn);
    let temp_ban_user_route = warp::path("api")
        .and(warp::path("temp-ban"))
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || Arc::clone(&db_conn_temp_ban)))
        .and_then(|body: TempBanRequest, db_conn: Arc<Mutex<Connection>>| async move {
            let result: Result<(), Rejection> = {
                let username = &body.username;
                if !validators::validate_username(&username).await {
                    return Err(custom(ValidationError));
                }
                let conn = db_conn.lock().map_err(|_| custom(DatabaseError))?;
                let ban_until = Utc::now().timestamp() + (body.ban_duration as i64 * 3600);
                let mut stmt = conn.prepare("UPDATE users SET status = ?1 WHERE username = ?2")
                    .map_err(|_| custom(DatabaseError))?;
                stmt.execute(&[&format!("tempbanned:{}", ban_until), username])
                    .map_err(|_| custom(DatabaseError))?;
                Ok(())
            };

            result.map(|_| warp::reply::json(&ResponseMessage {
                message: format!("User {} has been temporarily banned for {} hours", body.username, body.ban_duration),
            }))
        });

    let routes = index_route
        .or(info_route)
        .or(users_route)
        .or(active_connections_route)
        .or(active_users_route)
        .or(ban_user_route)
        .or(temp_ban_user_route);

    warp::serve(routes)
        .run((host.parse::<IpAddr>().unwrap(), port))
        .await;
}
