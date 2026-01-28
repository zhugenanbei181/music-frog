#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_manager_default() {
        let manager = ProxyManager::default();
        assert_eq!(manager.client_url, String::new());
        assert_eq!(manager.secret, String::new());
    }

    #[test]
    fn test_proxy_manager_new() {
        let manager = ProxyManager::new(
            "http://127.0.0.1:9090".to_string(),
            "abc123".to_string(),
        );
        assert_eq!(manager.client_url, "http://127.0.0.1:9090");
        assert_eq!(manager.secret, "abc123");
    }

    #[test]
    fn test_proxy_manager_url_parsing() {
        let manager = ProxyManager::new(
            "http://127.0.0.1:9090".to_string(),
            "secret".to_string(),
        );

        // Test URL parsing
        let parsed = manager.parse_url().unwrap();
        assert_eq!(parsed.host(), "127.0.0.1");
        assert_eq!(parsed.port().unwrap(), 9090u16);
    }

    #[test]
    fn test_proxy_manager_validation() {
        // Test valid configurations
        assert!(ProxyManager::new(
            "http://127.0.0.1:9090".to_string(),
            "secret".to_string(),
        ).validate().is_ok());

        // Test invalid URLs
        let result = ProxyManager::new(
            "not-a-valid-url".to_string(),
            "secret".to_string(),
        ).validate();
        assert!(result.is_err());

        // Test empty secret
        let result = ProxyManager::new(
            "http://127.0.0.1:9090".to_string(),
            "".to_string(),
        ).validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_proxy_manager_serialization() {
        let manager = ProxyManager::new(
            "http://127.0.0.1:9090".to_string(),
            "secret456".to_string(),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&manager);
        assert!(json.contains("127.0.0.1:9090"));

        // Test deserialization
        let deserialized: ProxyManager = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.client_url, "http://127.0.0.1:9090");
        assert_eq!(deserialized.secret, "secret456");
    }
}
