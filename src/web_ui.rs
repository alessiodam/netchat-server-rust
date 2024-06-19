use warp::{Filter, Rejection};
use std::net::IpAddr;
use tokio::sync::RwLock;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use rusqlite::Connection;
use crate::conn_handler::ActiveUsers;
use crate::config::Config;
use chrono::Utc;

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
    session_duration: String,
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
struct ActiveUser {
    username: String,
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
    duration: u32,
}

pub async fn run_web_ui(
    host: String,
    port: u16,
    active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>,
    active_users: ActiveUsers,
    config: Config,
    db_conn: Arc<Mutex<Connection>>,
) {
    let index_route = warp::path::end().map(move || warp::reply::html(HTML_CONTENT));

    let db_conn_info = Arc::clone(&db_conn);
    let info_route = warp::path("api")
        .and(warp::path("info"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&db_conn_info)))
        .and_then(move |db_conn: Arc<Mutex<Connection>>| async move {
            let conn = db_conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT total_messages, total_time_online, uptime FROM server_info LIMIT 1").unwrap();
            let server_info_iter = stmt.query_map([], |row| {
                Ok(ServerInfo {
                    total_messages: row.get(0)?,
                    total_time_online: row.get(1)?,
                    uptime: row.get(2)?,
                })
            }).unwrap();

            let server_info = server_info_iter.into_iter().next().unwrap_or_else(|| Ok(ServerInfo {
                total_messages: 0,
                total_time_online: "0".to_string(),
                uptime: "0".to_string(),
            })).unwrap();

            Ok::<_, Rejection>(warp::reply::json(&server_info))
        });

    let db_conn_users = Arc::clone(&db_conn);
    let users_route = warp::path("api")
        .and(warp::path("users"))
        .and(warp::get())
        .and(warp::any().map(move || Arc::clone(&db_conn_users)))
        .and_then(move |db_conn: Arc<Mutex<Connection>>| async move {
            let conn = db_conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT username, status, session_duration, last_online, messages_sent, total_time_online, permission FROM users").unwrap();
            let user_iter = stmt.query_map([], |row| {
                Ok(User {
                    username: row.get(0)?,
                    status: row.get(1)?,
                    session_duration: row.get(2)?,
                    last_online: row.get(3)?,
                    messages_sent: row.get(4)?,
                    total_time_online: row.get(5)?,
                    permission: row.get(6)?,
                })
            }).unwrap();

            let users: Vec<User> = user_iter.map(|user| user.unwrap()).collect();

            Ok::<_, Rejection>(warp::reply::json(&users))
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
            let conn = db_conn.lock().unwrap();
            let mut stmt = conn.prepare("UPDATE users SET status = 'banned:indefinitely' WHERE username = ?1").unwrap();
            stmt.execute(&[&body.username]).unwrap();

            Ok::<_, Rejection>(warp::reply::json(&ResponseMessage {
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
            let ban_until = Utc::now().timestamp() + (body.duration as i64 * 3600);
            let conn = db_conn.lock().unwrap();
            let mut stmt = conn.prepare("UPDATE users SET status = ?1 WHERE username = ?2").unwrap();
            stmt.execute(&[&format!("tempbanned:{}", ban_until), &body.username]).unwrap();

            Ok::<_, Rejection>(warp::reply::json(&ResponseMessage {
                message: format!("User {} has been temporarily banned for {} hours", body.username, body.duration),
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
