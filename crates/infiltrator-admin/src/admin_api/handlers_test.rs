#[cfg(test)]
mod tests {
    use crate::handlers::{parse_version, compare_versions_desc, sort_versions_desc};
    use std::cmp::Ordering;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("v1.18.0"), Some((1, 18, 0)));
        assert_eq!(parse_version("1.19.1"), Some((1, 19, 1)));
        assert_eq!(parse_version("v1.18.0-beta"), Some((1, 18, 0)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_compare_versions_desc() {
        assert_eq!(compare_versions_desc("v1.19.0", "v1.18.0"), Ordering::Less);
        assert_eq!(compare_versions_desc("v1.18.0", "v1.19.0"), Ordering::Greater);
        assert_eq!(compare_versions_desc("v1.18.0", "v1.18.0"), Ordering::Equal);
    }

    #[test]
    fn test_sort_versions_desc() {
        let mut versions = vec![
            "v1.18.0".to_string(),
            "v1.20.0".to_string(),
            "v1.19.0".to_string(),
        ];
        sort_versions_desc(&mut versions);
        assert_eq!(versions[0], "v1.20.0");
        assert_eq!(versions[1], "v1.19.0");
        assert_eq!(versions[2], "v1.18.0");
    }
}