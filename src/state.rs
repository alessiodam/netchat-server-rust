// src/state.rs
use tokio::sync::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::net::TcpStream;
use lazy_static::lazy_static;

type ActiveConnections = Arc<RwLock<Vec<Arc<tokio::sync::Mutex<TcpStream>>>>>;
pub(crate) type ActiveUsers = Arc<RwLock<HashMap<String, Arc<tokio::sync::Mutex<TcpStream>>>>>;
type ChatRooms = Arc<RwLock<HashMap<String, Vec<Arc<tokio::sync::Mutex<TcpStream>>>>>>;

lazy_static! {
    pub static ref ACTIVE_CONNECTIONS: ActiveConnections = Arc::new(RwLock::new(Vec::new()));
    pub static ref CHAT_ROOMS: ChatRooms = Arc::new(RwLock::new(HashMap::new()));
    pub static ref ACTIVE_USERS: ActiveUsers = Arc::new(RwLock::new(HashMap::new()));
}

pub fn get_active_connections() -> ActiveConnections {
    Arc::clone(&ACTIVE_CONNECTIONS)
}

pub fn get_chat_rooms() -> ChatRooms {
    Arc::clone(&CHAT_ROOMS)
}

pub fn get_active_users() -> ActiveUsers {
    Arc::clone(&ACTIVE_USERS)
}
