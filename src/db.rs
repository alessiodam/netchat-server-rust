use chrono::Utc;
use rusqlite::{Connection, params, Result};
use crate::DB_PATH;

pub fn get_db_conn() -> Result<Connection> {
    Connection::open(DB_PATH)
}

pub fn init_db() -> Result<Connection> {
    let conn = get_db_conn()?;
    conn.execute("
        CREATE TABLE IF NOT EXISTS users (
            username TEXT PRIMARY KEY,
            status TEXT,
            last_online TEXT,
            messages_sent INTEGER,
            total_time_online TEXT,
            permission TEXT
        )", [],
    )?;
    conn.execute("
        CREATE TABLE IF NOT EXISTS server_data (
            key TEXT PRIMARY KEY,
            value TEXT
        )", [],
    )?;
    conn.execute("
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            timestamp TEXT,
            username TEXT,
            recipient TEXT,
            message TEXT,
            FOREIGN KEY(username) REFERENCES users(username) ON DELETE CASCADE
        )", [],
    )?;
    Ok(conn)
}

pub fn add_or_update_user(username: &str) {
    let conn = get_db_conn().unwrap();
    let mut stmt = conn.prepare("SELECT username FROM users WHERE username = ?1").unwrap();
    let user_exists = stmt.exists(params![username]).unwrap();

    if !user_exists {
        conn.execute(
            "INSERT INTO users (username, status, last_online, messages_sent, total_time_online, permission) VALUES (?1, 'online', ?2, 0, 0, 'user')",
            params![username, Utc::now().timestamp_millis().to_string()],
        ).unwrap();
    } else {
        conn.execute(
            "UPDATE users SET status = 'online', last_online = ?1 WHERE username = ?2",
            params![Utc::now().timestamp_millis().to_string(), username],
        ).unwrap();
    }
}

pub fn set_user_status(username: &str, status: &str) {
    let conn = get_db_conn().unwrap();
    conn.execute(
        "UPDATE users SET status = ?1 WHERE username = ?2",
        params![status, username],
    ).unwrap();
}

pub fn increment_user_sent_messages(username: &str) -> Result<()> {
    let conn = get_db_conn()?;

    conn.execute(
        "UPDATE users SET messages_sent = messages_sent + 1 WHERE username = ?1",
        params![username],
    )?;

    conn.execute(
        "INSERT INTO server_data (key, value) VALUES ('messages_sent', '1') ON CONFLICT(key) DO UPDATE SET value = value + 1",
        params![],
    )?;

    Ok(())
}

pub fn update_user_time_online(username: &str, time_online: i64) -> Result<()> {
    let conn = get_db_conn()?;

    conn.execute(
        "UPDATE users SET total_time_online = total_time_online + ?1 WHERE username = ?2",
        params![time_online, username],
    )?;

    conn.execute(
        "INSERT INTO server_data (key, value) VALUES ('total_time_online', ?1) ON CONFLICT(key) DO UPDATE SET value = value + ?1",
        params![time_online],
    )?;

    Ok(())
}

pub fn update_user_data(username: &str, key: &str, value: &str) {
    let conn = get_db_conn().unwrap();
    let sql = format!("UPDATE users SET {} = ?1 WHERE username = ?2", key);
    conn.execute(&sql, params![value, username]).unwrap();
}

pub fn add_message_to_db(timestamp: i64, username: &str, recipient: &str, message: &str) -> Result<()> {
    let conn = get_db_conn()?;
    conn.execute(
        "INSERT INTO messages (timestamp, username, recipient, message) VALUES (?1, ?2, ?3, ?4)",
        params![timestamp, username, recipient, message],
    )?;
    Ok(())
}
