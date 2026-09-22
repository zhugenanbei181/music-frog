//! Headless integration tests for Page Matrix A (Proxies, Profiles, Connections, Logs, Rules):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - Empty lists, boundary conditions, and defensive rendering.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ui::Checked;
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
use infiltrator_bevy_ui::pages::profiles_aggregator::{
    AddAggregatorCustomGroupButton, AggregatorCustomGroupKeywordsField,
    AggregatorCustomGroupNameField, AggregatorNameField, AggregatorRenamesField,
    AggregatorSourceToggle, AggregatorSwitch, AggregatorSwitchKind, AggregatorTemplateNameField,
    DeleteAggregationTemplateButton, PreviewAggregationButton, ReAggregateTemplateButton,
    SaveAggregatedProfileButton, SaveAggregationTemplateButton, UseAggregationTemplateButton,
};
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
use infiltrator_bevy_ui::pages::profiles_subscription_policy::{
    SaveSubscriptionAutoReloadButton, SaveSubscriptionPolicyButton, SubscriptionAutoReloadToggle,
    SubscriptionPolicyCronField, SubscriptionPolicyIntervalField,
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
use infiltrator_bevy_ui::pages::rules_projection::{RuleTypeBadge, RuleTypeText};
use infiltrator_bevy_ui::pages::rules_subrules::{
    RulesSubRuleState, SubRuleConditionRow, SubRuleInsertButton, SubRuleOperatorChip,
    SubRulePresetButton, SubRulePreviewLine, SubRuleRemoveConditionButton, SubRuleTargetField,
    preview_label,
};
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

/// Test-controlled shared surface source: the router mounts exactly this
/// snapshot, so a page can be exercised with a hand-built read model.
struct StaticRulesSurface {
    snapshot: infiltrator_contract::surface_snapshot::SurfaceSnapshot,
}

impl infiltrator_bevy_ui::projection::OverviewSource for StaticRulesSurface {
    fn current(&self) -> infiltrator_bevy_ui::projection::OverviewProjection {
        infiltrator_bevy_ui::surface::overview_projection(&self.snapshot)
    }

    fn kind(&self) -> infiltrator_bevy_ui::projection::SourceKind {
        infiltrator_bevy_ui::projection::SourceKind::LiveCore
    }
}

impl infiltrator_bevy_ui::surface::SurfaceSource for StaticRulesSurface {
    fn surface_snapshot(&self) -> infiltrator_contract::surface_snapshot::SurfaceSnapshot {
        self.snapshot.clone()
    }
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
        "自动更新: 2 个订阅已启用 · 最短周期 6 小时"
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
    assert!(subtree_has_text(app.world(), root, "预览聚合结果"));
    assert!(subtree_has_text(app.world(), root, "保存为新配置"));
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
    updated.auto_update_interval_hours = 3;
    updated.profiles[0].is_active = false;
    updated.profiles[1].is_active = true;
    updated.profiles[1].name = "备用容灾线路 (Active Live)".to_owned();
    updated.profiles[1].update_interval_hours = Some(3);

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
        "自动更新: 2 个订阅已启用 · 最短周期 3 小时"
    ));
    assert!(subtree_has_text(app.world(), root, "定时计划: 每 3 小时"));
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
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
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
    assert!(subtree_has_text(app.world(), root, "自动更新: 未启用"));
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
        truncated_rule_count: None,
        rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
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
    projection.providers[0].refresh_interval_secs = Some(86_400);
    projection.providers[1].source_url = None;
    projection.providers[1].refresh_interval_secs = None;
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-04: declared URL is shown; runtime-only providers stay honest.
    assert!(subtree_has_text(
        app.world(),
        root,
        "更新: 2026-09-02 06:00 · 来源: https://example.com/geo.mrs · 自动刷新: 1d (内核调度)"
    ));
    assert!(subtree_has_text(app.world(), root, "来源: 未声明"));
    // DUAL-11-05: the declared automatic-refresh interval is rendered as the
    // kernel-scheduled fact; no cache hit/miss state is invented.
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动刷新: 1d (内核调度)"
    ));
    assert!(subtree_has_text(app.world(), root, "自动刷新: 未声明"));
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

    // 11-01: rule types render their shared catalogue label.
    let mut projection = RulesProjection::demo();
    projection.rules[0].rule_type = "PROCESS-NAME".to_owned();
    projection.rules[1].rule_type = "GEOIP".to_owned();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "[ProcessName]"));
    assert!(subtree_has_text(app.world(), root, "[GeoIP]"));
    // The typed chip behind every row label is mounted from the shared family.
    assert_eq!(count_with::<RuleTypeBadge>(&mut app), 5);

    // 11-03/04/13: shared MRS card, provider lifecycle and search/paging.
    assert!(subtree_has_text(app.world(), root, "MRS 加速就绪"));
    assert!(subtree_has_text(app.world(), root, "来源:"));
    assert!(subtree_has_text(app.world(), root, "第 1/1 页"));
    // 11-08: a complete list carries no truncation note.
    assert!(!subtree_has_text(app.world(), root, "发布视口已截断"));

    // 11-09/10: one toggle + one reorder pair per mounted rule row.
    assert_eq!(count_with::<RuleToggleButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveUpButton>(&mut app), 5);
    assert_eq!(count_with::<RuleMoveDownButton>(&mut app), 5);

    // 11-11/12: the wizard exposes the shared type vocabulary + both actions.
    assert_eq!(count_with::<AddCustomRuleButton>(&mut app), 1);
    assert_eq!(count_with::<InjectGamePresetsButton>(&mut app), 1);

    // 11-02: the logical builder is mounted with the shared vocabulary, the
    // shared default draft and the shared preview expression.
    assert_eq!(
        count_with::<SubRuleOperatorChip>(&mut app),
        infiltrator_domain::rules::logical::LOGICAL_OPERATOR_CHOICES.len()
    );
    assert_eq!(
        count_with::<SubRulePresetButton>(&mut app),
        infiltrator_domain::rules::logical::SUB_RULE_CONDITION_PRESETS.len()
    );
    assert_eq!(count_with::<SubRuleConditionRow>(&mut app), 2);
    assert_eq!(count_with::<SubRuleInsertButton>(&mut app), 1);
    let mut preview_lines = app.world_mut().query::<(&Text, &SubRulePreviewLine)>();
    let preview = preview_lines
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .next()
        .expect("sub-rule preview line");
    assert_eq!(
        preview,
        preview_label(&infiltrator_domain::rules::logical::default_logical_draft(
            infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET
        ))
    );
    assert!(preview.contains("AND((DOMAIN-SUFFIX,company.com),(NETWORK,TCP),PROXY)"));
}

/// DUAL-11-01: every catalogue spelling mounts with its shared label, and the
/// chip fill follows the shared family rather than a per-surface spelling list.
#[test]
fn test_rules_type_matrix_renders_every_shared_label() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let source = infiltrator_bevy_ui::surface::DemoSurfaceSource::running();
    let mut snapshot = infiltrator_bevy_ui::surface::SurfaceSource::surface_snapshot(&source);
    let rules: Vec<infiltrator_contract::surface_snapshot::RuleSnapshot> =
        infiltrator_domain::rules::matrix::RULE_TYPE_MATRIX
            .iter()
            .enumerate()
            .map(
                |(index, spec)| infiltrator_contract::surface_snapshot::RuleSnapshot {
                    id: index + 1,
                    rule_type: spec.name.to_owned(),
                    payload: "payload".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: 0,
                    is_enabled: true,
                    no_resolve: spec.accepts_no_resolve,
                    last_hit_secs: None,
                    is_shadowed: false,
                    shadow_reason: None,
                },
            )
            .collect();
    let matrix_len = rules.len();
    snapshot.pages.rules = infiltrator_contract::surface_snapshot::PageData::ready(
        infiltrator_contract::surface_snapshot::RulesPageSnapshot {
            total_rules: matrix_len,
            default_action: "DIRECT".to_owned(),
            providers: Vec::new(),
            rules,
            tracer: Default::default(),
            mrs_acceleration: Default::default(),
            total_hits: 0,
            rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
        },
    );

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new_surface(StaticRulesSurface { snapshot }));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let expected: Vec<String> = infiltrator_domain::rules::matrix::RULE_TYPE_MATRIX
        .iter()
        .map(|spec| format!("[{}]", spec.label))
        .collect();
    for label in expected {
        assert!(
            subtree_has_text(app.world(), root, &label),
            "missing rendered label {label}"
        );
    }
    assert_eq!(count_with::<RuleTypeText>(&mut app), matrix_len);
    assert_eq!(count_with::<RuleTypeBadge>(&mut app), matrix_len);
}

/// DUAL-11-02: the Bevy logical builder delegates every mutation to the shared
/// draft and submits the shared canonical expression through the command bus.
#[test]
fn test_rules_subrules_builder_submits_shared_logical_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // A preset condition is appended by the shared reduction; the rebuilt list
    // shows the new row and the preview reflects the canonical expression.
    let preset = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SubRulePresetButton>>()
        .iter(app.world())
        .next()
        .expect("preset button");
    activate(&mut app, preset);
    let draft = app.world().resource::<RulesSubRuleState>().draft.clone();
    assert_eq!(draft.conditions.len(), 3);
    assert_eq!(
        draft.conditions[2],
        infiltrator_domain::rules::logical::SUB_RULE_CONDITION_PRESETS[0]
    );
    assert_eq!(count_with::<SubRuleConditionRow>(&mut app), 3);

    // Selecting an operator goes through the shared vocabulary.
    let not_chip = app
        .world_mut()
        .query::<(Entity, &SubRuleOperatorChip)>()
        .iter(app.world())
        .find(|(_, chip)| {
            infiltrator_domain::rules::logical::LOGICAL_OPERATOR_CHOICES
                .get(chip.0)
                .is_some_and(|operator| *operator == "NOT")
        })
        .map(|(entity, _)| entity)
        .expect("NOT chip");
    activate(&mut app, not_chip);
    assert_eq!(
        app.world().resource::<RulesSubRuleState>().draft.operator,
        "NOT"
    );

    // NOT accepts exactly one condition, so the insert is gated by the shared
    // builder and nothing is submitted while the draft is invalid.
    let insert = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SubRuleInsertButton>>()
        .iter(app.world())
        .next()
        .expect("insert button");
    activate(&mut app, insert);
    assert!(sink.submitted().is_empty());
    assert!(subtree_has_text(app.world(), root, "校验未通过"));

    // Removing conditions down to one lets the shared builder build.
    while app
        .world()
        .resource::<RulesSubRuleState>()
        .draft
        .conditions
        .len()
        > 1
    {
        let remove = app
            .world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<SubRuleRemoveConditionButton>>()
            .iter(app.world())
            .next()
            .expect("remove button");
        activate(&mut app, remove);
        app.update();
    }

    // The target field is the live source of the composition target.
    let target = app
        .world_mut()
        .query_filtered::<&Children, bevy::ecs::query::With<SubRuleTargetField>>()
        .single(app.world())
        .expect("target wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("target text field");
    app.world_mut()
        .get_mut::<TextField>(target)
        .expect("target field")
        .0 = TextFieldState::new("AI");

    activate(&mut app, insert);
    app.update();
    let draft = app.world().resource::<RulesSubRuleState>().draft.clone();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::AddCustomRule {
            rule_type: "NOT".to_owned(),
            payload: infiltrator_domain::rules::logical::draft_payload(&draft.conditions),
            target: "AI".to_owned(),
        })
    );
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::AddCustomRule {
            rule_type: "NOT".to_owned(),
            payload: "(DOMAIN-SUFFIX,google.com)".to_owned(),
            target: "AI".to_owned(),
        })
    );
}

/// DUAL-11-08: the truncation fact is rendered when the published list is a
/// capped view of the profile list, and the paging indicator keeps its bounds.
#[test]
fn test_rules_truncation_note_reports_publish_cap() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.total_rules = 50_000;
    projection.truncated_rule_count = Some(45_000);
    projection.rule_publish_limit = infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "发布视口已截断 · 已省略 45000 条 (发布上限 5000 条，非 O(1) 虚拟滚动)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 50000 条规则 (3 个规则集 / 命中统计开启)"
    ));
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
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
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
            auto_update_enabled: true,
            update_interval_hours: Some(6),
            next_update: None,
            auto_reload_core: true,
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

// ---- DUAL-07-09/14: subscription policy + auto-reload + delete --------------

/// DUAL-07-14: the card actions delete a profile through the shared command
/// bus, and the active profile's delete action is never submitted — exactly
/// like the Iced card, which only offers the action for inactive profiles.
#[test]
fn test_profiles_delete_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    let active = app
        .world_mut()
        .query::<(Entity, &DeleteProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.profile_id == "sub-1")
        .map(|(entity, _)| entity)
        .expect("sub-1 delete button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: active });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "the active profile is never deleted from this surface"
    );

    let inactive = app
        .world_mut()
        .query::<(Entity, &DeleteProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.profile_id == "sub-2")
        .map(|(entity, _)| entity)
        .expect("sub-2 delete button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: inactive });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::DeleteProfile {
            id: "sub-2".to_owned(),
        }],
        "delete routes through the shared command"
    );
}

/// DUAL-07-14: the update-policy card is driven by the shared projection and
/// submits the whole schedule draft through the shared command bus.
#[test]
fn test_profiles_schedule_policy_restamps_and_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut projection = subscription_fetch_projection();
    projection.profiles[0].url = "https://fetch.example/sub".to_owned();
    projection.profiles[0].auto_update_enabled = true;
    projection.profiles[0].update_interval_hours = Some(12);
    projection.profiles[0].auto_reload_core = true;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "订阅更新策略 (Update Policy)"),
        "the policy card mounts on the profiles page"
    );
    assert!(
        subtree_has_text(app.world(), root, "更新策略：按 Cron `0 */6 * * *` 排程"),
        "the status line restamps from the shared snapshot"
    );
    assert!(
        subtree_has_text(app.world(), root, "保存更新策略"),
        "the policy save action exists"
    );

    // The interval field is prefilled from the snapshot, then edited.
    set_marker_text::<SubscriptionPolicyIntervalField>(&mut app, "6");
    set_marker_text::<SubscriptionPolicyCronField>(&mut app, "");
    let save = marker_entity::<SaveSubscriptionPolicyButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSubscriptionSchedule {
            profile_id: "sub-fetch".to_owned(),
            draft: infiltrator_contract::subscription_import::SubscriptionScheduleDraft {
                url: "https://fetch.example/sub".to_owned(),
                auto_update_enabled: true,
                update_interval_hours: "6".to_owned(),
                cron_expression: None,
            },
        }],
        "the edited schedule rides the shared command"
    );
}

/// DUAL-07-09: the auto-reload control mirrors the shared projection and
/// submits the shared command; the checkbox follows the persisted value.
#[test]
fn test_profiles_auto_reload_toggle_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut projection = subscription_fetch_projection();
    projection.profiles[0].auto_reload_core = true;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "更新后自动重载内核"),
        "the reload control exists on the profile management surface"
    );
    assert!(
        subtree_has_text(app.world(), root, "内核重载：更新成功后重载内核"),
        "the reload status line renders the persisted preference"
    );
    let checked = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionAutoReloadToggle, &Children)>();
        let (_, children) = toggles.single(app.world()).expect("reload toggle");
        children
            .iter()
            .any(|child| app.world().get::<bevy::ui::Checked>(*child).is_some())
    };
    assert!(checked, "the checkbox reflects the shared snapshot");

    // Untick + save: the command carries the edited preference.
    if let Some(child) = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionAutoReloadToggle, &Children)>();
        toggles
            .single(app.world())
            .expect("reload toggle")
            .1
            .iter()
            .next()
            .copied()
    } {
        app.world_mut()
            .entity_mut(child)
            .remove::<bevy::ui::Checked>();
    }
    let save = marker_entity::<SaveSubscriptionAutoReloadButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSubscriptionAutoReload {
            profile_id: "sub-fetch".to_owned(),
            enabled: false,
        }],
        "the reload preference rides the shared command"
    );
}

// ---- DUAL-08: aggregator wizard + shared preview dual surface ---------------

/// DUAL-08: a report carrying real region clusters and a master cascade.
fn aggregation_preview_report() -> infiltrator_contract::aggregator::AggregationReport {
    use infiltrator_contract::aggregator::{
        AggregationReport, GeneratedGroupSnapshot, RegionalClusterSnapshot,
    };
    AggregationReport {
        draft: infiltrator_contract::aggregator::AggregationDraft {
            source_profiles: vec![
                "主力高速订阅 (Primary VIP)".to_owned(),
                "备用容灾线路 (Backup Anycast)".to_owned(),
                "局域网调试配置 (LAN Lab)".to_owned(),
            ],
            target_name: "Merged-All".to_owned(),
            deduplicate: true,
            deduplicate_names: true,
            geo_cluster: true,
            generate_groups: true,
            remove_emojis: true,
            rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
                pattern: "-广告$".to_owned(),
                replacement: String::new(),
            }],
            custom_groups: vec![infiltrator_contract::aggregator::AggregationCustomGroup {
                name: "流媒体专用".to_owned(),
                group_type: "select".to_owned(),
                member_keywords: vec!["香港".to_owned()],
            }],
            availability_precheck: true,
            activate_after_create: false,
        },
        source_count: 3,
        missing_sources: vec![],
        input_nodes: 32,
        total_nodes: 30,
        duplicates_removed: 2,
        renamed_nodes: 30,
        rule_renamed_nodes: 4,
        invalid_nodes_removed: 2,
        invalid_node_samples: vec!["广告节点: trojan: password is required".to_owned()],
        regions: vec![
            RegionalClusterSnapshot {
                iso: "HK".to_owned(),
                label: "香港".to_owned(),
                flag: "🇭🇰".to_owned(),
                group_name: "香港自动测速".to_owned(),
                node_names: vec!["香港 01".to_owned(), "香港 02".to_owned()],
            },
            RegionalClusterSnapshot {
                iso: "JP".to_owned(),
                label: "日本".to_owned(),
                flag: "🇯🇵".to_owned(),
                group_name: "日本自动测速".to_owned(),
                node_names: vec!["东京 01".to_owned()],
            },
        ],
        groups: vec![
            GeneratedGroupSnapshot {
                name: "🚀 节点选择".to_owned(),
                group_type: "select".to_owned(),
                is_master: true,
                is_custom: false,
                members: vec!["♻️ 自动选择".to_owned(), "香港自动测速".to_owned()],
            },
            GeneratedGroupSnapshot {
                name: "香港自动测速".to_owned(),
                group_type: "url-test".to_owned(),
                is_master: false,
                is_custom: false,
                members: vec!["香港 01".to_owned()],
            },
            GeneratedGroupSnapshot {
                name: "流媒体专用".to_owned(),
                group_type: "select".to_owned(),
                is_master: false,
                is_custom: true,
                members: vec!["香港 01".to_owned(), "香港 02".to_owned()],
            },
        ],
        yaml: "port: 7890\nproxies: []\nproxy-groups:\n  - name: 🚀 节点选择\n".to_owned(),
        generated_at: "2026-09-22T10:00:00+00:00".to_owned(),
    }
}

fn aggregation_page_projection() -> ProfilesProjection {
    ProfilesProjection {
        profiles: vec![],
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: Some(aggregation_preview_report()),
        aggregation_templates: vec![infiltrator_contract::aggregator::AggregationTemplate {
            name: "已保存模板".to_owned(),
            draft: infiltrator_contract::aggregator::AggregationDraft {
                source_profiles: vec!["主力高速订阅 (Primary VIP)".to_owned()],
                target_name: "Template-Target".to_owned(),
                deduplicate: false,
                deduplicate_names: true,
                geo_cluster: false,
                generate_groups: true,
                remove_emojis: false,
                rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
                    pattern: "-广告$".to_owned(),
                    replacement: String::new(),
                }],
                custom_groups: Vec::new(),
                availability_precheck: false,
                activate_after_create: true,
            },
            updated_at: "2026-09-22T11:00:00+00:00".to_owned(),
        }],
        aggregation_templates_available: true,
    }
}

/// Flip the `Checked` state of one source row by profile name.
fn set_source_checked(app: &mut App, name: &str, checked: bool) {
    let child = {
        let mut query = app
            .world_mut()
            .query::<(&AggregatorSourceToggle, &Children)>();
        let mut found = None;
        for (toggle, children) in query.iter(app.world()) {
            if toggle.1 == name {
                found = children.iter().next().copied();
                break;
            }
        }
        found.expect("source toggle row")
    };
    let mut entity = app.world_mut().entity_mut(child);
    if checked {
        entity.insert(Checked);
    } else {
        entity.remove::<Checked>();
    }
}

/// Flip the `Checked` state of one named wizard switch.
fn set_switch_checked(app: &mut App, kind: AggregatorSwitchKind, checked: bool) {
    let child = {
        let mut query = app.world_mut().query::<(&AggregatorSwitch, &Children)>();
        let mut found = None;
        for (switch, children) in query.iter(app.world()) {
            if switch.0 == kind {
                found = children.iter().next().copied();
                break;
            }
        }
        found.expect("switch row")
    };
    let mut entity = app.world_mut().entity_mut(child);
    if checked {
        entity.insert(Checked);
    } else {
        entity.remove::<Checked>();
    }
}

/// DUAL-08: the aggregator card projects the shared report and submits the
/// complete edited draft through the shared command bus.
#[test]
fn test_profiles_aggregator_previews_and_saves_through_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(aggregation_page_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "多订阅节点聚合器 (Profile Aggregator)"),
        "the aggregator card mounts on the profiles page"
    );
    // The preview is the shared report, never a local fabrication.
    assert!(
        subtree_has_text(app.world(), root, "🇭🇰 HK 香港 → 香港自动测速（2 节点）"),
        "the region cluster line restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "🚀 节点选择 [主选择器]"),
        "the master cascade line restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "去重 2"),
        "the dedup counter restamps from the shared report"
    );
    // DUAL-08-09/08-11/08-13: the new preview lines all restamp from the
    // shared report and the projected template library.
    assert!(
        subtree_has_text(app.world(), root, "预检剔除 2"),
        "the precheck counter restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "聚合 YAML 结构（共 4 行）"),
        "the YAML viewport renders the shared document"
    );
    assert!(
        subtree_has_text(app.world(), root, "流媒体专用 [自定义]"),
        "the custom group rides the shared group cascade"
    );
    assert!(
        subtree_has_text(app.world(), root, "已保存模板 → Template-Target"),
        "the template library restamps from the shared projection"
    );

    // The card's switches are user input (the projection supplies the report),
    // so the test states them explicitly: everything on except activation.
    set_switch_checked(&mut app, AggregatorSwitchKind::ActivateAfterCreate, false);
    set_marker_text::<AggregatorRenamesField>(&mut app, "-广告$ => ");
    set_marker_text::<AggregatorNameField>(&mut app, "Merged-New");
    set_source_checked(&mut app, "备用容灾线路 (Backup Anycast)", false);
    set_source_checked(&mut app, "局域网调试配置 (LAN Lab)", false);

    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();

    let expected = infiltrator_contract::aggregator::AggregationDraft {
        source_profiles: vec!["主力高速订阅 (Primary VIP)".to_owned()],
        target_name: "Merged-New".to_owned(),
        deduplicate: true,
        deduplicate_names: true,
        geo_cluster: true,
        generate_groups: true,
        remove_emojis: true,
        rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
            pattern: "-广告$".to_owned(),
            replacement: String::new(),
        }],
        custom_groups: Vec::new(),
        availability_precheck: true,
        activate_after_create: false,
    };
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::PreviewProfileAggregation {
            draft: expected.clone(),
        }],
        "the edited draft rides the shared preview command"
    );

    let save = marker_entity::<SaveAggregatedProfileButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::CreateAggregatedProfile {
            draft: expected.clone(),
        }),
        "the save action rides the shared create command"
    );
    sink.clear();

    // DUAL-08-10: the appended custom group joins the submitted draft.
    set_marker_text::<AggregatorCustomGroupNameField>(&mut app, "游戏专用");
    set_marker_text::<AggregatorCustomGroupKeywordsField>(&mut app, "LAN, 调试");
    let add = marker_entity::<AddAggregatorCustomGroupButton>(&mut app);
    app.world_mut().commands().trigger(Activate { entity: add });
    app.update();
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "自定义策略组：游戏专用 · select · LAN, 调试"
        ),
        "the appended custom group restamps onto the composer line"
    );
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let mut with_group = expected.clone();
    with_group.custom_groups = vec![infiltrator_contract::aggregator::AggregationCustomGroup {
        name: "游戏专用".to_owned(),
        group_type: "select".to_owned(),
        member_keywords: vec!["LAN".to_owned(), "调试".to_owned()],
    }];
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: with_group.clone(),
        }),
        "the custom group rides the shared preview command"
    );
    sink.clear();

    // DUAL-08-12: the activation switch rides the shared draft.
    set_switch_checked(&mut app, AggregatorSwitchKind::ActivateAfterCreate, true);
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let mut activated = with_group.clone();
    activated.activate_after_create = true;
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: activated.clone(),
        }),
        "the activation switch rides the shared draft"
    );
    sink.clear();

    // DUAL-08-08: a malformed rename line is refused at the surface, and the
    // previous command is not repeated.
    set_marker_text::<AggregatorRenamesField>(&mut app, "missing arrow");
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "a malformed rename rule never reaches the shared bus"
    );
    assert!(
        subtree_has_text(app.world(), root, "重命名规则格式错误"),
        "the wizard status line points at the malformed rule"
    );
    set_marker_text::<AggregatorRenamesField>(&mut app, "-广告$ => ");

    // DUAL-08-13: "use template" prefills the wizard from the projection.
    set_marker_text::<AggregatorTemplateNameField>(&mut app, "已保存模板");
    let use_template = marker_entity::<UseAggregationTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: use_template,
    });
    app.update();
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let from_template = infiltrator_contract::aggregator::AggregationDraft {
        source_profiles: vec!["主力高速订阅 (Primary VIP)".to_owned()],
        target_name: "Template-Target".to_owned(),
        deduplicate: false,
        deduplicate_names: true,
        geo_cluster: false,
        generate_groups: true,
        remove_emojis: false,
        rename_rules: vec![infiltrator_contract::aggregator::AggregationRenameRule {
            pattern: "-广告$".to_owned(),
            replacement: String::new(),
        }],
        custom_groups: Vec::new(),
        availability_precheck: false,
        activate_after_create: true,
    };
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: from_template.clone(),
        }),
        "the reused template drives the wizard's own fields"
    );
    sink.clear();

    // DUAL-08-13: save the live draft as a template under the typed name.
    let save_template = marker_entity::<SaveAggregationTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: save_template,
    });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::SaveAggregationTemplate {
            name: "已保存模板".to_owned(),
            draft: from_template.clone(),
        }),
        "the template save rides the shared command"
    );
    sink.clear();

    // DUAL-08-07: re-aggregate resolves the template by name and submits it.
    let reaggregate = marker_entity::<ReAggregateTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: reaggregate,
    });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::ReAggregateProfile {
            template_name: "已保存模板".to_owned(),
        }),
        "the re-aggregate action rides the shared command"
    );
    sink.clear();

    // DUAL-08-13: deleting the named template rides the shared command.
    let delete = marker_entity::<DeleteAggregationTemplateButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: delete });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::DeleteAggregationTemplate {
            name: "已保存模板".to_owned(),
        }),
        "the template delete rides the shared command"
    );
    sink.clear();

    // Unknown template names are refused with a status hint, not a command.
    set_marker_text::<AggregatorTemplateNameField>(&mut app, "不存在");
    let delete = marker_entity::<DeleteAggregationTemplateButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: delete });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "an unknown template never reaches the shared bus"
    );

    // DUAL-08-13: a host without the template sidecar is reported as such, not
    // as an empty template library.
    let mut unavailable = aggregation_page_projection();
    unavailable.aggregation_templates.clear();
    unavailable.aggregation_templates_available = false;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(unavailable));
    app.update();
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "历史聚合模板：宿主未提供模板存储（不支持）"
        ),
        "the unsupported template store is stated explicitly"
    );
}
