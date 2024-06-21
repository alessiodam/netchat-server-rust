use std::collections::HashMap;
use sysinfo::System;
use crate::state::{get_active_users};
use futures::future::BoxFuture;

pub trait Command: Send + Sync {
    fn execute<'a>(&'a self, args: &'a [&'a str]) -> BoxFuture<'a, Vec<u8>>;
}

pub fn get_commands() -> HashMap<&'static str, Box<dyn Command>> {
    let mut commands: HashMap<&'static str, Box<dyn Command>> = HashMap::new();

    // put commands here
    commands.insert("!perf", Box::new(PerfCommand));
    commands.insert("!list", Box::new(ListCommand));

    commands
}

#[derive(Clone)]
pub struct PerfCommand;
impl Command for PerfCommand {
    fn execute<'a>(&'a self, _args: &'a [&'a str]) -> BoxFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let mut system = System::new_all();
            system.refresh_all();

            let cpu_usage = system.global_cpu_info().cpu_usage();
            let total_memory = system.total_memory() / 1024 / 1024;
            let used_memory = system.used_memory() / 1024 / 1024;

            let mut response = String::new();
            response.push_str(&format!("CPU Usage: {:.2}%", cpu_usage));
            response.push_str(&format!(", RAM Usage: {}MB/{}MB", used_memory, total_memory));
            response.into_bytes()
        })
    }
}

#[derive(Clone)]
pub struct ListCommand;
impl Command for ListCommand {
    fn execute<'a>(&'a self, _args: &'a [&'a str]) -> BoxFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let mut response = String::new();
            response.push_str("Users: ");
            {
                for (user, _) in get_active_users().read().await.iter() {
                    response.push_str(&format!("{}, ", user));
                }
            }
            response.into_bytes()
        })
    }
}
