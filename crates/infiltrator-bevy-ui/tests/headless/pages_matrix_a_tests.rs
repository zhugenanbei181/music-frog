//! Headless integration tests for Page Matrix A (Proxies, Profiles, Connections, Logs, Rules):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - Empty lists, boundary conditions, and defensive rendering.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::connections::*;
use infiltrator_bevy_ui::pages::connections_drawer::*;
use infiltrator_bevy_ui::pages::connections_idle::*;
use infiltrator_bevy_ui::pages::connections_view::*;
use infiltrator_bevy_ui::pages::logs::*;
use infiltrator_bevy_ui::pages::profiles::*;
use infiltrator_bevy_ui::pages::profiles_import::{
    ChooseLocalFileButton, ImportLocalFileButton, ProfilesImportRoot,
    RestoreSubscriptionBackupButton, SaveUserAgentButton, SubscriptionBackupStatus,
    SubscriptionInsecureToggle, SubscriptionUserAgentField,
};
use infiltrator_bevy_ui::pages::profiles_import_channels::{
    ImportClipboardSubscriptionButton, ImportLocalPathField, ImportLocalSubscriptionButton,
    ImportSubscriptionNameField, ImportSubscriptionUrlButton, ImportSubscriptionUrlField,
    SaveSubscriptionFilterButton, SubscriptionFilterIncludeField,
};
use infiltrator_bevy_ui::pages::proxies::*;
use infiltrator_bevy_ui::pages::rules::*;
use infiltrator_bevy_ui::pages::rules_builder::{
    AddCustomRuleButton, InjectGamePresetsButton, RuleBuilderSelection, RulePayloadField,
    RuleTargetField, RuleTypeChip, RulesBuilderState,
};
use infiltrator_bevy_ui::pages::rules_edit::{
    RuleMoveDownButton, RuleMoveUpButton, RuleToggleButton,
};
use infiltrator_bevy_ui::pages::rules_mrs::{RulesMrsRoot, UnpackRuleProviderButton};
use infiltrator_bevy_ui::pages::rules_tracer::{
    ApplyTracerRuleOverrideButton, SimulateRuleTraceButton, TracerOverrideTargetField,
    TracerQueryField, TracerSourceIpField,
};
use infiltrator_bevy_ui::pages::rules_view::{
    RuleRow, RuleSearchField, RulesPageIndicator, RulesViewState,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::state::TextFieldState;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::mrs_acceleration::{
    MrsAccelerationSnapshot, MrsBehaviorKind, MrsCompressionKind, MrsItemSnapshot,
};
use infiltrator_contract::rule_tracer::{RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot};
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_tracer::RuleOverridePort;
use std::sync::Mutex;

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
fn test_proxies_node_cards_reflow_across_tiers() {
    // Proxy node cards must track the shared tier column operator, not a fixed
    // 49% width. Sizes chosen to land squarely inside each tier band.
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Proxies);

    let node_width = |app: &mut App| -> bevy::ui::prelude::Val {
        let mut query = app
            .world_mut()
            .query::<(&bevy::ui::prelude::Node, &ProxyNodeButton)>();
        query
            .iter(app.world())
            .next()
            .map(|(node, _)| node.width)
            .expect("at least one proxy node card")
    };

    let cases = [(400.0_f32, 1usize), (700.0, 2), (1000.0, 3), (1600.0, 4)];
    for (width, columns) in cases {
        app.world_mut()
            .resource_mut::<infiltrator_bevy_widgets::responsive::ResponsiveContext>()
            .set_dimensions(width, 900.0);
        app.update();
        let expected = bevy::ui::prelude::Val::Percent(
            infiltrator_bevy_widgets::fluid_grid::FluidCardGrid::wrapped_item_percent(columns),
        );
        assert_eq!(
            node_width(&mut app),
            expected,
            "proxy node width at {width}px should reflect {columns} columns"
        );
    }
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
    assert!(subtree_has_text(app.world(), root, "保存请求设置"));
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
fn test_profiles_update_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(ProfilesProjection::demo()));
    app.update();

    let update = app
        .world_mut()
        .query::<(Entity, &UpdateProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 1)
        .map(|(entity, _)| entity)
        .expect("sub-2 update button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: update });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateProfile {
            id: "sub-2".to_owned(),
        }],
        "per-profile update routes through the shared command (retry/backoff + single-flight)"
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
        "单连接深度透视 (Deep Telemetry)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "内核未提供该连接的 DNS/TCP/TLS/TTFB 耗时明细"
    ));
    assert!(subtree_has_text(app.world(), root, "一键添加为规则"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "按域名/IP/进程即时搜索连接"
    ));
    assert!(subtree_has_text(app.world(), root, "断开筛选结果"));
    assert_eq!(
        app.world_mut()
            .query::<&ConnAggregationPill>()
            .iter(app.world())
            .count(),
        3
    );
}

#[test]
fn test_connections_close_all_requires_confirmation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CloseAllConnectionsButton>>()
        .single(app.world())
        .expect("close all connections button");

    // First click only arms the destructive action.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();
    assert!(sink.submitted().is_empty());
    let armed = app
        .world_mut()
        .query::<(&bevy::ui::widget::Text, &CloseAllConnectionsLabel)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .collect::<Vec<_>>();
    assert!(armed.iter().any(|line| line.contains("确认关闭全部")));

    // Second click executes it.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::CloseAllConnections]);
}

#[test]
fn test_connections_aggregation_pill_switches_shared_mode() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let mut query = app.world_mut().query::<(Entity, &ConnAggregationPill)>();
    let (pill_entity, _) = query
        .iter(app.world())
        .find(|(_, pill)| pill.0 == ConnectionGroupingMode::ByProcess)
        .expect("by-process aggregation pill");

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "按应用进程聚合 · 共 4 组"
    ));
    // Grouped mode hides the flat rows and shows the summary.
    let rows_container_display = app
        .world_mut()
        .query_filtered::<&Node, bevy::ecs::query::With<ConnRowsContainer>>()
        .single(app.world())
        .expect("rows container")
        .display;
    assert_eq!(rows_container_display, Display::None);
}

#[test]
fn test_connections_search_hides_non_matching_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    let field_entity = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<ConnSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field");
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("github");
    app.update();

    let mut rows = app.world_mut().query::<(&Node, &ConnectionRow)>();
    let displays: Vec<(usize, Display)> = rows
        .iter(app.world())
        .map(|(node, row)| (row.0, node.display))
        .collect();
    assert!(displays.contains(&(0, Display::Flex)));
    assert!(displays.contains(&(1, Display::None)));
}

#[test]
fn test_connections_close_filtered_submits_matching_only() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let field_entity = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<ConnSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field");
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("github");
    app.update();

    let button_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CloseFilteredConnectionsButton>>()
        .single(app.world())
        .expect("close filtered button");
    app.world_mut().commands().trigger(Activate {
        entity: button_entity,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::CloseConnection {
            id: "c-1".to_owned(),
        }]
    );
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
        stream_phase: infiltrator_contract::connection::ConnectionStreamPhase::Unavailable,
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

#[test]
fn test_connections_stream_badge_reflects_shared_phase() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // The demo projection carries the shared `Live` phase.
    assert!(subtree_has_text(app.world(), root, "连接流 · 实时"));

    let mut reconnecting = ConnectionsProjection::demo();
    reconnecting.stream_phase =
        infiltrator_contract::connection::ConnectionStreamPhase::Reconnecting;
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(reconnecting));
    app.update();
    assert!(subtree_has_text(app.world(), root, "连接流 · 重连中"));
}

#[test]
fn test_connections_route_chain_renders_each_hop() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // DUAL-13-06: each hop is its own text node through the shared model.
    assert!(subtree_has_text(app.world(), root, "节点选择"));
    assert!(subtree_has_text(app.world(), root, "🇭🇰 香港 01"));
    assert!(subtree_has_text(app.world(), root, "国外媒体"));
    // The pre-joined snapshot string is no longer what the surface renders.
    assert!(!subtree_has_text(
        app.world(),
        root,
        "节点选择 -> 🇭🇰 香港 01"
    ));
}

#[test]
fn test_connections_inspect_opens_shared_drawer() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Connections);

    let inspect_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.0 == 0)
            .map(|(entity, _)| entity)
            .expect("row 0 inspect button")
    };

    app.world_mut().commands().trigger(Activate {
        entity: inspect_entity,
    });
    app.update();

    let state = app.world().resource::<ConnectionsDrawerState>();
    assert!(state.open);
    assert_eq!(state.selected, Some(0));

    let layer_display = app
        .world_mut()
        .query_filtered::<&Node, bevy::ecs::query::With<ConnectionDrawerLayer>>()
        .single(app.world())
        .expect("drawer layer")
        .display;
    assert_eq!(layer_display, Display::Flex);
    assert!(subtree_has_text(app.world(), root, "api.github.com:443"));
}

#[test]
fn test_connections_add_rule_draft_uses_shared_seam() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    // Select row 0, then draft a reverse rule from the drawer action.
    let inspect_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnInspectButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.0 == 0)
            .map(|(entity, _)| entity)
            .expect("row 0 inspect button")
    };
    app.world_mut().commands().trigger(Activate {
        entity: inspect_entity,
    });
    app.update();

    let add_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DrawerAddRuleButton>>()
        .single(app.world())
        .expect("add rule button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: add_entity });
    app.update();

    let first = app.world().resource::<ConnectionsRuleDraft>().clone();
    assert_eq!(first.entries.len(), 1);
    assert_eq!(first.entries[0].rule, "DOMAIN-SUFFIX,api.github.com,DIRECT");

    // The shared seam de-duplicates the same rule line.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: add_entity });
    app.update();
    assert_eq!(
        app.world().resource::<ConnectionsRuleDraft>().entries.len(),
        1
    );
}

#[test]
fn test_connections_idle_timeout_pill_switches_shared_choice() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Connections);

    let pill_entity = {
        let mut query = app.world_mut().query::<(Entity, &ConnIdleTimeoutPill)>();
        query
            .iter(app.world())
            .find(|(_, pill)| pill.0 == 1800)
            .map(|(entity, _)| entity)
            .expect("30m idle timeout pill")
    };
    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ConnectionsIdleState>().timeout_secs,
        1800
    );
}

#[test]
fn test_connections_idle_sweep_submits_and_reports() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Connections);

    // Rewind the tracker's activity clock to the epoch so the demo rows count
    // as idle, then run the sweep button.
    let demo = ConnectionsProjection::demo();
    {
        let mut state = app.world_mut().resource_mut::<ConnectionsIdleState>();
        let mut changed = demo.connections.clone();
        for item in &mut changed {
            item.upload_total += 1;
        }
        state.tracker.observe(&changed, 1);
        state.tracker.observe(&demo.connections, 1);
        state.timeout_secs = 0;
    }

    let sweep_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ConnIdleSweepButton>>()
        .single(app.world())
        .expect("idle sweep button");
    app.world_mut().commands().trigger(Activate {
        entity: sweep_entity,
    });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 4);
    assert!(
        submitted
            .iter()
            .all(|command| matches!(command, UiCommand::CloseConnection { .. }))
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "上次清理: 4 条空闲连接"
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
        "【匹配命中】规则 #42: DOMAIN-SUFFIX,github.com,PROXY -> PROXY"
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
    // DUAL-11-09: the shared disabled flag reaches the row label.
    assert!(subtree_has_text(app.world(), root, "已停用"));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<RuleToggleButton>>()
            .iter(app.world())
            .count()
            == 5,
        "one toggle control per demo rule"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<RuleTypeChip>>()
            .iter(app.world())
            .count()
            == infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES.len(),
        "wizard exposes the shared rule-type vocabulary"
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "geoip-cn.mrs (8500 条目 · ipcidr · 校验通过 · sha256 e3b0c44298fc)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geosite-geolocation-!cn.mrs (28400 条目 · domain · 校验通过 · sha256 cbf529a4d5d4)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 加速就绪 · 3 个规则集 · 37472 条规则"
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
        tracer: Default::default(),
        hit_audit: Default::default(),
        mrs_acceleration: Default::default(),
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
    // Honest zero-audit line: no fabricated hit totals.
    assert!(subtree_has_text(
        app.world(),
        root,
        "命中 0 · 冷门/被遮蔽 0 · CIDR 重叠 0 · 匹配 —"
    ));
    // Empty tracer snapshot renders the honest empty state, never a
    // fabricated replay.
    assert!(subtree_has_text(
        app.world(),
        root,
        "输入测试目标后执行模拟追踪，决策链路将在此回放"
    ));
}

#[test]
fn test_rules_tracer_projection_renders_shared_decision_chain() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // The demo projection carries the shared demo_fixture chain, so the
    // tracer card must replay that exact decision chain — five stages, the
    // matched rule headline and the resolved outbound node.
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(RulesProjection::demo()));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "【匹配命中】规则 #42: DOMAIN-SUFFIX,github.com,PROXY -> PROXY"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "· inbound | 混合端口监听 (Mixed) | 127.0.0.1:7890 (TCP)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "· rule_set | 命中规则 #42: DOMAIN-SUFFIX, github.com"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "最终出站: 香港 IPLC 01"
    ));
}

#[test]
fn test_rules_tracer_source_ip_sandbox_submits_shared_context() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // The shared snapshot's Inbound stage reflects the simulated source IP;
    // the card must render that exact shared decision-chain line.
    let mut projection = RulesProjection::demo();
    if let Some(chain) = projection.tracer.decision_chain.as_mut() {
        chain.nodes[0].detail = "10.20.30.40:7890 (TCP)".to_owned();
    }
    projection.tracer.simulated_context.src_ip = Some("10.20.30.40".to_owned());
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "· inbound | 混合端口监听 (Mixed) | 10.20.30.40:7890 (TCP)"
    ));

    // Both sandbox inputs mount: the target query and the source-IP field.
    let query_source = {
        let mut fields = app.world_mut().query::<(&TracerQueryField, &Children)>();
        *fields
            .single(app.world())
            .expect("query field wrapper")
            .1
            .iter()
            .next()
            .expect("query text field")
    };
    let src_source = {
        let mut fields = app.world_mut().query::<(&TracerSourceIpField, &Children)>();
        *fields
            .single(app.world())
            .expect("source ip field wrapper")
            .1
            .iter()
            .next()
            .expect("source ip text field")
    };
    app.world_mut()
        .get_mut::<TextField>(query_source)
        .expect("query field state")
        .0
        .apply(TextFieldInput::SetText("google.com".to_owned()));
    app.world_mut()
        .get_mut::<TextField>(src_source)
        .expect("source ip field state")
        .0
        .apply(TextFieldInput::SetText("10.20.30.40".to_owned()));

    // The simulate button submits the shared sandbox context, then the query;
    // no UI-local trace is fabricated.
    let simulate = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SimulateRuleTraceButton>>()
        .single(app.world())
        .expect("simulate button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: simulate });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetRuleTracerContext {
                src_ip: Some("10.20.30.40".to_owned()),
            },
            UiCommand::SimulateRuleTrace {
                query: "google.com".to_owned(),
            },
        ]
    );
}

/// DUAL-12-08: an in-memory host apply capability for the Bevy headless test.
struct FakeRuleOverridePort {
    rules: Mutex<Vec<RuleEntry>>,
}

#[async_trait::async_trait]
impl RuleOverridePort for FakeRuleOverridePort {
    async fn load_rule_entries(&self) -> Result<Vec<RuleEntry>, PortError> {
        Ok(self.rules.lock().expect("rules lock").clone())
    }

    async fn apply_rule_entries(&self, entries: &[RuleEntry]) -> Result<(), PortError> {
        *self.rules.lock().expect("rules lock") = entries.to_vec();
        Ok(())
    }
}

#[test]
fn test_rules_tracer_override_submits_and_consumes_shared_result() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // The demo fixture matched rule #42 (0-based index 41) with target PROXY,
    // so the shared `can_reverse_apply` fact mounts the chooser.
    let mut projection = RulesProjection::demo();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection.clone()));
    app.update();

    let override_source = {
        let mut fields = app
            .world_mut()
            .query::<(&TracerOverrideTargetField, &Children)>();
        *fields
            .single(app.world())
            .expect("override target field wrapper")
            .1
            .iter()
            .next()
            .expect("override text field")
    };
    // The field seeds from the shared suggested target; the user overrides it.
    assert_eq!(
        app.world()
            .get::<TextField>(override_source)
            .expect("override field state")
            .0
            .text(),
        "DIRECT"
    );
    app.world_mut()
        .get_mut::<TextField>(override_source)
        .expect("override field state")
        .0
        .apply(TextFieldInput::SetText("REJECT".to_owned()));

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ApplyTracerRuleOverrideButton>>()
        .single(app.world())
        .expect("apply override button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    let expected = UiCommand::ApplyTracerRuleOverride {
        rule_index: 41,
        new_target: "REJECT".to_owned(),
    };
    assert_eq!(sink.submitted().last(), Some(&expected));

    // Drive the exact submitted intent through the shared application with a
    // composed override port and consume the one typed result.
    let intent = expected.to_intent().expect("override intent");
    let request = match intent {
        CommandIntent::ApplyTracerRuleOverride { request } => request,
        other => panic!("unexpected intent: {other:?}"),
    };
    let mut rules: Vec<RuleEntry> = (0..42)
        .map(|index| RuleEntry {
            rule: format!("DOMAIN-SUFFIX,rule{index}.com,PROXY"),
            enabled: true,
        })
        .collect();
    rules[41] = RuleEntry {
        rule: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
        enabled: true,
    };
    let port = Arc::new(FakeRuleOverridePort {
        rules: Mutex::new(rules),
    });
    let application = RuleTracerApplication::new();
    application.set_override_port(port);
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime")
        .block_on(application.apply_override(&request));
    assert!(result.is_applied());
    assert_eq!(
        result.updated_rule_raw.as_deref(),
        Some("DOMAIN-SUFFIX,github.com,REJECT")
    );

    // Consume the shared result on the Bevy surface: the tracer card replays
    // the applied decision chain's new outbound, not a UI-local guess.
    let chain = projection
        .tracer
        .decision_chain
        .as_mut()
        .expect("demo decision chain");
    chain.target_proxy = result.new_target.clone();
    chain.matched_rule_raw = result.updated_rule_raw.clone().unwrap_or_default();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "DOMAIN-SUFFIX,github.com,REJECT -> REJECT"
    ));
}

#[test]
fn test_rules_hit_audit_projection_and_clear_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let dead = RuleDeadEntry {
        rule_raw: "MATCH,DIRECT".to_owned(),
        hit_count: 0,
        reason: RuleDeadReason::ZeroHits,
        shadowed_by: None,
        detail: None,
        last_hit_secs: None,
    };
    let overlap = RuleDeadEntry {
        rule_raw: "IP-CIDR,10.1.2.0/24,PROXY".to_owned(),
        hit_count: 0,
        reason: RuleDeadReason::Shadowed,
        shadowed_by: Some("IP-CIDR,10.0.0.0/8,DIRECT".to_owned()),
        detail: Some("IP CIDR is shadowed by an earlier broader IP-CIDR rule".to_owned()),
        last_hit_secs: None,
    };

    let mut projection = RulesProjection::demo();
    projection.hit_audit = RuleHitAuditSnapshot {
        total_hits: 1287,
        tracked_rules: 42,
        top_hits: Vec::new(),
        dead_rules: vec![dead, overlap.clone()],
        cidr_overlaps: vec![overlap],
        last_hit_rule: Some("DOMAIN-SUFFIX,google.com,PROXY".to_owned()),
        last_hit_secs: Some(1_700_000_012),
        can_clear: true,
        trace_count: 128,
        avg_match_latency_us: Some(18.5),
        last_match_latency_us: Some(14),
    };
    // The demo MATCH rule is the last entry; flag it shadowed so the row label
    // must render the shared shadow fact.
    if let Some(rule) = projection.rules.last_mut() {
        rule.is_shadowed = true;
        rule.shadow_reason = Some("unreachable after MATCH".to_owned());
    }

    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "命中 1287 · 冷门/被遮蔽 2 · CIDR 重叠 1 · 匹配 18.5µs"
    ));
    assert!(subtree_has_text(app.world(), root, "56 次命中 · 被遮蔽"));

    // The clear button submits the shared reset intent (no UI-local reset).
    let clear_entity = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &ClearRuleHitCountersButton)>();
        buttons
            .iter(world)
            .next()
            .expect("clear hit counters button mounted")
            .0
    };
    app.world_mut().commands().trigger(Activate {
        entity: clear_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ClearRuleHitCounters))
    );
}

fn rules_mrs_test_item(name: &str, behavior: MrsBehaviorKind, rule_count: u32) -> MrsItemSnapshot {
    MrsItemSnapshot {
        name: name.to_owned(),
        behavior,
        format_version: 1,
        compression: MrsCompressionKind::None,
        rule_count,
        payload_size_bytes: 128,
        file_size_bytes: 192,
        sha256_digest: Some("deadbeefcafebabe0123456789abcdef".to_owned()),
        crc32_checksum: Some(1),
        is_mmap_accelerated: true,
        is_valid: true,
        description: String::new(),
        updated_at: "2026-09-06 12:00".to_owned(),
        source_url: None,
        unpack_supported: true,
    }
}

#[test]
fn test_rules_mrs_renders_shared_snapshot_not_fabricated() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.mrs_acceleration = MrsAccelerationSnapshot::ready(
        1,
        1,
        vec![rules_mrs_test_item(
            "custom-test.mrs",
            MrsBehaviorKind::IpCidr,
            1234,
        )],
        true,
    );
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-03: the row is rendered from the shared snapshot, digest included.
    assert!(subtree_has_text(
        app.world(),
        root,
        "custom-test.mrs (1234 条目 · ipcidr · 校验通过 · sha256 deadbeefcafe)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 加速就绪 · 1 个规则集"
    ));
    // The previously hardcoded fabricated item must be gone.
    assert!(!subtree_has_text(app.world(), root, "14,200 条目"));

    // An unsupported snapshot renders its honest typed status, not a fake list.
    let mut unsupported = RulesProjection::demo();
    unsupported.mrs_acceleration =
        MrsAccelerationSnapshot::unsupported(1, 1, "内核未配置规则集提供者网关");
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(unsupported));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 加速不受支持：内核未配置规则集提供者网关"
    ));
}

#[test]
fn test_rules_provider_lifecycle_renders_shared_source_url() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.providers[0].source_url = Some("https://example.com/geo.mrs".to_owned());
    projection.providers[1].source_url = None;
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-04: declared URL is shown; runtime-only providers stay honest.
    assert!(subtree_has_text(
        app.world(),
        root,
        "更新: 2026-09-02 06:00 · 来源: https://example.com/geo.mrs"
    ));
    assert!(subtree_has_text(app.world(), root, "来源: 未声明"));
}

#[test]
fn test_rules_search_hides_non_matching_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);

    let field_entity = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<RuleSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field");
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("github");
    app.update();

    // DUAL-11-13: demo row #1 is DOMAIN-KEYWORD,github; the rest are hidden.
    let mut rows = app.world_mut().query::<(&Node, &RuleRow)>();
    let displays: Vec<(usize, Display)> = rows
        .iter(app.world())
        .map(|(node, row)| (row.0, node.display))
        .collect();
    assert!(displays.contains(&(1, Display::Flex)));
    assert!(displays.contains(&(0, Display::None)));
}

#[test]
fn test_rules_pagination_hides_rows_outside_page() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);

    {
        let mut view = app.world_mut().resource_mut::<RulesViewState>();
        view.page_size = 2;
        view.page = 1;
    }
    app.update();

    // DUAL-11-13: page 2 of size 2 over 5 demo rules shows indices 2 and 3.
    let mut rows = app.world_mut().query::<(&Node, &RuleRow)>();
    let displays: Vec<(usize, Display)> = rows
        .iter(app.world())
        .map(|(node, row)| (row.0, node.display))
        .collect();
    assert!(displays.contains(&(2, Display::Flex)));
    assert!(displays.contains(&(3, Display::Flex)));
    assert!(displays.contains(&(0, Display::None)));

    let indicator = app
        .world_mut()
        .query::<(&Text, &RulesPageIndicator)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .find(|text| text.contains("页"))
        .expect("page indicator");
    assert_eq!(indicator, "第 2/3 页 · 共 5 条");
}

fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}

#[test]
fn test_rules_toggle_and_reorder_submit_shared_intents() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    // DUAL-11-09: row #1 is disabled in the demo fixture; the toggle submits the
    // shared index intent.
    let toggle = app
        .world_mut()
        .query::<(Entity, &RuleToggleButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 1)
        .map(|(entity, _)| entity)
        .expect("row #1 toggle");
    activate(&mut app, toggle);
    assert_eq!(sink.submitted(), vec![UiCommand::ToggleRuleEnabled(1)]);

    // DUAL-11-10: the reorder handles map to the shared move directions.
    let up = app
        .world_mut()
        .query::<(Entity, &RuleMoveUpButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 3)
        .map(|(entity, _)| entity)
        .expect("row #3 move up");
    activate(&mut app, up);
    assert_eq!(sink.submitted().last(), Some(&UiCommand::MoveRuleUp(3)));

    let down = app
        .world_mut()
        .query::<(Entity, &RuleMoveDownButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 0)
        .map(|(entity, _)| entity)
        .expect("row #0 move down");
    activate(&mut app, down);
    assert_eq!(sink.submitted().last(), Some(&UiCommand::MoveRuleDown(0)));
}

#[test]
fn test_rules_add_wizard_submits_shared_draft_and_type_selection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    // DUAL-11-11: pick a non-default type chip; the shared state and caption
    // restamp together.
    let geoip_chip = app
        .world_mut()
        .query::<(Entity, &RuleTypeChip)>()
        .iter(app.world())
        .find(|(_, chip)| {
            infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES
                .get(chip.0)
                .is_some_and(|choice| *choice == "GEOIP")
        })
        .map(|(entity, _)| entity)
        .expect("GEOIP chip");
    activate(&mut app, geoip_chip);
    assert_eq!(
        app.world().resource::<RulesBuilderState>().rule_type,
        "GEOIP"
    );

    // Type the payload and target into the wizard fields.
    let set_field = |app: &mut App, wrapper: fn(&mut App) -> Entity, text: &str| {
        let field = wrapper(app);
        app.world_mut()
            .get_mut::<TextField>(field)
            .expect("wizard field")
            .0 = TextFieldState::new(text);
    };
    set_field(&mut app, payload_field_entity, "CN");
    set_field(&mut app, target_field_entity, "DIRECT");

    let add = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<AddCustomRuleButton>>()
        .single(app.world())
        .expect("add button");
    activate(&mut app, add);
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::AddCustomRule {
            rule_type: "GEOIP".to_owned(),
            payload: "CN".to_owned(),
            target: "DIRECT".to_owned(),
        })
    );
}

fn payload_field_entity(app: &mut App) -> Entity {
    let children: Vec<Entity> = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<RulePayloadField>>()
        .single(app.world())
        .expect("payload wrapper")
        .iter()
        .copied()
        .collect();
    children
        .into_iter()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("payload text field")
}

fn target_field_entity(app: &mut App) -> Entity {
    let children: Vec<Entity> = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<RuleTargetField>>()
        .single(app.world())
        .expect("target wrapper")
        .iter()
        .copied()
        .collect();
    children
        .into_iter()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("target text field")
}

#[test]
fn test_rules_game_presets_submit_shared_target() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    // DUAL-11-12: the target field defaults to the shared constant; the inject
    // button forwards whatever the field holds.
    let target = target_field_entity(&mut app);
    assert_eq!(
        app.world()
            .get::<TextField>(target)
            .expect("target")
            .0
            .text(),
        infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET
    );
    app.world_mut()
        .get_mut::<TextField>(target)
        .expect("target")
        .0 = TextFieldState::new("Game-Proxy");

    let inject = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<InjectGamePresetsButton>>()
        .single(app.world())
        .expect("inject button");
    activate(&mut app, inject);
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::ApplyGameRoutingPresets {
            target: "Game-Proxy".to_owned(),
        })
    );

    // The wizard caption is a real i18n-free bare-Chinese label on Bevy.
    let caption = app
        .world_mut()
        .query::<(&Text, &RuleBuilderSelection)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .next()
        .expect("selection caption");
    assert!(caption.contains("已选类型"));
}

#[test]
fn test_rules_matrix_covers_closed_items() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // 11-01: rule types are projected generically, so any type renders from the
    // shared read model without a per-type branch.
    let mut projection = RulesProjection::demo();
    projection.rules[0].rule_type = "PROCESS-NAME".to_owned();
    projection.rules[1].rule_type = "GEOIP".to_owned();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "[PROCESS-NAME]"));
    assert!(subtree_has_text(app.world(), root, "[GEOIP]"));

    // 11-03/04/13: shared MRS card, provider lifecycle and search/paging.
    assert!(subtree_has_text(app.world(), root, "MRS 加速就绪"));
    assert!(subtree_has_text(app.world(), root, "来源:"));
    assert!(subtree_has_text(app.world(), root, "第 1/1 页"));

    // 11-09/10: one toggle + one reorder pair per mounted rule row.
    assert_eq!(count_with::<RuleToggleButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveUpButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveDownButton>(&mut app), 5);

    // 11-11/12: the wizard exposes the shared type vocabulary + both actions.
    assert_eq!(
        count_with::<RuleTypeChip>(&mut app),
        infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES.len()
    );
    assert_eq!(count_with::<AddCustomRuleButton>(&mut app), 1);
    assert_eq!(count_with::<InjectGamePresetsButton>(&mut app), 1);
}

/// Count mounted entities carrying a marker component.
fn count_with<T: bevy::ecs::component::Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<T>>()
        .iter(app.world())
        .count()
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

#[test]
fn test_proxies_toggle_group_expand_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let fold_btn = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &ProxyGroupFoldButton)>();
        buttons.iter(world).next().expect("fold button mounted").0
    };

    app.world_mut()
        .commands()
        .trigger(Activate { entity: fold_btn });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::ToggleProxyGroupExpand { group } if !group.is_empty()
    )));
}

#[test]
fn test_proxies_filter_alive_toggle_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let toggle_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<FilterAliveToggle>>()
        .single(app.world())
        .expect("filter alive toggle");

    app.world_mut().commands().trigger(Activate {
        entity: toggle_entity,
    });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ToggleFilterAlive(true)]);
}

#[test]
fn test_proxies_sort_pills_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let pill_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &ProxySortPill)>();
        query.iter(world).next().expect("sort pill").0
    };

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::SetProxySortOrder(_)))
    );
}

#[test]
fn test_proxies_favorite_pin_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    let pin_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &NodePinButton)>();
        query.iter(world).next().expect("pin button").0
    };

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pin_entity });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::ToggleFavoriteProxy(_)))
    );
}

#[test]
fn test_proxies_advanced_chips_and_color_ladder() {
    assert_eq!(
        infiltrator_bevy_ui::pages::proxies_filter::format_protocol_chip("Shadowsocks"),
        "Shadowsocks"
    );
    assert_eq!(
        infiltrator_bevy_ui::pages::proxies_filter::format_protocol_chip("vless"),
        "Vless"
    );
    assert_eq!(
        infiltrator_bevy_ui::pages::proxies_filter::format_protocol_chip("hy2"),
        "Hysteria2"
    );

    let (label, tier) = infiltrator_bevy_ui::pages::proxies::format_latency(Some(45));
    assert_eq!(label, "45 ms");
    assert_eq!(tier, infiltrator_bevy_ui::pages::proxies::LatencyTier::Fast);

    let (label, tier) = infiltrator_bevy_ui::pages::proxies::format_latency(Some(0));
    assert_eq!(label, "超时");
    assert_eq!(
        tier,
        infiltrator_bevy_ui::pages::proxies::LatencyTier::Timeout
    );
}

#[test]
fn test_proxies_pinyin_fuzzy_and_protocol_filtering() {
    let node = infiltrator_bevy_ui::pages::proxies::ProxyNode {
        name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
        node_type: "VLESS".to_owned(),
        delay_ms: Some(45),
        selected: true,
        favorite: true,
        features: vec!["Reality".to_owned(), "Vision".to_owned()],
    };

    assert!(infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "xg"));
    assert!(infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "hk"));
    assert!(infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "vless"));
    assert!(infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "reality"));
    assert!(infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "<100"));
    assert!(!infiltrator_bevy_ui::pages::proxies_filter::matches_proxy_filter(&node, "日本"));
}

#[test]
fn test_proxies_node_detail_drawer_and_group_reorder() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    let _ = navigate_to(&mut app, Route::Proxies);

    // 1. Detail button activation (DUAL-04-11)
    let detail_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(
            Entity,
            &infiltrator_bevy_ui::pages::proxies::NodeDetailButton,
        )>();
        query
            .iter(world)
            .next()
            .expect("node detail button mounted")
            .0
    };

    app.world_mut().commands().trigger(Activate {
        entity: detail_entity,
    });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::SelectProxyNode { group, node } if group == "PROXIES" && !node.is_empty()
    )));

    // 2. Group move up activation (DUAL-04-12)
    let move_up_entity = {
        let world = app.world_mut();
        let mut query = world.query::<(
            Entity,
            &infiltrator_bevy_ui::pages::proxies::ProxyGroupMoveUpButton,
        )>();
        query.iter(world).next().expect("move up button mounted").0
    };

    app.world_mut().commands().trigger(Activate {
        entity: move_up_entity,
    });
    app.update();

    assert!(sink.submitted().iter().any(|cmd| matches!(
        cmd,
        UiCommand::ReorderProxyGroups { group_names } if !group_names.is_empty()
    )));

    // 3. Reset group order activation (DUAL-04-12)
    let reset_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<infiltrator_bevy_ui::pages::proxies::ResetProxyGroupOrderButton>>()
        .single(app.world())
        .expect("reset proxy group order button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: reset_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::ResetProxyGroupOrder))
    );

    // 4. Toggle compact view activation (DUAL-04-13)
    let toggle_view_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<infiltrator_bevy_ui::pages::proxies::ToggleViewModeButton>>()
        .single(app.world())
        .expect("toggle view mode button mounted");

    app.world_mut().commands().trigger(Activate {
        entity: toggle_view_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|cmd| matches!(cmd, UiCommand::SetProxyCompactView(true)))
    );

    // 5. Latency skeleton pulse mounted (DUAL-04-14)
    assert!(
        app.world_mut()
            .query::<&infiltrator_bevy_ui::pages::proxies::LatencySkeletonPulse>()
            .iter(app.world())
            .next()
            .is_some(),
        "latency skeleton pulse component must be mounted"
    );

    // 6. Full Group 04 regression matrix verification (DUAL-04-15)
    let report =
        infiltrator_contract::proxies::ProxyRegressionMatrixReport::run_deterministic_matrix();
    assert!(report.is_all_passed());
    assert_eq!(report.total_scenarios, 15);
}

// ---- DUAL-07-02/04/12: subscription fetch options dual surface ---------------

fn subscription_fetch_projection() -> ProfilesProjection {
    ProfilesProjection {
        auto_update_interval_hours: 12,
        updating: false,
        profiles: vec![ProfileItem {
            id: "sub-fetch".to_owned(),
            name: "抓取选项订阅".to_owned(),
            url: "https://fetch.example/sub".to_owned(),
            updated_at: "2026-09-22 09:00".to_owned(),
            upload_bytes: 0,
            download_bytes: 0,
            total_bytes: 0,
            is_active: true,
            user_agent: "ClashVerge/2.0".to_owned(),
            insecure_skip_verify: true,
            etag: Some("\"fetch-etag\"".to_owned()),
            last_modified: Some("Tue, 22 Sep 2026 09:00:00 GMT".to_owned()),
            has_backup: true,
            cron_expression: Some("0 */6 * * *".to_owned()),
            filter: infiltrator_contract::subscription_import::SubscriptionFilterDraft {
                include: "香港".to_owned(),
                exclude: "广告".to_owned(),
                ..Default::default()
            },
        }],
    }
}

#[test]
fn test_profiles_fetch_options_projection_restamps_ua_insecure_and_validators() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "ClashVerge/2.0"),
        "active profile User-Agent reaches the text field"
    );
    assert!(
        subtree_has_text(app.world(), root, "条件请求已缓存"),
        "cached ETag / Last-Modified reaches the status line"
    );
    let insecure_checked = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionInsecureToggle, &Children)>();
        let (_, children) = toggles.single(app.world()).expect("insecure toggle");
        children
            .iter()
            .any(|child| app.world().get::<bevy::ui::Checked>(*child).is_some())
    };
    assert!(
        insecure_checked,
        "insecure-TLS toggle reflects the projection"
    );
}

#[test]
fn test_profiles_save_fetch_settings_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    let field = {
        let mut fields = app
            .world_mut()
            .query::<(&SubscriptionUserAgentField, &Children)>();
        *fields
            .single(app.world())
            .expect("ua field wrapper")
            .1
            .iter()
            .next()
            .expect("ua text field")
    };
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("ua field state")
        .0
        .apply(TextFieldInput::SetText("Custom-UA/9".to_owned()));

    let save = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SaveUserAgentButton>>()
        .single(app.world())
        .expect("save button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SaveSubscriptionFetchSettings {
            profile_id: "sub-fetch".to_owned(),
            user_agent: Some("Custom-UA/9".to_owned()),
            insecure_skip_verify: true,
        }]
    );
}

/// Overwrite the single-marker text field's state in a headless app.
fn set_marker_text<M: bevy::ecs::component::Component>(app: &mut App, value: &str) {
    let field = {
        let mut wrappers = app
            .world_mut()
            .query_filtered::<&Children, bevy::ecs::query::With<M>>();
        *wrappers
            .single(app.world())
            .expect("field wrapper")
            .iter()
            .next()
            .expect("text field child")
    };
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("text field state")
        .0
        .apply(TextFieldInput::SetText(value.to_owned()));
}

fn marker_entity<M: bevy::ecs::component::Component>(app: &mut App) -> Entity {
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<M>>();
    query.single(app.world()).expect("marker entity")
}

// ---- DUAL-07-03/08: cron schedule + node cleaning pipeline dual surface ------

#[test]
fn test_profiles_filter_panel_restamps_and_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "香港"),
        "stored include filter restamps onto the panel"
    );
    assert!(
        subtree_has_text(app.world(), root, "清洗管道"),
        "filter pipeline status line renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "0 */6 * * *"),
        "cron schedule is visible on the import card (DUAL-07-03)"
    );

    set_marker_text::<SubscriptionFilterIncludeField>(&mut app, "香港, 日本");
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SaveSubscriptionFilter {
            profile_id: "sub-fetch".to_owned(),
            filter: infiltrator_contract::subscription_import::SubscriptionFilterDraft {
                include: "香港, 日本".to_owned(),
                exclude: "广告".to_owned(),
                dedup_index: 0,
                ..Default::default()
            },
        }],
        "the filter editor rides the shared command (shared pipeline)"
    );
}

#[test]
fn test_profiles_import_channels_submit_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    set_marker_text::<ImportSubscriptionNameField>(&mut app, "new-sub");
    set_marker_text::<ImportSubscriptionUrlField>(&mut app, "https://example.com/sub");
    set_marker_text::<ImportLocalPathField>(&mut app, "/tmp/local.yaml");

    let url_button = marker_entity::<ImportSubscriptionUrlButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: url_button });
    app.update();
    let local_button = marker_entity::<ImportLocalSubscriptionButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: local_button,
    });
    app.update();
    let clipboard_button = marker_entity::<ImportClipboardSubscriptionButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: clipboard_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel: infiltrator_contract::subscription_import::SubscriptionImportChannel::Url,
                source: "https://example.com/sub".to_owned(),
            },
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel:
                    infiltrator_contract::subscription_import::SubscriptionImportChannel::LocalFile,
                source: "/tmp/local.yaml".to_owned(),
            },
            UiCommand::ImportSubscription {
                profile_id: "new-sub".to_owned(),
                channel:
                    infiltrator_contract::subscription_import::SubscriptionImportChannel::Clipboard,
                source: String::new(),
            },
        ],
        "all three import channels route through the shared command bus"
    );
}

// ---- DUAL-07-11/13: batch update + safe backup dual surface -------------------

#[test]
fn test_profiles_update_all_toolbar_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    assert!(
        subtree_has_text(app.world(), root, "一键更新全部订阅"),
        "toolbar exposes the batch update entry"
    );

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<UpdateAllSubscriptionsButton>>()
        .single(app.world())
        .expect("update-all button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::UpdateAllSubscriptions]);
}

#[test]
fn test_profiles_restore_backup_submits_shared_command_and_restamps_status() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "安全备份已就绪"),
        "available backup reaches the status line"
    );

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<RestoreSubscriptionBackupButton>>()
        .single(app.world())
        .expect("restore backup button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RestoreSubscriptionBackup {
            id: "sub-fetch".to_owned(),
        }]
    );

    let mut no_backup = subscription_fetch_projection();
    no_backup.profiles[0].has_backup = false;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(no_backup));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "安全备份：暂无"),
        "missing backup restamps the status line"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<SubscriptionBackupStatus>>()
            .iter(app.world())
            .next()
            .is_some(),
        "backup status marker is mounted"
    );
}
