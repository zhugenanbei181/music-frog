//! Demo-mode fixture and env-parser tests (mounted from `src/demo.rs`).
//! They exercise the exact fixture tables the visual-capture pipeline
//! renders, plus the INFILTRATOR_* environment contract.
//! test-intent: behavior

use super::*;

fn demo_env(page: Route) -> DemoEnv {
    DemoEnv {
        enabled: true,
        page,
        skin: iced::Theme::Dark,
        window_size: DEFAULT_WINDOW,
        capture_marker: None,
    }
}

#[test]
fn demo_fixture_inventory_covers_all_pages() {
    // The demo boot task is empty — no startup side effects are scheduled.
    let (state, _task) = AppState::demo(&demo_env(Route::Overview));

    // demo flag + no live runtime (all runtime guards no-op naturally)
    assert!(state.demo);
    assert!(state.runtime.is_none());

    // runtime status
    assert!(matches!(state.status, RuntimeStatus::Running));
    assert_eq!(state.proxy_mode.as_deref(), Some("rule"));
    assert!(state.system_proxy_enabled);
    assert_eq!(state.tun_enabled, Some(false));

    // proxies: >0 groups & profiles (hard requirement)
    assert!(state.proxies.values().filter(|p| p.is_group()).count() >= 6);
    assert!(!state.filtered_groups.is_empty());
    assert!(!state.proxies.is_empty());
    assert!(state.profiles.len() >= 2);
    assert!(state.profiles.iter().any(|p| p.active));

    // traffic / memory / connections / logs
    assert_eq!(state.traffic_history.len(), 60);
    assert!(state.traffic.is_some());
    assert!(state.memory.is_some());
    assert_eq!(
        state.connections.as_ref().map(|c| c.connections.len()),
        Some(10)
    );
    assert!(state.logs.len() >= 40);

    // rules / dns / misc
    assert_eq!(state.rules.len(), 15);
    assert!(!state.rules_render_cache.is_empty());
    assert!(!state.rules_filtered_indices.is_empty());
    assert!(!state.dns_nameservers.is_empty());
    assert!(state.dns_form.enable);
    assert!(state.fake_ip_form.store_fake_ip);
    assert!(!state.tun_form.stack.is_empty());
    assert!(state.installed_kernels.iter().any(|k| k.is_default));
    assert_eq!(state.admin_port, crate::admin_server::ADMIN_DEFAULT_PORT);
    assert!(state.toasts.is_empty());
    assert_eq!(state.lang, "zh-CN");
}

#[test]
fn demo_connections_populate_every_field_the_runtime_view_renders() {
    // The runtime view reads exactly `state.connections` (same field the
    // live update path fills via `Message::ConnectionsReceived`). Every
    // per-row field it renders must be populated, otherwise rows paint as
    // blank cards even though the snapshot is `Some`.
    let (state, _task) = AppState::demo(&demo_env(Route::Runtime));
    let snapshot = state.connections.as_ref().expect("demo seeds connections");
    assert!(snapshot.connections.len() >= 10);

    for conn in &snapshot.connections {
        assert!(!conn.id.is_empty(), "id drives the close button");
        assert!(
            !conn.metadata.host.is_empty() || !conn.metadata.destination_ip.is_empty(),
            "row headline needs a host or destination IP"
        );
        assert!(!conn.metadata.network.is_empty(), "network chip");
        assert!(!conn.metadata.destination_port.is_empty(), "host:port mono");
        assert!(!conn.metadata.source_ip.is_empty(), "source mono");
        assert!(!conn.metadata.source_port.is_empty(), "source mono");
        assert!(!conn.rule.is_empty(), "rule badge");
        assert!(!conn.rule_payload.is_empty(), "rule badge payload");
        assert!(conn.upload > 0, "upload bytes");
        assert!(conn.download > 0, "download bytes");
        assert!(!conn.start.is_empty(), "latest-first sort key");
    }
}

#[test]
fn demo_state_reflects_requested_page_and_skin() {
    for page in [
        Route::Overview,
        Route::Proxies,
        Route::Runtime,
        Route::Rules,
        Route::Dns,
        Route::Profiles,
        Route::Sync,
        Route::Editor,
        Route::Settings,
    ] {
        let mut env = demo_env(page);
        env.skin = iced::Theme::Light;
        let (state, _) = AppState::demo(&env);
        assert_eq!(state.current_route, page);
        assert_eq!(state.theme, iced::Theme::Light);
    }
}

#[test]
fn capture_marker_is_idempotent_and_formats_the_contract_line() {
    let path = std::env::temp_dir().join(format!(
        "infiltrator_demo_marker_{}_{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_file(&path);

    let mut env = demo_env(Route::Proxies);
    env.capture_marker = Some(path.clone());
    let (state, _) = AppState::demo(&env);

    // Fire twice — the file must end up with exactly one contract line.
    state.write_capture_marker();
    state.write_capture_marker();

    let content = std::fs::read_to_string(&path).expect("marker file exists");
    assert_eq!(content, "CAPTURE_READY page=proxies skin=dark\n");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn capture_marker_without_path_is_a_noop() {
    let (state, _) = AppState::demo(&demo_env(Route::Overview));
    // Must not panic and must consume the once-flag without a path.
    state.write_capture_marker();
    assert!(state.capture_marker.is_none());
}

#[test]
fn env_page_mapping_is_exhaustive() {
    let cases = [
        ("overview", Route::Overview),
        ("proxies", Route::Proxies),
        ("runtime", Route::Runtime),
        ("rules", Route::Rules),
        ("dns", Route::Dns),
        ("profiles", Route::Profiles),
        ("sync", Route::Sync),
        ("editor", Route::Editor),
        ("settings", Route::Settings),
    ];
    for (name, route) in cases {
        assert_eq!(parse_page(name), Some(route));
        assert_eq!(parse_page(&name.to_uppercase()), Some(route));
        assert_eq!(route_env_name(route), name);
    }
    assert_eq!(parse_page("nope"), None);
    assert_eq!(parse_page(""), None);
}

#[test]
fn env_skin_and_window_size_parsers_fall_back_safely() {
    assert_eq!(parse_skin("light"), iced::Theme::Light);
    assert_eq!(parse_skin("LIGHT"), iced::Theme::Light);
    assert_eq!(parse_skin("dark"), iced::Theme::Dark);
    assert_eq!(parse_skin("bogus"), iced::Theme::Dark);

    assert_eq!(parse_window_size("1280x800"), (1280.0, 800.0));
    assert_eq!(parse_window_size("1440X900"), (1440.0, 900.0));
    assert_eq!(parse_window_size("bogus"), DEFAULT_WINDOW);
    assert_eq!(parse_window_size("0x100"), DEFAULT_WINDOW);
    assert_eq!(parse_window_size("-10x100"), DEFAULT_WINDOW);
}
