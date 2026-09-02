//! Demo-mode fixture and env-parser tests (mounted from `src/demo.rs`).
//! They exercise the exact fixture tables the visual-capture pipeline
//! renders, plus the INFILTRATOR_* environment contract.
//! test-intent: behavior

use super::*;
use crate::types::runtime::RuntimeStatus;

fn demo_env(page: Route) -> DemoEnv {
    DemoEnv {
        enabled: true,
        page,
        pane: crate::types::options::EditorPane::Profile,
        providers_tab: false,
        lang: "zh-CN".to_string(),
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
    assert!(state.shell.demo);
    assert!(state.runtime.runtime.is_none());

    // runtime status
    assert!(matches!(state.runtime.status, RuntimeStatus::Running));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("rule"));
    assert!(state.runtime.system_proxy_enabled);
    assert_eq!(state.runtime.tun_enabled, Some(false));

    // proxies: >0 groups & profiles (hard requirement)
    assert!(
        state
            .runtime
            .proxies
            .values()
            .filter(|p| p.is_group())
            .count()
            >= 6
    );
    assert!(!state.runtime.filtered_groups.is_empty());
    assert!(!state.runtime.proxies.is_empty());
    assert!(state.profile.profiles.len() >= 2);
    assert!(state.profile.profiles.iter().any(|p| p.active));

    // traffic / memory / connections / logs
    assert_eq!(state.diag.traffic_history.len(), 60);
    assert!(state.diag.traffic.is_some());
    assert!(state.diag.memory.is_some());
    assert_eq!(
        state.diag.connections.as_ref().map(|c| c.connections.len()),
        Some(10)
    );
    assert!(state.diag.logs.len() >= 40);

    // rules / dns / misc
    assert_eq!(state.editor.rules.len(), 15);
    assert!(!state.editor.rules_render_cache.is_empty());
    assert!(!state.editor.rules_filtered_indices.is_empty());
    assert!(!state.editor.dns_nameservers.is_empty());
    assert!(state.editor.dns_form.enable);
    assert!(state.editor.fake_ip_form.store_fake_ip);
    assert!(!state.editor.tun_form.stack.is_empty());
    assert!(state.runtime.installed_kernels.iter().any(|k| k.is_default));
    assert_eq!(
        state.shell.admin_port,
        crate::admin_server::ADMIN_DEFAULT_PORT
    );
    assert!(state.shell.toasts.is_empty());
    assert_eq!(state.shell.lang, "zh-CN");
}

#[test]
fn demo_connections_populate_every_field_the_runtime_view_renders() {
    // The runtime view reads exactly `state.diag.connections` (same field the
    // live update path fills via `Message::ConnectionsReceived`). Every
    // per-row field it renders must be populated, otherwise rows paint as
    // blank cards even though the snapshot is `Some`.
    let (state, _task) = AppState::demo(&demo_env(Route::Runtime));
    let snapshot = state
        .diag
        .connections
        .as_ref()
        .expect("demo seeds connections");
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
        assert_eq!(state.shell.current_route, page);
        assert_eq!(state.shell.theme, iced::Theme::Light);
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
    assert!(state.shell.capture_marker.is_none());
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
        ("mixin", Route::Editor),
        ("settings", Route::Settings),
    ];
    for (name, route) in cases {
        assert_eq!(parse_page(name), Some(route));
        assert_eq!(parse_page(&name.to_uppercase()), Some(route));
        if name != "mixin" {
            assert_eq!(route_env_name(route), name);
        }
    }
    assert_eq!(parse_page("nope"), None);
    assert_eq!(parse_page(""), None);
}

#[test]
fn demo_filter_page_activates_the_filter_pane_with_seeded_draft() {
    let mut env = demo_env(Route::Editor);
    env.pane = crate::types::options::EditorPane::Filter;
    let (state, _) = AppState::demo(&env);
    assert_eq!(
        state.editor.editor_pane,
        crate::types::options::EditorPane::Filter
    );
    assert_eq!(state.editor.filter_loaded_for.as_deref(), Some("机场订阅"));
    assert!(!state.editor.filter_draft.include.is_empty());
    // Subscription fixture carries userinfo traffic for the usage bar.
    let active = state
        .profile
        .profiles
        .iter()
        .find(|p| p.active)
        .expect("demo has an active profile");
    assert!(active.traffic_total.unwrap_or(0) > active.traffic_upload.unwrap_or(0));
}

#[test]
fn demo_mixin_page_activates_the_mixin_pane_with_fixture_document() {
    let mut env = demo_env(Route::Editor);
    env.pane = crate::types::options::EditorPane::Mixin;
    let (state, _) = AppState::demo(&env);
    assert_eq!(
        state.editor.editor_pane,
        crate::types::options::EditorPane::Mixin
    );
    assert_eq!(state.editor.mixin_loaded_for.as_deref(), Some("机场订阅"));
    assert!(state.editor.mixin_content.text().contains("rules:"));
    assert!(state.editor.mixin_content.text().contains("prepend:"));
}

#[test]
fn env_skin_and_window_size_parsers_fall_back_safely() {
    assert_eq!(parse_skin("light"), iced::Theme::Light);
    assert_eq!(parse_skin("LIGHT"), iced::Theme::Light);
    assert_eq!(parse_skin("dark"), iced::Theme::Dark);
    assert_eq!(parse_skin("bogus"), iced::Theme::Dark);
    assert_eq!(parse_skin("forest"), crate::view::theme::forest_theme());
    assert_eq!(parse_skin("EyeForest"), crate::view::theme::forest_theme());
    assert_eq!(parse_skin("eye-forest"), crate::view::theme::forest_theme());
    assert_eq!(
        crate::view::theme::theme_to_name(&crate::view::theme::forest_theme()),
        "forest"
    );
    assert!(crate::view::theme::is_forest(
        &crate::view::theme::forest_theme()
    ));
    assert_eq!(
        crate::view::theme::tokens(&crate::view::theme::forest_theme()).accent,
        crate::view::theme::FOREST.accent
    );

    assert_eq!(parse_window_size("1280x800"), (1280.0, 800.0));
    assert_eq!(parse_window_size("1440X900"), (1440.0, 900.0));
    assert_eq!(parse_window_size("bogus"), DEFAULT_WINDOW);
    assert_eq!(parse_window_size("0x100"), DEFAULT_WINDOW);
    assert_eq!(parse_window_size("-10x100"), DEFAULT_WINDOW);
}
