pub async fn validate_username(username: &str) -> bool {
    if username.len() < 3 || username.len() > 18 {
        return false;
    }

    if !regex::Regex::new(r"^[a-zA-Z0-9_\-.]+$").unwrap().is_match(username) {
        return false;
    }

    return true;
}

pub async fn validate_session_token(session_token: &str) -> bool {
    if session_token.len() != 256 {
        return false;
    }

    if !regex::Regex::new(r"^[a-zA-Z0-9]+$").unwrap().is_match(session_token) {
        return false;
    }
    return true;
}
