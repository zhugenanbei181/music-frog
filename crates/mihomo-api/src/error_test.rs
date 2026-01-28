#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mihomo_error_display() {
        let err = MihomoError::NotFound("test not found".to_string());
        assert_eq!(format!("{}", err), "test not found");

        let err = MihomoError::Config("invalid config".to_string());
        assert_eq!(format!("{}", err), "invalid config");

        let err = MihomoError::Network("network error".to_string());
        assert_eq!(format!("{}", err), "network error");
    }

    #[test]
    fn test_mihomo_error_from_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let mihomo_err = MihomoError::from(io_err);
        assert!(matches!(mihomo_err, MihomoError::NotFound(_)));
    }

    #[test]
    fn test_result_type() {
        // Test Ok result
        let ok_result: Result<String> = Ok("success".to_string());
        assert!(ok_result.is_ok());
        assert_eq!(ok_result.unwrap(), "success");

        // Test Err result
        let err_result: Result<String> = Err(MihomoError::Network("error".to_string()));
        assert!(err_result.is_err());
        assert!(err_result.is_err_and_then(|e| Some(e.to_string())).is_some());
    }

    #[test]
    fn test_error_variants() {
        // Test all error variants can be created
        let _ = MihomoError::NotFound("test".to_string());
        let _ = MihomoError::Config("test".to_string());
        let _ = MihomoError::Network("test".to_string());
        let _ = MihomoError::Parse("test".to_string());
        let _ = MihomoError::Timeout("test".to_string());
        let _ = MihomoError::Validation("test".to_string());
    }

    #[test]
    fn test_error_serialization() {
        let err = MihomoError::NotFound("test not found".to_string());

        // Test JSON serialization
        let json = serde_json::to_string(&err);
        assert!(json.contains("NotFound"));

        // Test deserialization
        let deserialized: MihomoError = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, MihomoError::NotFound(_)));
    }
}
