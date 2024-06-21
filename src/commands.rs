use std::collections::HashMap;
use sysinfo::System;

// This will register the commands, this is NEEDED to be updated for commands to work.
pub fn get_commands() -> HashMap<&'static str, Box<dyn Command>> {
    let mut commands: HashMap<&'static str, Box<dyn Command>> = HashMap::new();

    // put commands here
    commands.insert("!perf", Box::new(PerfCommand));

    commands
}

pub trait Command: Send + Sync {
    fn execute(&self, args: &[&str]) -> Vec<u8>;
}

#[derive(Clone)]
pub struct PerfCommand;

impl Command for PerfCommand {
    fn execute(&self, _args: &[&str]) -> Vec<u8> {
        let mut system = System::new_all();
        system.refresh_all();

        let cpu_usage = system.global_cpu_info().cpu_usage();
        let total_memory = system.total_memory() / 1024 / 1024;
        let used_memory = system.used_memory() / 1024 / 1024;

        let mut response = String::new();
        response.push_str(&format!("CPU Usage: {:.2}%", cpu_usage));
        response.push_str(&format!(", RAM Usage: {}MB/{}MB", used_memory, total_memory));
        response.into_bytes()
    }
}
