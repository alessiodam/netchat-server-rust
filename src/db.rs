use rusqlite::{Connection, Result};

pub fn init_db(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            username TEXT PRIMARY KEY,
            status TEXT,
            session_duration TEXT,
            last_online TEXT,
            messages_sent INTEGER,
            total_time_online TEXT,
            permission TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS server_info (
            total_messages INTEGER,
            total_time_online TEXT,
            uptime TEXT
        )",
        [],
    )?;
    Ok(conn)
}
