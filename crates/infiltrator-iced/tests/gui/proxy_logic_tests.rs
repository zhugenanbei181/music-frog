//! Proxy-page logic tests: filtering, delay sorting, runtime connection
//! sort/filter state and the runtime proxy selector sync/apply flow.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_domain::proxy::{Proxy, ProxyBase, ProxyGroup, ProxyHistory};

#[test]
fn test_proxy_filtering_and_sorting() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    // Group GLOBAL
    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "Proxy-A".to_string(),
            all: vec!["Proxy-A".into(), "Proxy-B".into(), "Special".into()],
            history: vec![],
        }),
    );

    // Node A (100ms)
    proxies.insert(
        "Proxy-A".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-A".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 100,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node B (50ms)
    proxies.insert(
        "Proxy-B".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-B".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 50,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node Special (200ms)
    proxies.insert(
        "Special".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Special".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 200,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    state.runtime.proxies = proxies;

    // Test Search
    let _ = state.update(Message::FilterProxies("special".into()));
    assert_eq!(state.runtime.proxy_filter, "special");

    // Test Sort Toggle
    let _ = state.update(Message::ToggleProxySort);
    assert!(state.runtime.proxy_sort_by_delay);

    // Verification of logic (manual check of sorting logic used in view)
    let global = state.runtime.proxies.get("GLOBAL").unwrap();
    let mut members = global.all().unwrap().to_vec();

    // Apply filter
    let filter = state.runtime.proxy_filter.to_lowercase();
    members.retain(|m| m.to_lowercase().contains(&filter));
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "Special");

    // Apply sort (on all members)
    let mut all_members = global.all().unwrap().to_vec();
    all_members.sort_by_key(|m| {
        state
            .runtime
            .proxies
            .get(m)
            .and_then(|p| p.history().last().map(|h| h.delay))
            .unwrap_or(u32::MAX)
    });

    assert_eq!(all_members[0], "Proxy-B"); // 50ms
    assert_eq!(all_members[1], "Proxy-A"); // 100ms
    assert_eq!(all_members[2], "Special"); // 200ms
}

#[test]
fn test_runtime_auto_refresh_toggle() {
    let (mut state, _) = AppState::new();
    assert!(state.runtime.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(false));
    assert!(!state.runtime.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(true));
    assert!(state.runtime.runtime_auto_refresh);
}

#[test]
fn test_runtime_connection_sort_mode_switch() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.runtime.runtime_connection_sort, "download_desc");

    let _ = state.update(Message::UpdateRuntimeConnectionSort("upload_desc".into()));
    assert_eq!(state.runtime.runtime_connection_sort, "upload_desc");

    let _ = state.update(Message::UpdateRuntimeConnectionSort("invalid_key".into()));
    assert_eq!(state.runtime.runtime_connection_sort, "download_desc");
}

#[test]
fn test_proxy_delay_sort_mode_switch() {
    let (mut state, _) = AppState::new();

    let _ = state.update(Message::UpdateProxyDelaySort("name_desc".into()));
    assert_eq!(state.runtime.proxy_delay_sort, "name_desc");
    assert!(!state.runtime.proxy_sort_by_delay);

    let _ = state.update(Message::UpdateProxyDelaySort("delay_desc".into()));
    assert_eq!(state.runtime.proxy_delay_sort, "delay_desc");
    assert!(state.runtime.proxy_sort_by_delay);
}

#[test]
fn test_profiles_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateProfilesFilter("default".into()));
    assert_eq!(state.profile.profiles_filter, "default");
}

#[test]
fn test_runtime_connection_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("api".into()));
    assert_eq!(state.runtime.runtime_connection_filter, "api");
}

#[test]
fn test_runtime_proxy_selector_sync_and_apply() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "Proxy-A".to_string(),
            all: vec!["Proxy-A".into(), "Proxy-B".into()],
            history: vec![],
        }),
    );
    proxies.insert(
        "Proxy-A".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-A".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    proxies.insert(
        "Proxy-B".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-B".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    let _ = state.update(Message::ProxiesLoaded(Ok(proxies)));
    assert_eq!(state.runtime.runtime_selected_group, "GLOBAL");
    assert_eq!(state.runtime.runtime_selected_proxy, "Proxy-A");

    let _ = state.update(Message::UpdateRuntimeSelectedProxy("Proxy-B".into()));
    assert_eq!(state.runtime.runtime_selected_proxy, "Proxy-B");

    let _ = state.update(Message::ApplyRuntimeSelectedProxy);
}

#[test]
fn test_filter_alive_and_favorite_pinning() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    // Group GLOBAL with 3 nodes
    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "Node-Alive-1".to_string(),
            all: vec![
                "Node-Alive-1".into(),
                "Node-Dead".into(),
                "Node-Alive-2".into(),
            ],
            history: vec![],
        }),
    );

    // Node-Alive-1 (120ms)
    proxies.insert(
        "Node-Alive-1".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Node-Alive-1".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 120,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node-Dead (0ms / no delay)
    proxies.insert(
        "Node-Dead".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Node-Dead".to_string(),
                history: vec![],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    // Node-Alive-2 (50ms)
    proxies.insert(
        "Node-Alive-2".to_string(),
        Proxy::Shadowsocks(infiltrator_domain::proxy::Shadowsocks {
            base: ProxyBase {
                name: "Node-Alive-2".to_string(),
                history: vec![ProxyHistory {
                    time: "".into(),
                    delay: 50,
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
    );

    state.runtime.proxies = proxies;
    state.recompute_filtered_groups();
    assert_eq!(state.runtime.filtered_groups.len(), 1);
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), 3);
    assert_eq!(members[0], "Node-Alive-2"); // 50ms lowest
    assert_eq!(members[1], "Node-Alive-1"); // 120ms
    assert_eq!(members[2], "Node-Dead");

    // Test Filter Alive
    let _ = state.update(Message::ToggleFilterAlive(true));
    state.recompute_filtered_groups();
    assert!(state.runtime.filter_alive_only);
    let alive_members = &state.runtime.filtered_groups[0].1;
    assert_eq!(alive_members.len(), 2);
    assert!(!alive_members.contains(&"Node-Dead".to_string()));

    // Test Favorite Pinning: Favorite Node-Alive-1 (even though it has higher latency than Node-Alive-2)
    let _ = state.update(Message::ToggleFavoriteProxy("Node-Alive-1".into()));
    state.recompute_filtered_groups();
    assert!(state.runtime.favorite_proxies.contains("Node-Alive-1"));
    let fav_members = &state.runtime.filtered_groups[0].1;
    assert_eq!(fav_members[0], "Node-Alive-1"); // Pinned to top!
    assert_eq!(fav_members[1], "Node-Alive-2");

    // Test Inspect Proxy and Compact View
    let _ = state.update(Message::InspectProxy(Some("Node-Alive-1".into())));
    assert_eq!(
        state.runtime.inspecting_proxy.as_deref(),
        Some("Node-Alive-1")
    );
    let _ = state.update(Message::InspectProxy(None));
    assert!(state.runtime.inspecting_proxy.is_none());

    let _ = state.update(Message::ToggleProxyCompactView);
    assert!(state.runtime.proxy_compact_view);
}

#[test]
fn test_pinyin_fuzzy_filter_in_gui() {
    let (mut state, _) = AppState::new();
    let mut proxies = std::collections::HashMap::new();

    proxies.insert(
        "GLOBAL".to_string(),
        Proxy::Selector(ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "香港 IEPL-01".to_string(),
            all: vec![
                "香港 IEPL-01".into(),
                "日本 Tokyo-01".into(),
                "美国 GIA-01".into(),
            ],
            history: vec![],
        }),
    );

    state.runtime.proxies = proxies;

    // Search with pinyin initials "xg" -> should match 香港 IEPL-01
    let _ = state.update(Message::FilterProxies("xg".into()));
    state.recompute_filtered_groups();
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "香港 IEPL-01");

    // Search with country code "jp" -> should match 日本 Tokyo-01
    let _ = state.update(Message::FilterProxies("jp".into()));
    state.recompute_filtered_groups();
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "日本 Tokyo-01");
}

#[test]
fn test_custom_node_modal_interactions() {
    let (mut state, _) = AppState::new();
    assert!(!state.runtime.is_adding_custom_node);

    let _ = state.update(Message::OpenAddCustomNodeModal(true));
    assert!(state.runtime.is_adding_custom_node);

    let _ = state.update(Message::UpdateNewNodeType("vless".into()));
    assert_eq!(state.runtime.new_node_type, "vless");

    let _ = state.update(Message::UpdateNewNodeName("US-Fast-01".into()));
    assert_eq!(state.runtime.new_node_name, "US-Fast-01");

    let _ = state.update(Message::UpdateNewNodeServer("1.2.3.4".into()));
    assert_eq!(state.runtime.new_node_server, "1.2.3.4");

    let _ = state.update(Message::UpdateNewNodePort("443".into()));
    assert_eq!(state.runtime.new_node_port, "443");

    let _ = state.update(Message::UpdateNewNodeCredential("a1b2c3d4-uuid".into()));
    assert_eq!(state.runtime.new_node_credential, "a1b2c3d4-uuid");

    let _ = state.update(Message::UpdateNewNodeCipher(
        "chacha20-ietf-poly1305".into(),
    ));
    assert_eq!(state.runtime.new_node_cipher, "chacha20-ietf-poly1305");

    let _ = state.update(Message::UpdateNewNodeTls(true));
    assert!(state.runtime.new_node_tls);

    let _ = state.update(Message::OpenAddCustomNodeModal(false));
    assert!(!state.runtime.is_adding_custom_node);
}
