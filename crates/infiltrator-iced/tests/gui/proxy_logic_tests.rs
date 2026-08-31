//! Proxy-page logic tests: filtering, delay sorting, runtime connection
//! sort/filter state and the runtime proxy selector sync/apply flow.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::types::message::Message;
use mihomo_api::proxy::types::{Proxy, ProxyBase, ProxyGroup, ProxyHistory};

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
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
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
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
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
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
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
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
            base: ProxyBase {
                name: "Proxy-A".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    proxies.insert(
        "Proxy-B".to_string(),
        Proxy::Shadowsocks(mihomo_api::proxy::types::Shadowsocks {
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
