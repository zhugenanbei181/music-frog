#[cfg(test)]
mod tests {
    use crate::tray::menu::*;
    use mihomo_api::{ProxyInfo, DelayHistory};

    #[tokio::test]
    async fn test_build_menu_id() {
        let id1 = build_menu_id("test", "key1");
        let id2 = build_menu_id("test", "key2");
        let id1_again = build_menu_id("test", "key1");

        // IDs should be consistent for the same key
        assert_eq!(id1, id1_again);
        // IDs should be different for different keys
        assert_ne!(id1, id2);
        // IDs should be hex format
        assert!(id1.starts_with("test-"));
        assert!(id1.len() > "test-".len());
    }

    #[tokio::test]
    async fn test_insert_profile_menu_id() {
        let mut profile_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        let id1 = insert_profile_menu_id(&mut profile_map, "profile1");
        let id2 = insert_profile_menu_id(&mut profile_map, "profile2");
        let id3 = insert_profile_menu_id(&mut profile_map, "profile1");

        // Should have 3 entries (2 for profile1 with different suffixes, 1 for profile2)
        assert_eq!(profile_map.len(), 3);

        // Check that profile names are correctly mapped
        assert_eq!(profile_map.get(&id1), Some(&"profile1".to_string()));
        assert_eq!(profile_map.get(&id2), Some(&"profile2".to_string()));
        assert_eq!(profile_map.get(&id3), Some(&"profile1".to_string()));

        // ID3 should have a different suffix from ID1 due to collision
        assert_ne!(id1, id3);
    }

    #[tokio::test]
    async fn test_insert_proxy_menu_id() {
        let mut proxy_map: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();

        let id1 = insert_proxy_menu_id(&mut proxy_map, "group1", "node1");
        let id2 = insert_proxy_menu_id(&mut proxy_map, "group2", "node1");
        let id3 = insert_proxy_menu_id(&mut proxy_map, "group1", "node2");
        let id4 = insert_proxy_menu_id(&mut proxy_map, "group1", "node1");

        // Should have 4 entries
        assert_eq!(proxy_map.len(), 4);

        // Check that group/node names are correctly mapped
        assert_eq!(
            proxy_map.get(&id1),
            Some(&("group1".to_string(), "node1".to_string()))
        );
        assert_eq!(
            proxy_map.get(&id2),
            Some(&("group2".to_string(), "node1".to_string()))
        );
        assert_eq!(
            proxy_map.get(&id3),
            Some(&("group1".to_string(), "node2".to_string()))
        );
        assert_eq!(
            proxy_map.get(&id4),
            Some(&("group1".to_string(), "node1".to_string()))
        );

        // ID4 should have a different suffix from ID1 due to collision
        assert_ne!(id1, id4);
    }

    #[test]
    fn test_truncate_label() {
        // Test with short label (no truncation)
        assert_eq!(truncate_label("short", 10), "short");

        // Test with exact length
        assert_eq!(truncate_label("exactly10", 10), "exactly10");

        // Test with truncation
        assert_eq!(truncate_label("very long label", 10), "very lo...");

        // Test with very short max
        assert_eq!(truncate_label("long", 0), "");
        assert_eq!(truncate_label("long", 1), "...");
        assert_eq!(truncate_label("long", 2), "...");
        assert_eq!(truncate_label("long", 3), "...");
    }

    #[test]
    fn test_is_selectable_group() {
        let selector_info = ProxyInfo {
            proxy_type: "Selector".to_string(),
            all: None,
            now: None,
            history: vec![],
        };

        let fallback_info = ProxyInfo {
            proxy_type: "Fallback".to_string(),
            all: None,
            now: None,
            history: vec![],
        };

        let load_balance_info = ProxyInfo {
            proxy_type: "LoadBalance".to_string(),
            all: None,
            now: None,
            history: vec![],
        };

        let url_test_info = ProxyInfo {
            proxy_type: "URLTest".to_string(),
            all: None,
            now: None,
            history: vec![],
        };

        let direct_info = ProxyInfo {
            proxy_type: "Direct".to_string(),
            all: None,
            now: None,
            history: vec![],
        };

        assert!(is_selectable_group(&selector_info));
        assert!(is_selectable_group(&fallback_info));
        assert!(is_selectable_group(&load_balance_info));
        assert!(is_selectable_group(&url_test_info));
        assert!(!is_selectable_group(&direct_info));
    }

    #[test]
    fn test_is_script_enabled() {
        // Test with script enabled
        let script_enabled = serde_json::json!({"enable": true});
        assert!(is_script_enabled(Some(&script_enabled)));

        // Test with script disabled
        let script_disabled = serde_json::json!({"enable": false});
        assert!(!is_script_enabled(Some(&script_disabled)));

        // Test with script missing enable field (default true)
        let script_default = serde_json::json!({"path": "/path/to/script"});
        assert!(is_script_enabled(Some(&script_default)));

        // Test with no script
        assert!(!is_script_enabled(None));
    }

    #[test]
    fn test_build_proxy_node_label() {
        let mut proxies = std::collections::HashMap::new();

        // Proxy with delay
        let node1 = "node1".to_string();
        let mut info1 = ProxyInfo {
            proxy_type: "Shadowsocks".to_string(),
            all: None,
            now: None,
            history: vec![],
        };
        info1.history.push(DelayHistory {
            time: "2024-01-01T00:00:00Z".to_string(),
            delay: 150,
        });
        proxies.insert(node1.clone(), info1);

        // Proxy without delay
        let node2 = "node2".to_string();
        let info2 = ProxyInfo {
            proxy_type: "Shadowsocks".to_string(),
            all: None,
            now: None,
            history: vec![],
        };
        proxies.insert(node2.clone(), info2);

        // Test with delay
        let label1 = build_proxy_node_label(&proxies, &node1);
        assert_eq!(label1, "node1 (150ms)");

        // Test without delay
        let label2 = build_proxy_node_label(&proxies, &node2);
        assert_eq!(label2, "node2");

        // Test with non-existent node
        let label3 = build_proxy_node_label(&proxies, "nonexistent");
        assert_eq!(label3, "nonexistent");
    }
}