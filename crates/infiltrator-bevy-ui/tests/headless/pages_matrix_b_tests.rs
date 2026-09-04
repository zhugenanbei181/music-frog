//! Headless integration tests for Page Matrix B (DNS, Doctor, Settings, Sync, AppRouting):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - WebDAV conflict resolution, DNS cache clearing, Doctor repair actions, and AppRouting rules.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::app_routing::*;
use infiltrator_bevy_ui::pages::dns::*;
use infiltrator_bevy_ui::pages::doctor::*;
use infiltrator_bevy_ui::pages::settings::*;
use infiltrator_bevy_ui::pages::sync::*;
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};

use crate::support::*;

fn setup_matrix_b_app(sink: Arc<DemoCommandSink>) -> App {
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
// 1. DNS Page Tests
// ===========================================================================

#[test]
fn test_dns_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "域名解析 · Fake-IP 模式 (增强隐私与速度) (缓存条目: 342)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "https://1.1.1.1/dns-query"
    ));
    assert!(subtree_has_text(app.world(), root, "DoH (HTTPS)"));
    assert!(subtree_has_text(app.world(), root, "28 ms"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分配网段: 198.18.0.1/16"
    ));

    // Verify 6 DNS form switches matching Iced
    assert!(subtree_has_text(app.world(), root, "enable"));
    assert!(subtree_has_text(app.world(), root, "启用 DNS 服务"));
    assert!(subtree_has_text(app.world(), root, "ipv6"));
    assert!(subtree_has_text(app.world(), root, "IPv6 解析"));
    assert!(subtree_has_text(app.world(), root, "cache"));
    assert!(subtree_has_text(app.world(), root, "DNS 内存缓存"));
    assert!(subtree_has_text(app.world(), root, "use_hosts"));
    assert!(subtree_has_text(app.world(), root, "遵循系统 Hosts"));
    assert!(subtree_has_text(app.world(), root, "use_system_hosts"));
    assert!(subtree_has_text(app.world(), root, "系统默认解析器"));
    assert!(subtree_has_text(app.world(), root, "respect_rules"));
    assert!(subtree_has_text(app.world(), root, "分流规则优先"));

    // Verify Domain Mapping Mode and Filter Mode segmented controls
    assert!(subtree_has_text(
        app.world(),
        root,
        "域名映射模式 (enhanced_mode)"
    ));
    assert!(subtree_has_text(app.world(), root, "虚拟 IP (Fake-IP)"));
    assert!(subtree_has_text(app.world(), root, "真实 IP (Redir-Host)"));
    assert!(subtree_has_text(app.world(), root, "取消映射 (None)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "过滤模式 (fake_ip_filter_mode)"
    ));
    assert!(subtree_has_text(app.world(), root, "黑名单 (Blacklist)"));
    assert!(subtree_has_text(app.world(), root, "白名单 (Whitelist)"));
    assert!(subtree_has_text(app.world(), root, "规则 (Rules)"));

    let switch_count = app
        .world_mut()
        .query::<&DnsSwitchButton>()
        .iter(app.world())
        .count();
    assert_eq!(switch_count, 6);
}

#[test]
fn test_dns_switch_submits_setting_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let switch_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsSwitchButton>>()
        .iter(app.world())
        .next()
        .expect("dns switch button");

    app.world_mut().commands().trigger(Activate {
        entity: switch_entity,
    });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::UpdateSetting { key, value } => {
            assert!(key.starts_with("dns."));
            assert_eq!(value, "toggle");
        }
        other => panic!("expected UpdateSetting, got {:?}", other),
    }
}

#[test]
fn test_dns_clear_cache_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ClearDnsCacheButton>>()
        .single(app.world())
        .expect("clear dns cache button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ClearDnsCache]);
}

#[test]
fn test_dns_test_latency_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<TestDnsLatencyButton>>()
        .single(app.world())
        .expect("test dns latency button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::TestDnsLatency]);
}

#[test]
fn test_dns_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    let mut updated = DnsProjection::demo();
    updated.mode = DnsMode::RedirHost;
    updated.cache_entries = 999;
    updated.fake_ip_range = "198.19.0.0/16".to_owned();
    updated.servers[0].latency_ms = Some(12);

    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "域名解析 · Redir-Host 模式 (真实 IP 解析) (缓存条目: 999)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分配网段: 198.19.0.0/16"
    ));
    assert!(subtree_has_text(app.world(), root, "12 ms"));
}

// ===========================================================================
// 2. Doctor Page Tests
// ===========================================================================

#[test]
fn test_doctor_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Doctor);

    assert!(subtree_has_text(
        app.world(),
        root,
        "自愈诊断 · 健康评估 (6 / 6 项检查通过)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "TUN 虚拟网卡与路由表健康度"
    ));
    assert!(subtree_has_text(app.world(), root, "正常 (PASS)"));
    assert!(subtree_has_text(app.world(), root, "立即诊断"));
    assert!(subtree_has_text(app.world(), root, "一键修复"));
}

#[test]
fn test_doctor_run_diagnostics_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Doctor);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<RunDoctorDiagnosticsButton>>()
        .single(app.world())
        .expect("run doctor button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RunDoctorDiagnostics]);
}

#[test]
fn test_doctor_repair_all_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Doctor);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<RepairAllDoctorButton>>()
        .single(app.world())
        .expect("repair all doctor button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RepairAllDoctorIssues]);
}

#[test]
fn test_doctor_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Doctor);

    let mut updated = DoctorProjection::demo();
    updated.overall_healthy = false;
    updated.last_run = "2026-09-02 12:00:00".to_owned();
    updated.checks[0].state = DoctorCheckState::Fail;
    updated.checks[0].detail = "TUN 接口 utun9 意外掉线".to_owned();

    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "自愈诊断 · 健康评估 (5 / 6 项检查通过)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "最近诊断: 2026-09-02 12:00:00"
    ));
    assert!(subtree_has_text(app.world(), root, "异常 (FAIL)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "TUN 接口 utun9 意外掉线"
    ));
}

// ===========================================================================
// 3. Settings Page Tests
// ===========================================================================

#[test]
fn test_settings_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Settings);

    assert!(subtree_has_text(
        app.world(),
        root,
        "系统与内核全局设置 · 统一策略中枢"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "开机自动启动 (Autostart on Boot)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "设置系统代理 (Set System Proxy)"
    ));
    assert!(subtree_has_text(app.world(), root, "端口: 7890"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "gVisor (高性能用户态协议栈)"
    ));
    assert!(subtree_has_text(app.world(), root, "127.0.0.1:9090"));

    // TUN permission alert banner assertions
    assert!(subtree_has_text(app.world(), root, "准备 TUN 权限"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "⚡ 权限状态: 启用 TUN 前需要为 mihomo 配置平台权限；完成后请重新开启 TUN。"
    ));

    // OS integration settings assertions
    assert!(subtree_has_text(app.world(), root, "关闭窗口最小化到托盘"));
    assert!(subtree_has_text(app.world(), root, "系统通知"));
    assert!(subtree_has_text(app.world(), root, "浅色模式"));
    assert!(subtree_has_text(app.world(), root, "深色模式"));
    assert!(subtree_has_text(app.world(), root, "护眼森林"));
    assert!(subtree_has_text(app.world(), root, "AMOLED"));
    assert!(subtree_has_text(app.world(), root, "zh-CN (简体中文)"));
    assert!(subtree_has_text(app.world(), root, "en-US (English)"));

    // Verify component markers exist in page tree
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<PrepareTunPermissionButton>>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<CloseToTrayToggle>>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<SystemNotificationsToggle>>()
            .iter(app.world())
            .next()
            .is_some()
    );
}

#[test]
fn test_settings_prepare_tun_and_toggles_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let prepare_btn = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<PrepareTunPermissionButton>>()
        .single(app.world())
        .expect("prepare tun button");
    app.world_mut().commands().trigger(Activate {
        entity: prepare_btn,
    });
    app.update();

    let tray_toggle = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CloseToTrayToggle>>()
        .single(app.world())
        .expect("close to tray toggle");
    app.world_mut().commands().trigger(Activate {
        entity: tray_toggle,
    });
    app.update();

    let notif_toggle = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SystemNotificationsToggle>>()
        .single(app.world())
        .expect("system notifications toggle");
    app.world_mut().commands().trigger(Activate {
        entity: notif_toggle,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::UpdateSetting {
                key: "tun_privilege".to_owned(),
                value: "prepare".to_owned(),
            },
            UiCommand::UpdateSetting {
                key: "close_to_tray".to_owned(),
                value: "toggle".to_owned(),
            },
            UiCommand::UpdateSetting {
                key: "notifications_enabled".to_owned(),
                value: "toggle".to_owned(),
            },
        ]
    );
}

#[test]
fn test_settings_save_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SaveSettingsButton>>()
        .single(app.world())
        .expect("save settings button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSetting {
            key: "apply".to_owned(),
            value: "true".to_owned(),
        }]
    );
}

#[test]
fn test_settings_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut updated = SettingsProjection::demo();
    updated.mixed_port = 7899;
    updated.controller_port = 9191;
    updated.log_level = "debug".to_owned();

    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(app.world(), root, "端口: 7899"));
    assert!(subtree_has_text(app.world(), root, "127.0.0.1:9191"));
    assert!(subtree_has_text(app.world(), root, "DEBUG"));
}

// ===========================================================================
// 4. Sync Page Tests
// ===========================================================================

#[test]
fn test_sync_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Sync);

    assert!(subtree_has_text(
        app.world(),
        root,
        "数据同步 · 已连接 · 同步就绪"
    ));
    assert!(subtree_has_text(app.world(), root, "立即同步"));
    assert!(subtree_has_text(app.world(), root, "创建备份"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "https://dav.jianguoyun.com/dav/MusicFrog/"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Linux Desktop (CachyOS) · 2026-09-02 10:15"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "字段级三向冲突差异合并 (3-Way Merge & Conflict Resolver)"
    ));
    assert!(subtree_has_text(app.world(), root, "智能合并两者"));
}

#[test]
fn test_sync_now_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SyncNowButton>>()
        .single(app.world())
        .expect("sync now button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::SyncNow]);
}

#[test]
fn test_sync_create_backup_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CreateBackupButton>>()
        .single(app.world())
        .expect("create backup button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::CreateBackupSnapshot]);
}

#[test]
fn test_sync_conflict_state_and_resolution_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Sync);

    let mut conflict_proj = SyncProjection::demo();
    conflict_proj.status = SyncStatus::Conflict;
    conflict_proj.conflict = Some(SyncConflictInfo {
        remote_device: "Android (Pixel 9 Pro)".to_owned(),
        conflict_time: "2026-09-02 10:14".to_owned(),
        conflicting_keys: vec![ConflictingKey {
            key: "mode".to_owned(),
            local_value: "Rule".to_owned(),
            remote_value: "Global".to_owned(),
        }],
    });

    app.world_mut()
        .commands()
        .trigger(SyncProjectionUpdated(conflict_proj));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "数据同步 · 同步冲突 · 需要手动解决"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "检测到冲突：远端设备 Android (Pixel 9 Pro) 于 2026-09-02 10:14 产生变更，共 1 处不一致"
    ));

    // Test keep local button
    let keep_local_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<KeepLocalConflictButton>>()
        .single(app.world())
        .expect("keep local button");

    app.world_mut().commands().trigger(Activate {
        entity: keep_local_entity,
    });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ResolveConflictKeepLocal]);

    // Test take remote button
    sink.clear();
    let take_remote_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<TakeRemoteConflictButton>>()
        .single(app.world())
        .expect("take remote button");

    app.world_mut().commands().trigger(Activate {
        entity: take_remote_entity,
    });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ResolveConflictTakeRemote]);
}

#[test]
fn test_sync_restore_snapshot_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Sync);

    let mut query = app.world_mut().query::<(Entity, &RestoreSnapshotButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.snapshot_id == "snap-1")
        .expect("snap-1 restore button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RestoreSnapshot {
            id: "snap-1".to_owned(),
        }]
    );
}

// ===========================================================================
// 5. AppRouting Page Tests
// ===========================================================================

#[test]
fn test_app_routing_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::AppRouting);

    assert!(subtree_has_text(
        app.world(),
        root,
        "应用分流 · 白名单分流 (指定应用直连) (已配置 6 个应用)"
    ));
    assert!(subtree_has_text(app.world(), root, "Google Chrome 浏览器"));
    assert!(subtree_has_text(app.world(), root, "代理 (Proxy)"));
    assert!(subtree_has_text(app.world(), root, "Steam 游戏平台"));
    assert!(subtree_has_text(app.world(), root, "直连 (Direct)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Windows UWP 回环隔离豁免工具 (UWP Loopback Exemption)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键豁免全部 UWP 应用"));
}

#[test]
fn test_app_routing_add_rule_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<AddAppRouteButton>>()
        .single(app.world())
        .expect("add app rule button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAppRule {
            app_id: "new-app".to_owned(),
            rule: "Proxy".to_owned(),
        }]
    );
}

#[test]
fn test_app_routing_switch_rule_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let mut query = app.world_mut().query::<(Entity, &SwitchAppRuleButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.app_id == "app-1")
        .expect("app-1 switch rule button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAppRule {
            app_id: "app-1".to_owned(),
            rule: "Direct".to_owned(),
        }]
    );
}

#[test]
fn test_app_routing_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::AppRouting);

    let mut updated = AppRoutingProjection::demo();
    updated.mode = AppRoutingMode::ProxyList;
    updated.apps[0].rule = AppRouteRule::Block;

    app.world_mut()
        .commands()
        .trigger(AppRoutingProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "应用分流 · 黑名单分流 (仅指定应用代理) (已配置 6 个应用)"
    ));
    assert!(subtree_has_text(app.world(), root, "拦截 (Block)"));
}
