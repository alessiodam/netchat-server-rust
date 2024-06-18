use warp::Filter;
use std::net::IpAddr;
use tokio::sync::RwLock;
use std::sync::Arc;
use crate::conn_handler::ActiveUsers;
use crate::config::Config;

pub async fn run_web_ui(host: String, port: u16, active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>, active_users: ActiveUsers, config: Config) {
    let active_connections = warp::any().map(move || Arc::clone(&active_connections));
    let active_users = warp::any().map(move || Arc::clone(&active_users));
    let host_clone = host.clone();
    let port_clone = port;
    let online_mode_clone = config.server.online_mode;

    let route = warp::path::end()
        .and(active_connections)
        .and(active_users)
        .and_then(move |active_connections: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::net::TcpStream>>>>>, active_users: ActiveUsers| {
            let host_clone = host_clone.clone();
            let port_clone = port_clone.clone();
            let online_mode_clone = online_mode_clone.clone();
            async move {
                let num_clients = active_connections.read().await.len();
                let usernames: Vec<String> = active_users.read().await.keys().cloned().collect();

                Ok::<_, warp::Rejection>(warp::reply::html(format!(
                    "<html>
                        <head>
                            <title>Server Info</title>
                            <style>
                                body {{
                                    font-family: Arial, sans-serif;
                                    margin: 40px;
                                    background-color: #f4f4f9;
                                }}
                                h1 {{
                                    color: #333;
                                }}
                                p {{
                                    color: #666;
                                }}
                                ul {{
                                    list-style-type: none;
                                    padding: 0;
                                }}
                                li {{
                                    background: #e2e2e2;
                                    margin: 5px 0;
                                    padding: 10px;
                                    border-radius: 4px;
                                }}
                            </style>
                        </head>
                        <body>
                            <h1>Server Info</h1>
                            <p><strong>IP:</strong> {}</p>
                            <p><strong>Port:</strong> {}</p>
                            <p><strong>Online mode:</strong> {}</p>
                            <p><strong>Number of connected clients:</strong> {}</p>
                            <p><strong>Logged in users:</strong></p>
                            <ul>
                                {}
                            </ul>
                        </body>
                    </html>",
                    host_clone,
                    port_clone,
                    online_mode_clone,
                    num_clients,
                    usernames.into_iter().map(|user| format!("<li>{}</li>", user)).collect::<String>()
                )))
            }
        });

    warp::serve(route)
        .run((host.parse::<IpAddr>().unwrap(), port))
        .await;
}
