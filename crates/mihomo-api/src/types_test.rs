#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_info_default() {
        let info = ProxyInfo::default();
        assert_eq!(info.proxy_type, None);
        assert_eq!(info.all, None);
        assert_eq!(info.now, None);
        assert_eq!(info.history, vec![]);
        assert_eq!(info.udp, false);
        assert_eq!(info.xudp, false);
    }

    #[test]
    fn test_proxy_info_new() {
        let info = ProxyInfo::new(
            Some("Selector".to_string()),
            Some(vec!["node1".to_string(), "node2".to_string()]),
            Some("node1".to_string()),
            false,
            false,
        );
        assert_eq!(info.proxy_type, Some("Selector".to_string()));
        assert_eq!(info.all, Some(vec!["node1".to_string(), "node2".to_string()]));
        assert_eq!(info.now, Some("node1".to_string()));
        assert_eq!(info.udp, false);
        assert_eq!(info.xudp, false);
    }

    #[test]
    fn test_history_entry_default() {
        let entry = HistoryEntry::default();
        assert_eq!(entry.time, chrono::Utc.timestamp(0, 0));
        assert_eq!(entry.delay, None);
    }

    #[test]
    fn test_history_entry_new() {
        let time = chrono::Utc::now();
        let entry = HistoryEntry::new(time, Some(150));
        assert_eq!(entry.time, time);
        assert_eq!(entry.delay, Some(150));
    }

    #[test]
    fn test_proxy_types() {
        assert_eq!("Selector", "Selector");
        assert_eq!("URLTest", "URLTest");
        assert_eq!("Fallback", "Fallback");
        assert_eq!("LoadBalance", "LoadBalance");
        assert_eq!("Direct", "Direct");
        assert_eq!("Reject", "Reject");
    }

    #[test]
    fn test_proxy_info_serialization() {
        let info = ProxyInfo::new(
            Some("Selector".to_string()),
            Some(vec!["node1".to_string(), "node2".to_string()]),
            Some("node1".to_string()),
            false,
            false,
        );

        // Test JSON serialization
        let json = serde_json::to_string(&info);
        assert!(json.contains("Selector"));

        // Test deserialization
        let deserialized: ProxyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.proxy_type, Some("Selector".to_string()));
        assert_eq!(deserialized.now, Some("node1".to_string()));
    }
}
