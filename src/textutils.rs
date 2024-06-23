pub fn format_outgoing_message(username: &str, recipient: &str, command_message: &str, timestamp: i64) -> String {
    return format!("{}:{}:{}:{}", timestamp, username, recipient, command_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_format_outgoing_message() {
        let username = "testuser";
        let recipient = "global";
        let command_message = "Hello, world!";
        let timestamp = Utc::now().timestamp_millis();
        let formatted_message = format_outgoing_message(username, recipient, command_message, timestamp);
        assert_eq!(formatted_message, format!("{}:{}:{}:{}", timestamp, username, recipient, command_message));
    }
}
