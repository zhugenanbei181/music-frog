#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_manager_default() {
        let manager = ConnectionManager::default();
        assert_eq!(manager.url, String::new());
        assert_eq!(manager.secret, String::new());
    }

    #[test]
    fn test_connection_manager_new() {
        let manager = ConnectionManager::new(
            "http://localhost:9090".to_string(),
            "secret123".to_string(),
        );
        assert_eq!(manager.url, "http://localhost:9090");
        assert_eq!(manager.secret, "secret123");
    }

    #[test]
    fn test_connection_manager_url_validation() {
        // Test valid URLs
        assert!(ConnectionManager::new(
            "http://localhost:9090".to_string(),
            "secret".to_string(),
        ).validate().is_ok());

        assert!(ConnectionManager::new(
            "https://example.com:7890".to_string(),
            "secret".to_string(),
        ).validate().is_ok());

        // Test invalid URLs
        let result = ConnectionManager::new(
            "not-a-url".to_string(),
            "secret".to_string(),
        ).validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_connection_manager_serialization() {
        let manager = ConnectionManager::new(
            "http://localhost:9090".to_string(),
            "secret123".to_string(),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&manager);
        assert!(json.contains("localhost:9090"));

        // Test deserialization
        let deserialized: ConnectionManager = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.url, "http://localhost:9090");
        assert_eq!(deserialized.secret, "secret123");
    }
}
