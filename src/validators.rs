pub fn validate_username(username: &str) -> bool {
    if username.len() < 3 || username.len() > 18 {
        return false;
    }

    if !regex::Regex::new(r"^[a-zA-Z0-9_\-.]+$").unwrap().is_match(username) {
        return false;
    }

    return true;
}

pub fn validate_session_token(session_token: &str) -> bool {
    if session_token.len() != 256 {
        return false;
    }

    if !regex::Regex::new(r"^[a-zA-Z0-9]+$").unwrap().is_match(session_token) {
        return false;
    }
    return true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_username() {
        assert!(validate_username("testuser"));
        assert!(validate_username("testuser123"));
        assert!(validate_username("test-user"));
        assert!(validate_username("test_user"));
        assert!(validate_username("test.user"));
        assert!(validate_username("test-user123"));
        assert!(validate_username("test_user123"));
        assert!(validate_username("test.user123"));
        assert!(validate_username("test-user1234"));
        assert!(validate_username("test_user1234"));
        assert!(validate_username("test.user1234"));
        assert!(validate_username("test-user12345"));
        assert!(validate_username("test_user12345"));
        assert!(validate_username("test.user12345"));
        assert!(validate_username("test-user123456"));
        assert!(validate_username("test_user123456"));
        assert!(validate_username("test.user123456"));
        assert!(validate_username("test-user1234567"));
        assert!(validate_username("test_user1234567"));
        assert!(validate_username("test.user1234567"));
        assert!(validate_username("test-user12345678"));
        assert!(validate_username("test_user12345678"));
        assert!(validate_username("test.user12345678"));
        assert!(validate_username("test-user123456789"));
        assert!(validate_username("test_user123456789"));
        assert!(validate_username("test.user123456789"));
        assert!(!validate_username("test-user1234567890"));
        assert!(!validate_username("test_user1234567890"));
        assert!(!validate_username("test.user1234567890"));
        assert!(!validate_username("test-user12345678901"));
        assert!(!validate_username("test_user12345678901"));
    }
}
