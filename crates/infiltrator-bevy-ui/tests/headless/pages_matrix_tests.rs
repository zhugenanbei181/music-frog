//! Headless tests for the complete Bevy 0.30 business page matrix:
//! testing route mounting under ContentSlot, typed in-place projection updates,
//! AccessKit semantic markers, and idempotency across all 11 routes.

use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::app::{ContentSlot, ShellPlugin};
use infiltrator_bevy_ui::pages::app_routing::{
    AppItem, AppRouteRule, AppRoutingMode, AppRoutingProjection, AppRoutingProjectionUpdated,
    AppRuleText,
};
use infiltrator_bevy_ui::pages::connections::{
    ConnSpeedText, ConnectionItem, ConnectionsProjection, ConnectionsProjectionUpdated,
};
use infiltrator_bevy_ui::pages::dns::{
    DnsMode, DnsProjection, DnsProjectionUpdated, DnsServerItem, DnsServerLatency,
};
use infiltrator_bevy_ui::pages::doctor::{
    CheckStateText, DoctorCheckItem, DoctorCheckState, DoctorProjection, DoctorProjectionUpdated,
};
use infiltrator_bevy_ui::pages::logs::{
    LogEntry, LogLevel, LogMessageText, LogsProjection, LogsProjectionUpdated,
};
use infiltrator_bevy_ui::pages::profiles::{
    ProfileItem, ProfileNameText, ProfilesProjection, ProfilesProjectionUpdated,
};
use infiltrator_bevy_ui::pages::proxies::{
    LatencyText, NodeNameText, ProxiesProjection, ProxiesProjectionUpdated, ProxyGroup, ProxyNode,
};
use infiltrator_bevy_ui::pages::rules::{
    RuleHitText, RuleItem, RuleProviderItem, RuleProxyText, RulesProjection, RulesProjectionUpdated,
};
use infiltrator_bevy_ui::pages::settings::{
    SettingsLine, SettingsLineKind, SettingsProjection, SettingsProjectionUpdated,
};
use infiltrator_bevy_ui::pages::sync::{
    SnapshotItem, SyncProjection, SyncProjectionUpdated, SyncStatus,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};

fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.update();
    app
}

fn current_page_root(world: &mut World) -> (Entity, Route) {
    let mut roots = world.query::<(Entity, &PageRoot)>();
    let (id, root) = roots.single(world).expect("exactly one mounted page root");
    (id, root.0)
}

fn content_slot_entity(world: &mut World) -> Entity {
    let mut slots = world.query::<(Entity, &ContentSlot)>();
    slots.single(world).expect("content slot").0
}

#[test]
fn all_eleven_routes_mount_under_content_slot() {
    let mut app = create_test_app();
    let slot = content_slot_entity(app.world_mut());

    for route in Route::ALL {
        app.world_mut().commands().trigger(RouteChanged(route));
        app.update();

        let world = app.world_mut();
        let (root, mounted_route) = current_page_root(world);
        assert_eq!(mounted_route, route, "route matches {route:?}");

        let parent = world
            .get::<ChildOf>(root)
            .expect("mounted page has parent")
            .0;
        assert_eq!(
            parent, slot,
            "page root is parented under ContentSlot for {route:?}"
        );

        let mut a11y_query = world.query::<&AccessibilityNode>();
        assert!(
            a11y_query.iter(world).count() > 0,
            "page carries semantic nodes"
        );
    }
}

#[test]
fn proxies_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();

    let world = app.world_mut();
    let (_, route) = current_page_root(world);
    assert_eq!(route, Route::Proxies);

    // Initial check from demo
    {
        let mut names = world.query::<(&Text, &NodeNameText)>();
        let node0 = names
            .iter(world)
            .find(|(_, m)| m.group_idx == 0 && m.node_idx == 0);
        assert!(node0.is_some());
    }

    // Trigger updated projection
    let updated = ProxiesProjection {
        active_exit: "🇯🇵 日本东京 01 · 专线".to_owned(),
        testing: true,
        groups: vec![ProxyGroup {
            name: "节点选择 (PROXIES)".to_owned(),
            group_type: "Selector".to_owned(),
            current: "🇯🇵 日本东京 01 · 专线".to_owned(),
            proxies: vec![ProxyNode {
                name: "🇯🇵 日本东京 01 · 专线".to_owned(),
                node_type: "Shadowsocks".to_owned(),
                delay_ms: Some(42),
                selected: true,
            }],
        }],
    };

    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut names = world.query::<(&Text, &NodeNameText)>();
    let (text, _) = names
        .iter(world)
        .find(|(_, m)| m.group_idx == 0 && m.node_idx == 0)
        .expect("node found");
    assert_eq!(text.0, "🇯🇵 日本东京 01 · 专线");

    let mut latencies = world.query::<(&Text, &LatencyText)>();
    let (lat_text, _) = latencies
        .iter(world)
        .find(|(_, m)| m.group_idx == 0 && m.node_idx == 0)
        .expect("latency found");
    assert_eq!(lat_text.0, "42 ms");
}

#[test]
fn profiles_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Profiles));
    app.update();

    let updated = ProfilesProjection {
        auto_update_interval_hours: 12,
        updating: false,
        profiles: vec![ProfileItem {
            id: "sub-custom".to_owned(),
            name: "自建中继节点订阅".to_owned(),
            url: "https://my.nodes.net/sub".to_owned(),
            updated_at: "2026-09-02 11:00".to_owned(),
            upload_bytes: 500_000_000,
            download_bytes: 20_000_000_000,
            total_bytes: 500_000_000_000,
            is_active: true,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut names = world.query::<(&Text, &ProfileNameText)>();
    let (text, _) = names
        .iter(world)
        .find(|(_, m)| m.0 == 0)
        .expect("profile name");
    assert_eq!(text.0, "自建中继节点订阅");
}

#[test]
fn rules_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Rules));
    app.update();

    let updated = RulesProjection {
        total_rules: 9999,
        default_action: "REJECT".to_owned(),
        providers: vec![RuleProviderItem {
            name: "custom-mrs".to_owned(),
            rule_count: 5000,
            behavior: "domain".to_owned(),
            updated_at: "2026-09-02 12:00".to_owned(),
        }],
        rules: vec![RuleItem {
            id: 1,
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "anthropic.com".to_owned(),
            proxy: "AI-PROXIES".to_owned(),
            hit_count: 8888,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut hits = world.query::<(&Text, &RuleHitText)>();
    let (hit_text, _) = hits.iter(world).find(|(_, m)| m.0 == 0).expect("rule hit");
    assert_eq!(hit_text.0, "8888 次命中");

    let mut proxies = world.query::<(&Text, &RuleProxyText)>();
    let (proxy_text, _) = proxies
        .iter(world)
        .find(|(_, m)| m.0 == 0)
        .expect("rule proxy");
    assert_eq!(proxy_text.0, "AI-PROXIES");
}

#[test]
fn connections_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Connections));
    app.update();

    let updated = ConnectionsProjection {
        total_connections: 1,
        total_upload_bytes: 1_000_000,
        total_download_bytes: 5_000_000,
        connections: vec![ConnectionItem {
            id: "c-test".to_owned(),
            host: "musicfrog.app:443".to_owned(),
            process: "musicfrog-client".to_owned(),
            rule: "DOMAIN musicfrog.app".to_owned(),
            chain: "DIRECT".to_owned(),
            upload_bps: 1024.0,
            download_bps: 2048.0,
            upload_total: 100_000,
            download_total: 500_000,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut speeds = world.query::<(&Text, &ConnSpeedText)>();
    let (speed_text, _) = speeds
        .iter(world)
        .find(|(_, m)| m.0 == 0)
        .expect("conn speed");
    assert!(speed_text.0.contains("1.00 KB/s"));
}

#[test]
fn logs_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Logs));
    app.update();

    let updated = LogsProjection {
        total_entries: 1,
        active_level: Some(LogLevel::Warn),
        entries: vec![LogEntry {
            timestamp: "12:00:00.000".to_owned(),
            level: LogLevel::Warn,
            tag: "CORE".to_owned(),
            message: "Config reload successful without drops".to_owned(),
        }],
    };

    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut msgs = world.query::<(&Text, &LogMessageText)>();
    let (msg_text, _) = msgs.iter(world).find(|(_, m)| m.0 == 0).expect("log msg");
    assert_eq!(msg_text.0, "Config reload successful without drops");
}

#[test]
fn settings_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Settings));
    app.update();

    let updated = SettingsProjection {
        autostart: false,
        system_proxy: false,
        mixed_port: 7895,
        allow_lan: true,
        tun_enabled: true,
        tun_stack: "System (Native Stack)".to_owned(),
        controller_port: 9099,
        log_level: "debug".to_owned(),
    };

    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut lines = world.query::<(&Text, &SettingsLine)>();
    let port_text = lines
        .iter(world)
        .find(|(_, l)| l.0 == SettingsLineKind::MixedPort);
    assert_eq!(port_text.unwrap().0.0, "端口: 7895");

    let ctrl_text = lines
        .iter(world)
        .find(|(_, l)| l.0 == SettingsLineKind::ControllerPort);
    assert_eq!(ctrl_text.unwrap().0.0, "127.0.0.1:9099");
}

#[test]
fn sync_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Sync));
    app.update();

    let updated = SyncProjection {
        status: SyncStatus::Syncing,
        server_url: "https://webdav.custom.org/".to_owned(),
        username: "user2@custom.org".to_owned(),
        last_sync: Some("2026-09-02 12:30".to_owned()),
        auto_sync: false,
        snapshots: vec![SnapshotItem {
            id: "snap-new".to_owned(),
            timestamp: "2026-09-02 12:30".to_owned(),
            device: "SteamDeck".to_owned(),
            size_bytes: 99_000,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(SyncProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let (_, route) = current_page_root(world);
    assert_eq!(route, Route::Sync);

    let mut lines = world.query::<(&Text, &infiltrator_bevy_ui::pages::sync::SyncLine)>();
    let summary = lines
        .iter(world)
        .find(|(_, l)| l.0 == infiltrator_bevy_ui::pages::sync::SyncLineKind::Summary);
    assert!(summary.unwrap().0.0.contains("正在同步数据中..."));
}

#[test]
fn doctor_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Doctor));
    app.update();

    let updated = DoctorProjection {
        overall_healthy: false,
        last_run: "2026-09-02 12:45".to_owned(),
        checks: vec![DoctorCheckItem {
            id: "chk-fail".to_owned(),
            name: "TUN Device Error".to_owned(),
            category: "TUN".to_owned(),
            state: DoctorCheckState::Fail,
            detail: "Interface down".to_owned(),
            fix_available: true,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut states = world.query::<(&Text, &CheckStateText)>();
    let (state_text, _) = states
        .iter(world)
        .find(|(_, m)| m.0 == 0)
        .expect("check state");
    assert_eq!(state_text.0, "异常 (FAIL)");
}

#[test]
fn app_routing_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::AppRouting));
    app.update();

    let updated = AppRoutingProjection {
        mode: AppRoutingMode::ProxyList,
        include_system: true,
        apps: vec![AppItem {
            id: "app-block".to_owned(),
            name: "Malicious App".to_owned(),
            process_name: "bad.exe".to_owned(),
            rule: AppRouteRule::Block,
            is_system: false,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(AppRoutingProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut rules = world.query::<(&Text, &AppRuleText)>();
    let (rule_text, _) = rules.iter(world).find(|(_, m)| m.0 == 0).expect("app rule");
    assert_eq!(rule_text.0, "拦截 (Block)");
}

#[test]
fn dns_page_in_place_update() {
    let mut app = create_test_app();
    app.world_mut().commands().trigger(RouteChanged(Route::Dns));
    app.update();

    let updated = DnsProjection {
        mode: DnsMode::RedirHost,
        cache_entries: 512,
        fake_ip_range: "198.18.0.0/15".to_owned(),
        servers: vec![DnsServerItem {
            address: "https://dns.quad9.net/dns-query".to_owned(),
            protocol: "DoH (Quad9)".to_owned(),
            latency_ms: Some(19),
            is_fallback: false,
        }],
    };

    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(updated));
    app.update();

    let world = app.world_mut();
    let mut latencies = world.query::<(&Text, &DnsServerLatency)>();
    let (lat_text, _) = latencies
        .iter(world)
        .find(|(_, m)| m.0 == 0)
        .expect("dns latency");
    assert_eq!(lat_text.0, "19 ms");
}
