use chrono::Utc;
use rusqlite::{Connection, Result};
use crate::DB_PATH;

pub fn get_db_conn() -> Result<Connection> {
    let conn = Connection::open(DB_PATH)?;
    Ok(conn)
}

pub fn init_db() -> Result<Connection> {
    let conn = get_db_conn().unwrap();
    conn.execute("
        CREATE TABLE IF NOT EXISTS users (
            username TEXT PRIMARY KEY,
            status TEXT,
            last_online TEXT,
            messages_sent INTEGER,
            total_time_online TEXT,
            permission TEXT
        )",
                 [],
    )?;
    conn.execute("
        CREATE TABLE IF NOT EXISTS server_data (
            key TEXT PRIMARY KEY,
            value TEXT
        )",
                 [],
    )?;

    conn.execute("
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            timestamp TEXT,
            username TEXT,
            recipient TEXT,
            message TEXT
        )",
                 [],
    )?;
    Ok(conn)
}

pub fn add_or_update_user(username: &str) {
    let conn = get_db_conn().unwrap();
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

pub fn set_user_status(username: &str, status: &str) {
    let conn = get_db_conn().unwrap();
    let mut stmt = conn.prepare("UPDATE users SET status = ?1 WHERE username = ?2").unwrap();
    stmt.execute([status.to_string(), username.to_string()]).unwrap();
}

pub fn increment_user_sent_messages(username: &str) -> std::result::Result<(), rusqlite::Error> {
    let conn = get_db_conn().unwrap();

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

pub fn update_user_time_online(username: &str, time_online: i64) -> std::result::Result<(), rusqlite::Error> {
    let conn = get_db_conn().unwrap();
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

pub fn update_user_data(username: &str, key: &str, value: &str) {
    let conn = get_db_conn().unwrap();
    let mut stmt = conn.prepare("UPDATE users SET ?1 = ?2 WHERE username = ?3").unwrap();
    stmt.execute([key, value, username]).unwrap();
}
