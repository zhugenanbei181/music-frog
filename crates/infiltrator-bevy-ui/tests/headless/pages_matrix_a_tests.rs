//! Headless integration tests for Page Matrix A (Proxies, Profiles, Connections, Logs, Rules):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - Empty lists, boundary conditions, and defensive rendering.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::connections::*;
use infiltrator_bevy_ui::pages::logs::*;
use infiltrator_bevy_ui::pages::profiles::*;
use infiltrator_bevy_ui::pages::profiles_import::{
    ChooseLocalFileButton, ImportLocalFileButton, ProfilesImportRoot, SaveUserAgentButton,
};
use infiltrator_bevy_ui::pages::proxies::*;
use infiltrator_bevy_ui::pages::rules::*;
use infiltrator_bevy_ui::pages::rules_mrs::{RulesMrsRoot, UnpackRuleProviderButton};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ControlVisual;

use crate::support::*;

fn setup_matrix_a_app(sink: Arc<DemoCommandSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    app
}

fn navigate_to(app: &mut App, route: Route) -> (Entity, Entity) {
    app.world_mut().commands().trigger(RouteChanged(route));
    app.update();
    let slot = content_slot(app.world_mut());
    let (root, mounted_route) = page_root(app.world_mut());
    assert_eq!(mounted_route, route);
    let parent = app
        .world()
        .get::<ChildOf>(root)
        .expect("page root parent")
        .0;
    assert_eq!(parent, slot, "page root is parented under ContentSlot");
    (root, slot)
}

// ===========================================================================
// 1. Proxies Page Tests
// ===========================================================================

#[test]
fn test_proxies_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    assert!(subtree_has_text(
        app.world(),
        root,
        "代理策略 · 共 3 个策略组 (8 个节点)"
    ));
    assert!(subtree_has_text(app.world(), root, "🇭🇰 香港 01 · BGP 专线"));
    assert!(subtree_has_text(app.world(), root, "测速就绪"));
    assert!(subtree_has_text(app.world(), root, "节点选择 (PROXIES)"));
    assert!(subtree_has_text(app.world(), root, "38 ms"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "搜索代理或节点 (Search Proxies)..."
    ));
    assert!(subtree_has_text(app.world(), root, "组测速"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自定义节点与分享链接 (Custom Node & URI Codec)"
    ));
    assert!(subtree_has_text(app.world(), root, "解析剪贴板 URI"));
    assert!(subtree_has_text(app.world(), root, "保存为自定义节点"));

    // Enhanced toolbar & controls parity with Iced
    assert!(subtree_has_text(app.world(), root, "只看可用"));
    assert!(subtree_has_text(app.world(), root, "延迟升序"));
    assert!(subtree_has_text(app.world(), root, "延迟降序"));
    assert!(subtree_has_text(app.world(), root, "名称升序"));
    assert!(subtree_has_text(app.world(), root, "名称降序"));
    assert!(subtree_has_text(app.world(), root, "测试地址"));
    assert!(subtree_has_text(app.world(), root, "+ 添加节点"));
    assert!(subtree_has_text(app.world(), root, "网格视图"));
    // Node capability tags & protocol badges
    assert!(subtree_has_text(app.world(), root, "udp"));
    assert!(subtree_has_text(app.world(), root, "Shadowsocks"));

    // Component markers verification
    assert!(
        app.world_mut()
            .query::<&FilterAliveToggle>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ProxySortPill>()
            .iter(app.world())
            .count()
            >= 4
    );
    assert!(
        app.world_mut()
            .query::<&DelayTestUrlIndicator>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&AddCustomNodeButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ToggleViewModeButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_proxies_test_all_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<TestAllProxiesButton>>()
        .single(app.world())
        .expect("test all proxies button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::TestAllProxyGroups]);
}

#[test]
fn test_proxies_test_group_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut query = app.world_mut().query::<(Entity, &TestProxyGroupButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.group_idx == 0)
        .expect("test proxy group button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::TestProxyGroup {
            group: "节点选择 (PROXIES)".to_owned(),
        }]
    );
}

#[test]
fn test_proxies_select_node_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Proxies);

    let mut query = app.world_mut().query::<(Entity, &ProxyNodeButton)>();
    let (node_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.node_name == "🇯🇵 日本东京 02 · 极速")
        .expect("target proxy node button");

    app.world_mut().commands().trigger(Activate {
        entity: node_entity,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SelectProxyNode {
            group: "节点选择 (PROXIES)".to_owned(),
            node: "🇯🇵 日本东京 02 · 极速".to_owned(),
        }]
    );
}

#[test]
fn test_proxies_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    let mut updated = ProxiesProjection::demo();
    updated.active_exit = "🇸🇬 新加坡 01 · Anycast".to_owned();
    updated.testing = true;
    updated.groups[0].current = "🇸🇬 新加坡 01 · Anycast".to_owned();
    updated.groups[0].proxies[0].selected = false;
    updated.groups[0].proxies[0].delay_ms = Some(180);
    updated.groups[0].proxies[2].selected = true;
    updated.groups[0].proxies[2].delay_ms = Some(28);

    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "🇸🇬 新加坡 01 · Anycast"
    ));
    assert!(subtree_has_text(app.world(), root, "正在全面测速中..."));
    assert!(subtree_has_text(app.world(), root, "180 ms"));
    assert!(subtree_has_text(app.world(), root, "28 ms"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "选中: 🇸🇬 新加坡 01 · Anycast"
    ));

    let mut node_query = app
        .world_mut()
        .query::<(&ProxyNodeButton, &ControlVisual)>();
    let selected_node = node_query
        .iter(app.world())
        .find(|(btn, _)| btn.group_idx == 0 && btn.node_idx == 2)
        .map(|(_, visual)| visual.0);
    assert_eq!(
        selected_node,
        Some(true),
        "newly selected node visual is true"
    );
}

#[test]
fn test_proxies_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    let empty = ProxiesProjection {
        groups: vec![],
        testing: false,
        active_exit: "无可用出口".to_owned(),
    };
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "代理策略 · 共 0 个策略组 (0 个节点)"
    ));
    assert!(subtree_has_text(app.world(), root, "无可用出口"));
    assert!(subtree_has_text(app.world(), root, "测速就绪"));
}

// ===========================================================================
// 2. Profiles Page Tests
// ===========================================================================

#[test]
fn test_profiles_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 3 个配置 (当前生效: 主力高速订阅 (Primary VIP))"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动更新周期: 每 24 小时"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "主力高速订阅 (Primary VIP)"
    ));
    assert!(subtree_has_text(app.world(), root, "当前生效中"));
    assert!(subtree_has_text(app.world(), root, "点击启用"));
    assert!(subtree_has_text(app.world(), root, "导入本地配置文件"));
    assert!(subtree_has_text(app.world(), root, "选择文件"));
    assert!(subtree_has_text(app.world(), root, "+ 导入本地文件"));
    assert!(subtree_has_text(app.world(), root, "导入后立即激活"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "订阅请求设置 (Subscription User-Agent)"
    ));
    assert!(subtree_has_text(app.world(), root, "保存 UA 设置"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "多订阅节点聚合器 (Profile Aggregator)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键聚合为新配置"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "配置历史快照比对 (Snapshot Visual Diff)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键安全还原此快照"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "QuickJS 扩展脚本沙箱控制台 (Script Sandbox)"
    ));
    assert!(subtree_has_text(app.world(), root, "测试运行脚本变换"));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<ProfilesImportRoot>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ProfilesImportRoot marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<ChooseLocalFileButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ChooseLocalFileButton marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<ImportLocalFileButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ImportLocalFileButton marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<SaveUserAgentButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "SaveUserAgentButton marker exists"
    );
}

#[test]
fn test_profiles_activate_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    let mut query = app.world_mut().query::<(Entity, &ActivateProfileButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.profile_id == "sub-2")
        .expect("sub-2 activate button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ActivateProfile {
            id: "sub-2".to_owned(),
        }]
    );
}

#[test]
fn test_profiles_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut updated = ProfilesProjection::demo();
    updated.auto_update_interval_hours = 6;
    updated.profiles[0].is_active = false;
    updated.profiles[1].is_active = true;
    updated.profiles[1].name = "备用容灾线路 (Active Live)".to_owned();

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 3 个配置 (当前生效: 备用容灾线路 (Active Live))"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动更新周期: 每 6 小时"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "备用容灾线路 (Active Live)"
    ));

    let mut btn_query = app
        .world_mut()
        .query::<(&ActivateProfileButton, &ControlVisual)>();
    let sub2_visual = btn_query
        .iter(app.world())
        .find(|(btn, _)| btn.profile_id == "sub-2")
        .map(|(_, visual)| visual.0);
    assert_eq!(sub2_visual, Some(true), "sub-2 is active visual");
}

#[test]
fn test_profiles_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let empty = ProfilesProjection {
        profiles: vec![],
        auto_update_interval_hours: 0,
        updating: false,
    };
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 0 个配置 (当前生效: 无活动配置)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动更新周期: 每 0 小时"
    ));
}

// ===========================================================================
// 3. Connections Page Tests
// ===========================================================================

#[test]
fn test_connections_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 4 个连接"
    ));
    assert!(subtree_has_text(app.world(), root, "api.github.com:443"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "manifest.googlevideo.com:443"
    ));
    assert!(subtree_has_text(app.world(), root, "关闭全部连接"));
    assert!(subtree_has_text(app.world(), root, "断开"));
    assert!(subtree_has_text(app.world(), root, "全部连接 (Flat)"));
    assert!(subtree_has_text(app.world(), root, "按应用进程聚合"));
    assert!(subtree_has_text(app.world(), root, "按目标域名聚合"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "单连接深度透视 (Deep Telemetry Waterfall)"
    ));
    assert!(subtree_has_text(app.world(), root, "DNS 解析"));
    assert!(subtree_has_text(app.world(), root, "一键添加为规则"));
    assert_eq!(
        app.world_mut()
            .query::<&ConnAggregationPill>()
            .iter(app.world())
            .count(),
        3
    );
}

#[test]
fn test_connections_close_all_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CloseAllConnectionsButton>>()
        .single(app.world())
        .expect("close all connections button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::CloseAllConnections]);
}

#[test]
fn test_connections_close_single_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let mut query = app.world_mut().query::<(Entity, &CloseConnectionButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.connection_id == "c-1")
        .expect("c-1 close button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
}

#[test]
fn test_connections_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let mut updated = ConnectionsProjection::demo();
    updated.total_connections = 12;
    updated.total_upload_bytes = 50_000_000;
    updated.total_download_bytes = 300_000_000;
    updated.connections[0].host = "api.cloudflare.com:443".to_owned();
    updated.connections[0].upload_bps = 500_000.0;
    updated.connections[0].download_bps = 1_200_000.0;

    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 12 个连接"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "api.cloudflare.com:443"
    ));
    assert!(subtree_has_text(app.world(), root, "488.28 KB/s"));
    assert!(subtree_has_text(app.world(), root, "1.14 MB/s"));
}

#[test]
fn test_connections_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let empty = ConnectionsProjection {
        total_connections: 0,
        total_upload_bytes: 0,
        total_download_bytes: 0,
        connections: vec![],
    };
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "活动连接 · 当前活跃 0 个连接"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "累积上传: 0 B | 累积下载: 0 B"
    ));
}

// ===========================================================================
// 4. Logs Page Tests
// ===========================================================================

#[test]
fn test_logs_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 5 行日志"
    ));
    assert!(subtree_has_text(app.world(), root, "清空"));
    assert!(subtree_has_text(app.world(), root, "滚屏锁定"));
    assert!(subtree_has_text(app.world(), root, "导出日志"));
    assert!(subtree_has_text(app.world(), root, "DEBUG"));
    assert!(subtree_has_text(app.world(), root, "INFO"));
    assert!(subtree_has_text(app.world(), root, "WARN"));
    assert!(subtree_has_text(app.world(), root, "ERROR"));
    assert!(subtree_has_text(app.world(), root, "[INFO]"));
    assert!(subtree_has_text(app.world(), root, "[WARN]"));
    assert!(subtree_has_text(app.world(), root, "[ERROR]"));
    assert!(
        app.world_mut()
            .query::<&PauseLogsButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&ExportLogsButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_logs_clear_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Logs);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ClearLogsButton>>()
        .single(app.world())
        .expect("clear logs button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ClearLogs]);
}

#[test]
fn test_logs_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    let mut updated = LogsProjection::demo();
    updated.total_entries = 100;
    updated.entries[0].message = "[TCP] connection reset by peer in test".to_owned();
    updated.entries[0].level = LogLevel::Error;
    updated.entries[0].timestamp = "11:22:33.444".to_owned();

    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 100 行日志"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "[TCP] connection reset by peer in test"
    ));
    assert!(subtree_has_text(app.world(), root, "11:22:33.444"));
}

#[test]
fn test_logs_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Logs);

    let empty = LogsProjection {
        total_entries: 0,
        active_level: None,
        entries: vec![],
    };
    app.world_mut()
        .commands()
        .trigger(LogsProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "运行日志 · 环形缓冲区共 0 行日志"
    ));
}

// ===========================================================================
// 5. Rules Page Tests
// ===========================================================================

#[test]
fn test_rules_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 2842 条规则 (3 个规则集 / 命中统计开启)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "最终匹配目标: DIRECT (漏网之鱼直连)"
    ));
    assert!(subtree_has_text(app.world(), root, "刷新规则集"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geosite-geolocation-!cn"
    ));
    assert!(subtree_has_text(app.world(), root, "google.com"));
    assert!(subtree_has_text(app.world(), root, "1420 次命中"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "实时分流追踪器沙盒 (Live Rule Tracer)"
    ));
    assert!(subtree_has_text(app.world(), root, "执行模拟追踪"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "【匹配命中】规则 #42: DOMAIN-SUFFIX, github.com"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 二进制规则集治理与解构 (MRS Ruleset Engine)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "一键解构导入为本地规则"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "添加自定义规则向导 (Add Custom Rule)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键注入游戏分流预设"));
    assert!(subtree_has_text(app.world(), root, "+ 确认添加规则"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geoip.mrs (14,200 条目 · IPCIDR · 高性能 mmap 索引)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geosite-cn.mrs (28,500 条目 · Domain · 二进制缓存正常)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "支持本地 .mrs 二进制规则集秒级索引与 diff 比对"
    ));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<RulesMrsRoot>>()
            .iter(app.world())
            .next()
            .is_some(),
        "RulesMrsRoot marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<UnpackRuleProviderButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "UnpackRuleProviderButton marker exists"
    );
}

#[test]
fn test_rules_refresh_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<RefreshRuleProvidersButton>>()
        .single(app.world())
        .expect("refresh rules button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RefreshRuleProviders]);
}

#[test]
fn test_rules_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut updated = RulesProjection::demo();
    updated.total_rules = 5000;
    updated.default_action = "REJECT (阻断)".to_owned();
    updated.rules[0].hit_count = 9999;
    updated.rules[0].proxy = "国外媒体".to_owned();
    updated.providers[0].name = "custom-mrs-provider".to_owned();
    updated.providers[0].rule_count = 2000;

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 5000 条规则 (3 个规则集 / 命中统计开启)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "最终匹配目标: REJECT (阻断)"
    ));
    assert!(subtree_has_text(app.world(), root, "9999 次命中"));
    assert!(subtree_has_text(app.world(), root, "国外媒体"));
    assert!(subtree_has_text(app.world(), root, "custom-mrs-provider"));
    assert!(subtree_has_text(app.world(), root, "2000 条 (domain)"));
}

#[test]
fn test_rules_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let empty = RulesProjection {
        total_rules: 0,
        default_action: "DIRECT".to_owned(),
        providers: vec![],
        rules: vec![],
    };
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 0 条规则 (0 个规则集 / 命中统计开启)"
    ));
    assert!(subtree_has_text(app.world(), root, "最终匹配目标: DIRECT"));
}

#[test]
fn test_proxies_favorite_and_features_rendering() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Proxies);

    // Default demo has favorite stars, pin icons, latency trend waves, and feature chips
    assert!(subtree_has_text(app.world(), root, "★"));
    assert!(subtree_has_text(app.world(), root, "📌"));
    assert!(subtree_has_text(app.world(), root, "📈"));
    assert!(subtree_has_text(app.world(), root, "udp"));
    assert!(subtree_has_text(app.world(), root, "UDP"));
    assert!(subtree_has_text(app.world(), root, "TFO"));
    assert!(subtree_has_text(app.world(), root, "Vision"));
    assert!(subtree_has_text(app.world(), root, "Reality"));
    assert!(subtree_has_text(app.world(), root, "Shadowsocks"));

    assert!(
        app.world_mut()
            .query::<&NodePinButton>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&LatencyTrendIcon>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query::<&NodeUdpTag>()
            .iter(app.world())
            .next()
            .is_some()
    );
}
