    use super::*;

    #[test]
    fn test_normalize_mode() {
        assert_eq!(normalize_mode("rule"), "rule");
        assert_eq!(normalize_mode("RULE"), "rule");
        assert_eq!(normalize_mode("Rule"), "rule");
        assert_eq!(normalize_mode("  rule  "), "rule");
    }

    #[test]
    fn test_normalize_mode_empty() {
        assert_eq!(normalize_mode(""), "rule");
        assert_eq!(normalize_mode("   "), "rule");
    }

    #[test]
    fn test_normalize_mode_other_modes() {
        assert_eq!(normalize_mode("global"), "global");
        assert_eq!(normalize_mode("direct"), "direct");
        assert_eq!(normalize_mode("script"), "script");
    }

    #[test]
    fn test_is_supported_mode() {
        assert!(is_supported_mode("rule"));
        assert!(is_supported_mode("global"));
        assert!(is_supported_mode("direct"));
        assert!(is_supported_mode("script"));
    }

    #[test]
    fn test_is_supported_mode_invalid() {
        assert!(!is_supported_mode("invalid"));
        assert!(!is_supported_mode(""));
        assert!(!is_supported_mode("other"));
    }

    #[test]
    fn test_parse_yaml_doc_success() {
        let yaml = r#"
port: 7890
mode: rule
"#;
        let result = parse_yaml_doc(yaml);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.is_hash());
    }

    #[test]
    fn test_parse_yaml_doc_invalid() {
        let yaml = "invalid: yaml: [";
        let result = parse_yaml_doc(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_yaml_doc_empty() {
        let yaml = "";
        let result = parse_yaml_doc(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_yaml_str_success() {
        let yaml = r#"
port: 7890
mode: rule
"#;
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "mode");
        assert_eq!(result, Some("rule"));
    }

    #[test]
    fn test_get_yaml_str_not_found() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "mode");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_yaml_str_invalid_type() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_str(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_yaml_u16_integer() {
        let yaml = "port: 7890";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_yaml_u16_string() {
        let yaml = "port: \"7890\"";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_yaml_u16_not_found() {
        let yaml = "mode: rule";
        let doc = parse_yaml_doc(yaml).unwrap();
        let result = get_yaml_u16(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_proxy_endpoint_uses_configured_non_default_port() {
        assert_eq!(
            proxy_endpoint_from_config("mixed-port: 18080\nport: 7890\n").unwrap(),
            Some("127.0.0.1:18080".to_string())
        );
        assert_eq!(
            proxy_endpoint_from_config("port: 19090\n").unwrap(),
            Some("127.0.0.1:19090".to_string())
        );
        assert_eq!(proxy_endpoint_from_config("mixed-port: 0\n").unwrap(), None);
    }

    #[test]
    fn test_collect_geoip_candidates() {
        let binary = PathBuf::from("/path/to/mihomo.exe");
        let bundled = vec![
            PathBuf::from("/path/to/bundled1/mihomo.exe"),
            PathBuf::from("/other/path/mihomo.exe"),
        ];

        let result = collect_geoip_candidates(&binary, &bundled);

        assert_eq!(result.len(), 3);
        assert!(result.contains(&PathBuf::from("/path/to/geoip.metadb")));
        assert!(result.contains(&PathBuf::from("/path/to/bundled1/geoip.metadb")));
        assert!(result.contains(&PathBuf::from("/other/path/geoip.metadb")));
    }

    #[test]
    fn test_collect_geoip_candidates_empty() {
        let binary = PathBuf::from("/path/to/mihomo.exe");
        let bundled: Vec<PathBuf> = vec![];

        let result = collect_geoip_candidates(&binary, &bundled);

        assert_eq!(result.len(), 1);
        assert!(result.contains(&PathBuf::from("/path/to/geoip.metadb")));
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_build_geoip_url_list_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("github.com"));
        assert!(result[1].contains("jsdelivr.net"));
    }

    #[test]
    fn test_build_geoip_url_list_custom() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("MIHOMO_GEOIP_URL", "https://custom.url/geoip.metadb") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "https://custom.url/geoip.metadb");
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
    }

    #[test]
    fn test_build_geoip_url_list_empty_custom() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("MIHOMO_GEOIP_URL", "   ") };
        let result = build_geoip_url_list();
        assert_eq!(result.len(), 3);
        unsafe { std::env::remove_var("MIHOMO_GEOIP_URL") };
    }

    #[test]
    fn test_mihomo_summary_serialization() {
        let summary = MihomoSummary {
            profile: "default".to_string(),
            mode: "rule".to_string(),
            running: true,
            controller: "http://127.0.0.1:9090".to_string(),
            groups: vec![],
        };

        let serialized = serde_json::to_string(&summary);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_mihomo_summary_debug() {
        let summary = MihomoSummary {
            profile: "default".to_string(),
            mode: "rule".to_string(),
            running: true,
            controller: "http://127.0.0.1:9090".to_string(),
            groups: vec![],
        };

        let debug_str = format!("{:?}", summary);
        assert!(debug_str.contains("default"));
        assert!(debug_str.contains("rule"));
    }
