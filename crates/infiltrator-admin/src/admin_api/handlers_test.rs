#[cfg(test)]
mod tests {
    use crate::admin_api::handlers::{
        compare_versions_desc, ensure_valid_profile_name, parse_version, sort_versions_desc,
    };

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("v1.18.0"), Some((1, 18, 0)));
        assert_eq!(parse_version("1.19.1"), Some((1, 19, 1)));
        assert_eq!(parse_version("v1.20.0-beta.1"), Some((1, 20, 0)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_compare_versions_desc() {
        use std::cmp::Ordering;
        assert_eq!(
            compare_versions_desc("v1.18.0", "v1.19.0"),
            Ordering::Greater
        );
        assert_eq!(compare_versions_desc("v1.20.0", "v1.19.0"), Ordering::Less);
        assert_eq!(compare_versions_desc("v1.18.0", "v1.18.0"), Ordering::Equal);
    }

    #[test]
    fn test_sort_versions_desc() {
        let mut versions = vec![
            "v1.18.0".to_string(),
            "v1.20.0".to_string(),
            "v1.19.5".to_string(),
        ];
        sort_versions_desc(&mut versions);
        assert_eq!(versions[0], "v1.20.0");
        assert_eq!(versions[1], "v1.19.5");
        assert_eq!(versions[2], "v1.18.0");
    }

    #[test]
    fn test_parse_incomplete_version() {
        assert_eq!(parse_version("v1"), Some((1, 0, 0)));
        assert_eq!(parse_version("v1.2"), Some((1, 2, 0)));
    }

    #[test]
    fn test_compare_versions_with_invalid() {
        use std::cmp::Ordering;
        // Invalid versions should be sorted after valid ones
        assert_eq!(
            compare_versions_desc("invalid", "v1.0.0"),
            Ordering::Greater
        );
        assert_eq!(compare_versions_desc("v1.0.0", "invalid"), Ordering::Less);
    }

    #[test]
    fn test_ensure_valid_profile_name_logic() {
        assert!(ensure_valid_profile_name("Valid-Name").is_ok());
        assert!(ensure_valid_profile_name("Invalid/Name").is_err());
        assert!(ensure_valid_profile_name("  ").is_err());
    }

    #[test]
    fn test_build_admin_url_edge_cases() {
        let mut versions = vec!["v1.18.0-alpha".into(), "v1.18.0-beta".into()];
        sort_versions_desc(&mut versions);
        assert_eq!(versions.len(), 2);
    }
}
