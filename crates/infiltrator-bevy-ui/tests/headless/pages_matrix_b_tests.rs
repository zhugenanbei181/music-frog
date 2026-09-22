//! Headless integration tests for Page Matrix B (DNS, Doctor, Settings, Sync, AppRouting):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - WebDAV conflict resolution, DNS cache clearing, Doctor repair actions, and AppRouting rules.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ValueChange};
use infiltrator_bevy_ui::app::{
    ShellPlugin, SidebarSystemProxyToggle, SidebarToggleProjection, SidebarTunToggle,
};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::app_routing::*;
use infiltrator_bevy_ui::pages::app_routing_uwp::{UwpAction, UwpActionButton};
use infiltrator_bevy_ui::pages::dns::*;
use infiltrator_bevy_ui::pages::dns_edit::{
    DnsEditApplyButton, DnsEditField, DnsEditGeoipToggle, DnsEditStatusLine, DnsEditTemplate,
};
use infiltrator_bevy_ui::pages::dns_fakeip::DnsFakeIpSearchField;
use infiltrator_bevy_ui::pages::dns_hosts::{
    DnsHostsApplyButton, DnsHostsEditorField, DnsHostsStatusLine,
};
use infiltrator_bevy_ui::pages::doctor::*;
use infiltrator_bevy_ui::pages::settings::settings_core::{
    CoreLogLevelButton, ProbeTunMtuButton, SettingsProjection, TunEnableToggle, TunRouteToggle,
    TunRouteToggleKind, TunStackButton, TunStackButtonAvailability,
};
use infiltrator_bevy_ui::pages::settings::settings_ipv6::Ipv6RoutingToggle;
use infiltrator_bevy_ui::pages::settings::settings_lan::{
    LanAllowedIpsField, LanAuthPasswordField, LanAuthUsernameField, LanAuthenticationToggle,
    LanBindAddressField, LanDisallowedIpsField, LanMixedPortField, LanSecurityApplyButton,
    LanSharingApplyButton, LanSharingToggle, LanSkipAuthPrefixesField,
};
use infiltrator_bevy_ui::pages::settings::settings_network_roaming::{
    NetworkRoamingRefreshButton, NetworkRoamingRepairButton,
};
use infiltrator_bevy_ui::pages::settings::settings_pac::{PacApplyButton, PacBypassField};
use infiltrator_bevy_ui::pages::settings::settings_privileged_network::PrivilegedNetworkRunButton;
use infiltrator_bevy_ui::pages::settings::settings_system::SystemProxyToggle;
use infiltrator_bevy_ui::pages::settings::settings_vpn::{VpnStartButton, VpnStopButton};
use infiltrator_bevy_ui::pages::settings::{
    CloseToTrayToggle, CoreRollbackButton, PortConflictButton, PrepareTunPermissionButton,
    SaveSettingsButton, ServiceModeButton, SettingsProjectionUpdated, SystemNotificationsToggle,
};
use infiltrator_bevy_ui::pages::sync::*;
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{DemoSurfaceSource, SurfaceSnapshotUpdated, SurfaceSource};
use infiltrator_bevy_widgets::button::{ControlVisual, PillLabel};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode, DnsSwitchField};
use infiltrator_contract::dns_form::DnsFormField;
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::{LanCredentials, LanSecuritySnapshot};
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, PhysicalMtuSnapshot};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingSnapshot, NetworkRoamingStatus,
};
use infiltrator_contract::offline_startup::{LocalAssetStatus, OfflineStartupSnapshot};
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_contract::system_proxy::{
    SystemProxyDesiredState, SystemProxyObservation, SystemProxyRecoveryStatus,
};
use infiltrator_contract::tun::TunStack;
use infiltrator_contract::version::CoreRollbackSnapshot;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};

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
fn test_dns_switch_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let switch_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsSwitchButton>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsSwitchButton>(*entity),
                Some(DnsSwitchButton(DnsSwitchField::Enable, _))
            )
        })
        .expect("enable dns switch button");

    app.world_mut().commands().trigger(Activate {
        entity: switch_entity,
    });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::ApplyDnsSettings { patch } => {
            let switches = patch
                .switches
                .expect("switch patch must carry the full set");
            assert!(!switches.enable, "demo enable=true toggles to false");
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_enhanced_mode_pill_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let pill_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsEnhancedModePill>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsEnhancedModePill>(*entity),
                Some(DnsEnhancedModePill(DnsEnhancedMode::RedirHost))
            )
        })
        .expect("redir-host pill");

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            assert_eq!(patch.enhanced_mode, Some(DnsEnhancedMode::RedirHost));
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_filter_mode_pill_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let pill_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsFilterModePill>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsFilterModePill>(*entity),
                Some(DnsFilterModePill(DnsFakeIpFilterMode::Whitelist))
            )
        })
        .expect("whitelist pill");

    app.world_mut().commands().trigger(Activate {
        entity: pill_entity,
    });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            assert_eq!(patch.filter_mode, Some(DnsFakeIpFilterMode::Whitelist));
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
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
    updated.mode = DnsEnhancedMode::RedirHost;
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
// 1b. DNS Workbench Form Parity Tests (DUAL-14-04 / 14-05 / 14-07 / 14-14)
// ===========================================================================

fn set_dns_field(app: &mut App, field: DnsFormField, value: &str) {
    let parent = app
        .world_mut()
        .query::<(Entity, &DnsEditField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == field)
        .map(|(entity, _)| entity)
        .expect("edit field row");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("edit field children")
        .iter()
        .next()
        .expect("text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("text field component")
        .0
        .apply(TextFieldInput::SetText(value.to_owned()));
}

fn trigger_dns_edit_apply(app: &mut App) {
    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsEditApplyButton>>()
        .single(app.world())
        .expect("dns edit apply button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
}

/// The rendered text of one typed DNS line kind.
fn dns_line_text(app: &mut App, kind: DnsLineKind) -> String {
    let world = app.world_mut();
    let mut query = world.query::<(&Text, &DnsLine)>();
    query
        .iter(world)
        .find(|(_, line)| line.0 == kind)
        .map(|(text, _)| text.0.clone())
        .expect("dns line")
}

#[test]
fn test_dns_edit_rows_cover_every_shared_text_field() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    let rendered: Vec<DnsFormField> = {
        let world = app.world_mut();
        let mut query = world.query::<&DnsEditField>();
        query.iter(world).map(|marker| marker.0).collect()
    };
    for field in DnsFormField::ALL {
        if DnsFormField::SWITCH_FIELDS
            .iter()
            .any(|switch| DnsFormField::from_switch(*switch) == field)
            || matches!(field, DnsFormField::EnhancedMode | DnsFormField::FilterMode)
        {
            continue;
        }
        if matches!(
            field,
            DnsFormField::FallbackGeoip | DnsFormField::FallbackGeoipCode
        ) {
            continue;
        }
        assert!(
            rendered.contains(&field),
            "Bevy workbench edit row missing for {field:?}"
        );
    }
    assert!(subtree_has_text(
        app.world(),
        root,
        "上游加密 DNS 配置与回退策略 (DUAL-14-04/05)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "GEOIP 触发回退 (fallback_filter.geoip)"
    ));
}

#[test]
fn test_dns_upstream_list_edit_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    set_dns_field(
        &mut app,
        DnsFormField::Nameserver,
        "https://dns.google/dns-query, quic://dns.adguard.com",
    );
    trigger_dns_edit_apply(&mut app);

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::ApplyDnsSettings { patch } => {
            assert_eq!(
                patch.nameserver.as_deref(),
                Some(
                    &[
                        "https://dns.google/dns-query".to_owned(),
                        "quic://dns.adguard.com".to_owned()
                    ][..]
                )
            );
            // The whole workbench patch is submitted, including the fallback tier.
            assert!(patch.fallback.is_some());
            assert!(patch.fallback_policy.is_some());
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_fallback_policy_toggle_and_trigger_submit_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    // Demo fixture starts with git-ip fallback geoip enabled; toggling flips it.
    let toggle_before = app
        .world_mut()
        .query::<&DnsEditGeoipToggle>()
        .single(app.world())
        .copied()
        .expect("geoip toggle");
    let toggle_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsEditGeoipToggle>>()
        .single(app.world())
        .expect("geoip toggle entity");
    app.world_mut().commands().trigger(Activate {
        entity: toggle_entity,
    });
    app.update();

    set_dns_field(
        &mut app,
        DnsFormField::FallbackTriggerIp,
        "240.0.0.0/4, 10.0.0.0/8",
    );
    trigger_dns_edit_apply(&mut app);

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let policy = patch.fallback_policy.as_ref().expect("fallback policy");
            assert_eq!(policy.geoip, !toggle_before.0);
            assert_eq!(
                policy.trigger_ipcidr,
                vec!["240.0.0.0/4".to_owned(), "10.0.0.0/8".to_owned()]
            );
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_form_local_validation_blocks_invalid_scheme() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    set_dns_field(&mut app, DnsFormField::Nameserver, "ftp://dns.example");
    trigger_dns_edit_apply(&mut app);

    assert!(sink.submitted().is_empty(), "invalid form must not submit");
    let status = {
        let world = app.world_mut();
        let mut query = world.query::<(&Text, &DnsEditStatusLine)>();
        query
            .iter(world)
            .map(|(text, _)| text.0.clone())
            .next()
            .expect("status line")
    };
    assert!(status.contains("本地校验未通过"), "{status}");
    assert!(status.contains("ftp://dns.example"), "{status}");
}

#[test]
fn test_dns_quick_template_chip_appends_unique_entry() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    navigate_to(&mut app, Route::Dns);

    let chip = app
        .world_mut()
        .query::<(Entity, &DnsEditTemplate)>()
        .iter(app.world())
        .find(|(_, chip)| chip.field == DnsFormField::Fallback && chip.server == "8.8.8.8")
        .map(|(entity, _)| entity)
        .expect("fallback template chip");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: chip });
    app.update();

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsEditField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == DnsFormField::Fallback)
        .map(|(entity, _)| entity)
        .expect("fallback field");
    let text = app
        .world()
        .get::<Children>(parent)
        .and_then(|children| children.iter().next().copied())
        .and_then(|child| app.world().get::<TextField>(child))
        .expect("fallback text field")
        .0
        .text()
        .to_owned();
    assert!(text.contains("8.8.8.8"), "{text}");
    assert!(
        text.contains("https://cloudflare-dns.com/dns-query"),
        "{text}"
    );
}

#[test]
fn test_dns_cache_flush_report_renders_honest_status() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "Fake-IP 缓存: 尚未执行 · 系统 DNS 缓存: 尚未执行"
    ));

    let mut updated = DnsProjection::demo();
    updated.cache_flush = infiltrator_contract::dns::DnsCacheFlushReport {
        fake_ip: infiltrator_contract::dns::DnsFlushOutcome::Flushed,
        os_cache: infiltrator_contract::dns::DnsFlushOutcome::Unsupported {
            reason: "host did not provide a system DNS cache adapter".to_owned(),
        },
    };
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "Fake-IP 缓存: 已清空 · 系统 DNS 缓存: 宿主不支持 (host did not provide a system DNS cache adapter)"
    ));
}

#[test]
fn test_dns_switch_patch_stays_full_after_shared_form_upgrade() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let switch_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsSwitchButton>>()
        .iter(app.world())
        .find(|entity| {
            matches!(
                app.world().get::<DnsSwitchButton>(*entity),
                Some(DnsSwitchButton(DnsSwitchField::RespectRules, _))
            )
        })
        .expect("respect_rules switch");
    app.world_mut().commands().trigger(Activate {
        entity: switch_entity,
    });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let switches = patch.switches.expect("switch set");
            assert!(switches.respect_rules);
            assert!(switches.enable);
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

// ===========================================================================
// 1c. DNS Fake-IP Pool / Latency / Hosts Editor (DUAL-14-06 / 14-10 / 14-11)
// ===========================================================================

#[test]
fn test_dns_fake_ip_pool_search_filters_the_observed_listing() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    assert!(subtree_has_text(
        app.world(),
        root,
        "198.18.0.5 ↔ music.example.org"
    ));

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsFakeIpSearchField)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("fake-ip search field");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("search children")
        .iter()
        .next()
        .expect("search text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("search text field")
        .0
        .apply(TextFieldInput::SetText("cdn".to_owned()));
    app.update();

    let listing = dns_line_text(&mut app, DnsLineKind::FakeIpMapping);
    assert_eq!(listing, "198.18.0.7 ↔ cdn.example.net");
    let count = dns_line_text(&mut app, DnsLineKind::FakeIpMappingCount);
    assert!(count.contains("显示 1 / 共 2 条"), "{count}");

    // A host without the controller connection feed never renders a binding.
    let mut unsupported = DnsProjection::demo();
    unsupported.fake_ip_pool = infiltrator_contract::dns::FakeIpMappingPool::default();
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(unsupported));
    app.update();
    assert!(dns_line_text(&mut app, DnsLineKind::FakeIpMapping).contains("宿主未提供映射事实"));
}

#[test]
fn test_dns_latency_policy_line_is_honest() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    // The demo fixture carries the pinned screenshot values, and every row is
    // rendered from the shared report type.
    assert!(subtree_has_text(
        app.world(),
        root,
        "逐 Nameserver 延迟: 4 个上游中 3 个应答"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "tls://8.8.8.8:853 [主上游] 未探测 (DNS over TLS is not probed by this host)"
    ));

    // A host with no prober publishes the typed refusal, never a number.
    let mut without_prober = DnsProjection::demo();
    without_prober.latency = infiltrator_contract::dns_latency::DnsLatencyReport::default();
    without_prober.self_heal = Default::default();
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(without_prober));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "逐 Nameserver 延迟: 宿主未提供探测能力 (this host did not inject a DNS latency prober)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "无逐 Nameserver 结果: 宿主未注入探测端口"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "尚未观测到 DNS 健康事实"
    ));
}

#[test]
fn test_dns_self_heal_card_renders_the_shared_observation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Dns);

    // The demo fixture's snapshot (one warning, two healthy) is rendered with
    // the bare-Chinese convention and the typed fix key.
    assert!(subtree_has_text(
        app.world(),
        root,
        "监听端口 (dns.listen) [正常] dns.listen port 1053 is available"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "上游解析可达性 [注意] only 3 of 4 upstreams answered · 建议修复: recheck_upstreams"
    ));
}

#[test]
fn test_dns_hosts_editor_submits_shared_patch() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    // The editor is seeded with the applied rows from the shared model.
    let parent = app
        .world_mut()
        .query::<(Entity, &DnsHostsEditorField)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("hosts editor field");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("hosts children")
        .iter()
        .next()
        .expect("hosts text field child");
    let seeded = app
        .world()
        .get::<TextField>(child)
        .expect("hosts text field")
        .0
        .text()
        .to_owned();
    assert!(seeded.contains("192.168.1.1 router.lan"), "{seeded}");

    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("hosts text field")
        .0
        .apply(TextFieldInput::SetText(
            "192.168.1.1 router.lan; 1.1.1.1 multi.example.com; 8.8.8.8 multi.example.com"
                .to_owned(),
        ));
    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsHostsApplyButton>>()
        .single(app.world())
        .expect("hosts apply button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            let hosts = patch.hosts.as_ref().expect("hosts rows");
            assert_eq!(hosts.len(), 3);
            assert!(!patch.clear_hosts);
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
}

#[test]
fn test_dns_hosts_editor_local_validation_blocks_bad_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsHostsEditorField)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("hosts editor field");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("hosts children")
        .iter()
        .next()
        .expect("hosts text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("hosts text field")
        .0
        .apply(TextFieldInput::SetText("nope bad domain".to_owned()));
    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsHostsApplyButton>>()
        .single(app.world())
        .expect("hosts apply button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(sink.submitted().is_empty(), "invalid hosts must not submit");
    let status = {
        let world = app.world_mut();
        let mut query = world.query::<(&Text, &DnsHostsStatusLine)>();
        query
            .iter(world)
            .map(|(text, _)| text.0.clone())
            .next()
            .expect("hosts status line")
    };
    assert!(status.contains("本地校验未通过"), "{status}");
}

#[test]
fn test_dns_hosts_editor_clears_an_emptied_mapping() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Dns);

    let parent = app
        .world_mut()
        .query::<(Entity, &DnsHostsEditorField)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .next()
        .expect("hosts editor field");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("hosts children")
        .iter()
        .next()
        .expect("hosts text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("hosts text field")
        .0
        .apply(TextFieldInput::SetText(String::new()));
    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<DnsHostsApplyButton>>()
        .single(app.world())
        .expect("hosts apply button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    match sink.submitted().first() {
        Some(UiCommand::ApplyDnsSettings { patch }) => {
            assert!(patch.clear_hosts, "an emptied editor clears dns.hosts");
            assert!(patch.hosts.is_none());
        }
        other => panic!("expected ApplyDnsSettings, got {:?}", other),
    }
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
    updated.watchdog = CoreWatchdogSnapshot {
        state: CoreWatchdogState::Waiting {
            attempt: 2,
            retry_in_ms: 200,
        },
        session_token: None,
        consecutive_failures: 2,
        last_error: None,
    };

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
        "核心看门狗：第 2 次重启将在 200 ms 后执行"
    ));
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
fn test_sidebar_system_toggles_use_shared_projection_and_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));

    let proxy = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SidebarSystemProxyToggle>>()
        .single(app.world())
        .expect("sidebar system proxy toggle");
    let tun = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SidebarTunToggle>>()
        .single(app.world())
        .expect("sidebar TUN toggle");

    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        infiltrator_contract::system_toggle::SystemToggleState::Enabled
    ));

    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSystemProxy { enabled: false }]
    );
    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        infiltrator_contract::system_toggle::SystemToggleState::Pending { desired: false }
    ));

    // A second activation while the application command is in flight is
    // fenced instead of producing a duplicate command.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(sink.submitted().len(), 1);

    let mut next = DemoSurfaceSource::running().surface_snapshot();
    next.revision = 2;
    next.system_proxy = infiltrator_contract::system_proxy::SystemProxySnapshot::from_observation(
        2,
        infiltrator_contract::system_proxy::SystemProxyObservation {
            enabled: false,
            endpoint: None,
            bypass: None,
        },
    );
    next.pages
        .settings
        .data
        .as_mut()
        .expect("demo settings")
        .tun_enabled = false;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(next));
    app.update();

    let proxy_visual = app
        .world()
        .get::<ControlVisual>(proxy)
        .expect("proxy visual");
    let tun_visual = app.world().get::<ControlVisual>(tun).expect("tun visual");
    assert!(!proxy_visual.0);
    assert!(!tun_visual.0);
    let proxy_label = app
        .world()
        .get::<Children>(proxy)
        .expect("proxy toggle children")
        .iter()
        .find_map(|child| {
            app.world()
                .get::<PillLabel>(*child)
                .and_then(|_| app.world().get::<Text>(*child))
        })
        .map(|text| text.0.clone());
    assert_eq!(proxy_label.as_deref(), Some("关"));
    assert_eq!(
        app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy
            .compact_label(),
        "关"
    );

    app.world_mut().commands().trigger(Activate { entity: tun });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetSystemProxy { enabled: false },
            UiCommand::ToggleTun { enabled: true },
        ]
    );
}

#[test]
fn test_privileged_network_regression_button_uses_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<PrivilegedNetworkRunButton>>()
        .single(app.world())
        .expect("privileged network regression button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RunPrivilegedNetworkRegression]
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

    let debug_button = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &CoreLogLevelButton)>();
        query
            .iter(world)
            .find(|(_, button)| button.level == CoreLogLevel::Debug)
            .map(|(entity, _)| entity)
            .expect("debug core log level button")
    };
    app.world_mut().commands().trigger(Activate {
        entity: debug_button,
    });
    app.update();

    let service_button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ServiceModeButton>>()
        .single(app.world())
        .expect("service mode button");
    app.world_mut().commands().trigger(Activate {
        entity: service_button,
    });
    app.update();

    let port_button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<PortConflictButton>>()
        .single(app.world())
        .expect("port conflict repair button");
    app.world_mut().commands().trigger(Activate {
        entity: port_button,
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
            UiCommand::SetCoreLogLevel(CoreLogLevel::Debug),
            UiCommand::PrepareServiceMode,
            UiCommand::RepairPortConflicts,
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
fn test_settings_core_rollback_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.core_versions.rollback = CoreRollbackSnapshot {
        current: Some("v1.19.30".to_owned()),
        target: Some("v1.19.29".to_owned()),
        history: vec!["v1.19.29".to_owned()],
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<CoreRollbackButton>>()
        .single(app.world())
        .expect("core rollback button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RollbackCore]);
}

#[test]
fn test_settings_network_roaming_projection_and_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.network_roaming = NetworkRoamingSnapshot {
        status: NetworkRoamingStatus::Stable,
        interfaces: vec![NetworkInterfaceSnapshot {
            name: "eth0".to_owned(),
            kind: NetworkInterfaceKind::Ethernet,
            is_up: true,
            is_default_gateway: true,
            gateway_ip: Some("192.0.2.1".to_owned()),
            ip_addresses: vec!["192.0.2.10/24".to_owned()],
            mtu: Some(1500),
            metric: Some(100),
            dns_servers: Vec::new(),
        }],
        active_interface: Some("eth0".to_owned()),
        default_gateway: Some("192.0.2.1".to_owned()),
        tun_interface: Some("Meta".to_owned()),
        physical_mtu: Some(1500),
        recommended_tun_mtu: Some(1420),
        tcp_mss: Some(1380),
        ..NetworkRoamingSnapshot::default()
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "active=eth0 · gateway=192.0.2.1"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "eth0 [up] gw=192.0.2.1"
    ));

    let refresh = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<NetworkRoamingRefreshButton>>()
        .single(app.world())
        .expect("network roaming refresh button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: refresh });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::RefreshNetworkRoaming]);

    sink.clear();
    let repair = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<NetworkRoamingRepairButton>>()
        .single(app.world())
        .expect("network roaming repair button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: repair });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::RepairNetworkRoutes]);
}

#[test]
fn test_settings_vpn_projection_and_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.vpn = VpnSessionSnapshot {
        state: VpnSessionState::PermissionRequired,
        foreground: false,
        revision: 3,
        ..VpnSessionSnapshot::default()
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "等待 Android VPN 用户授权"
    ));

    let start = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<VpnStartButton>>()
        .single(app.world())
        .expect("VPN start button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: start });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::StartVpn]);

    sink.clear();
    let stop = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<VpnStopButton>>()
        .single(app.world())
        .expect("VPN stop button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: stop });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::StopVpn]);
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
    updated.tun_auto_route = false;
    updated.tun_strict_route = true;
    updated.offline_startup = OfflineStartupSnapshot::ready(LocalAssetStatus::Missing);
    updated.mtu = MtuNegotiationSnapshot::ready(
        2,
        PhysicalMtuSnapshot {
            interface: "eth0".to_owned(),
            mtu: 1500,
        },
        1420,
        1380,
    );

    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(app.world(), root, "端口: 7899"));
    assert!(subtree_has_text(app.world(), root, "127.0.0.1:9191"));
    assert!(subtree_has_text(app.world(), root, "DEBUG"));
    assert!(subtree_has_text(app.world(), root, "离线优先"));
    assert!(subtree_has_text(app.world(), root, "可启动但已降级"));
    assert!(subtree_has_text(app.world(), root, "physical=1500"));

    let route_sources: Vec<(TunRouteToggleKind, Entity)> = {
        let mut toggles = app
            .world_mut()
            .query::<(&TunRouteToggle, &bevy::ecs::hierarchy::Children)>();
        toggles
            .iter(app.world())
            .map(|(toggle, children)| {
                (
                    toggle.0,
                    *children.iter().next().expect("route toggle checkbox"),
                )
            })
            .collect()
    };
    for (kind, source) in route_sources {
        let checked = app.world().get::<Checked>(source).is_some();
        assert_eq!(
            checked,
            matches!(kind, TunRouteToggleKind::StrictRoute),
            "route checkbox should follow the shared projection"
        );
    }
}

#[test]
fn test_settings_mtu_probe_submits_shared_application_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<ProbeTunMtuButton>>()
        .single(app.world())
        .expect("MTU probe button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ProbeTunMtu]);
}

#[test]
fn test_settings_tun_route_checkboxes_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut toggles = app
        .world_mut()
        .query::<(&TunRouteToggle, &bevy::ecs::hierarchy::Children)>();
    let sources: Vec<(TunRouteToggleKind, Entity)> = toggles
        .iter(app.world())
        .map(|(toggle, children)| {
            let source = children.iter().next().expect("route toggle checkbox");
            (toggle.0, *source)
        })
        .collect();

    for (kind, source) in sources {
        let value = match kind {
            TunRouteToggleKind::AutoRoute => false,
            TunRouteToggleKind::StrictRoute => true,
        };
        app.world_mut().commands().trigger(ValueChange {
            source,
            value,
            is_final: true,
        });
        app.update();
    }

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetTunAutoRoute(false),
            UiCommand::SetTunStrictRoute(true)
        ]
    );
}

#[test]
fn test_settings_tun_enable_checkbox_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let source = {
        let mut toggles = app
            .world_mut()
            .query::<(&TunEnableToggle, &bevy::ecs::hierarchy::Children)>();
        *toggles
            .single(app.world())
            .expect("TUN enable toggle")
            .1
            .iter()
            .next()
            .expect("TUN enable checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: false,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ToggleTun { enabled: false }]
    );
}

#[test]
fn test_settings_system_proxy_checkbox_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let source = {
        let mut toggles = app
            .world_mut()
            .query::<(&SystemProxyToggle, &bevy::ecs::hierarchy::Children)>();
        *toggles
            .single(app.world())
            .expect("system proxy toggle")
            .1
            .iter()
            .next()
            .expect("system proxy checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: false,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSystemProxy { enabled: false }]
    );
}

#[test]
fn test_settings_system_proxy_recovery_status_projects_shared_result() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.system_proxy_recovery.status = SystemProxyRecoveryStatus::Restored {
        previous: SystemProxyObservation::default(),
        restored: SystemProxyObservation::default(),
    };
    projection.system_proxy_recovery.revision = 9;
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection.clone()));
    app.update();
    assert!(subtree_has_text(app.world(), root, "启动已清理孤儿代理"));

    projection.system_proxy_recovery.status = SystemProxyRecoveryStatus::SkippedExternal {
        expected: SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        },
        observed: SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:9999".to_owned()),
            bypass: None,
        },
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "检测到外部修改，未覆盖"
    ));
}

#[test]
fn test_settings_lan_fields_submit_the_live_listener_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.allow_lan = true;
    projection.mixed_port = 8080;
    projection.lan_bind_address = "192.168.1.10".to_owned();
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "192.168.1.10"));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<LanSharingApplyButton>>()
        .single(app.world())
        .expect("Allow-LAN apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetLanSharing {
            enabled: true,
            mixed_port: 8080,
            bind_address: "192.168.1.10".to_owned(),
        }]
    );

    let _ = app
        .world_mut()
        .query::<&LanSharingToggle>()
        .single(app.world())
        .expect("Allow-LAN toggle");
    let _ = app
        .world_mut()
        .query::<&LanMixedPortField>()
        .single(app.world())
        .expect("mixed-port field");
    let _ = app
        .world_mut()
        .query::<&LanBindAddressField>()
        .single(app.world())
        .expect("bind-address field");
    let _ = app
        .world_mut()
        .query::<&TextField>()
        .iter(app.world())
        .count();
}

#[test]
fn test_settings_lan_security_submits_acl_and_redacted_basic_auth_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.lan_security = LanSecuritySnapshot::new(
        5,
        vec!["192.168.1.0/24".to_owned()],
        vec!["192.168.1.10/32".to_owned()],
        vec!["127.0.0.0/8".to_owned()],
        true,
        1,
        Some("lan-user".to_owned()),
    );
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(app.world(), root, "已启用 · 1 个账号"));

    let password_source = {
        let mut fields = app
            .world_mut()
            .query::<(&LanAuthPasswordField, &bevy::ecs::hierarchy::Children)>();
        *fields
            .single(app.world())
            .expect("LAN password field")
            .1
            .iter()
            .next()
            .expect("LAN password text field")
    };
    app.world_mut()
        .get_mut::<TextField>(password_source)
        .expect("LAN password text field state")
        .0
        .apply(TextFieldInput::SetText("secret-value".to_owned()));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<LanSecurityApplyButton>>()
        .single(app.world())
        .expect("LAN security apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetLanSecurity {
            allowed_ips: vec!["192.168.1.0/24".to_owned()],
            disallowed_ips: vec!["192.168.1.10/32".to_owned()],
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
            authentication_enabled: true,
            credentials: Some(LanCredentials {
                username: "lan-user".to_owned(),
                password: "secret-value".to_owned(),
            }),
        }]
    );
    assert!(!format!("{:?}", sink.submitted()).contains("secret-value"));
    assert!(
        app.world()
            .get::<TextField>(password_source)
            .expect("password field after submit")
            .0
            .text()
            .is_empty()
    );

    let _ = app
        .world_mut()
        .query::<&LanAllowedIpsField>()
        .single(app.world())
        .expect("LAN allowed field");
    let _ = app
        .world_mut()
        .query::<&LanDisallowedIpsField>()
        .single(app.world())
        .expect("LAN denied field");
    let _ = app
        .world_mut()
        .query::<&LanSkipAuthPrefixesField>()
        .single(app.world())
        .expect("LAN skip-auth field");
    let _ = app
        .world_mut()
        .query::<&LanAuthenticationToggle>()
        .single(app.world())
        .expect("LAN auth toggle");
    let _ = app
        .world_mut()
        .query::<&LanAuthUsernameField>()
        .single(app.world())
        .expect("LAN username field");
}

#[test]
fn test_settings_ipv6_routing_projects_and_submits_live_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.ipv6_routing = Ipv6RoutingSnapshot::new(7, false, true);
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "已禁用 IPv6（防止旁路泄漏） · TUN 已启用"
    ));

    let source = {
        let mut toggles = app
            .world_mut()
            .query::<(&Ipv6RoutingToggle, &bevy::ecs::hierarchy::Children)>();
        *toggles
            .single(app.world())
            .expect("IPv6 routing toggle")
            .1
            .iter()
            .next()
            .expect("IPv6 routing checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: true,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetIpv6Routing { enabled: true }]
    );
}

#[test]
fn test_settings_pac_projection_and_apply_submit_shared_request() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.pac = PacSnapshot {
        state: PacServiceState::Running {
            url: "http://127.0.0.1:31000/proxy.pac".to_owned(),
        },
        script_bytes: 2048,
        bypass_domains: vec!["example.com".to_owned(), "*.lan".to_owned()],
        revision: 3,
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "运行中 · http://127.0.0.1:31000/proxy.pac · 2048 bytes"
    ));

    let apply_button = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<PacApplyButton>>()
        .single(app.world())
        .expect("PAC apply button");
    app.world_mut().commands().trigger(Activate {
        entity: apply_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ApplyPac {
            enabled: true,
            bypass_domains: vec!["example.com".to_owned(), "*.lan".to_owned()],
            bypass_lan: true,
            minify: false,
        }]
    );
    let _ = app
        .world_mut()
        .query::<&PacBypassField>()
        .single(app.world())
        .expect("PAC bypass field");
}

#[test]
fn test_settings_tun_stack_catalog_has_three_live_values_and_safe_lwip() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut buttons = app
        .world_mut()
        .query::<(Entity, &TunStackButton, &TunStackButtonAvailability)>();
    let entries: Vec<(Entity, TunStack, bool)> = buttons
        .iter(app.world())
        .map(|(entity, button, availability)| (entity, button.stack, availability.0))
        .collect();
    assert_eq!(entries.len(), TunStack::ALL.len());
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::Gvisor && *available)
    );
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::System && *available)
    );
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::Mixed && *available)
    );
    let (lwip_entity, _, lwip_availability) = entries
        .iter()
        .find(|(_, stack, _)| *stack == TunStack::Lwip)
        .expect("LWIP reference entry");
    assert!(!lwip_availability);

    app.world_mut().commands().trigger(Activate {
        entity: *lwip_entity,
    });
    app.update();
    assert!(sink.submitted().is_empty(), "reference-only LWIP is inert");

    let (mixed_entity, _, mixed_availability) = entries
        .iter()
        .find(|(_, stack, _)| *stack == TunStack::Mixed)
        .expect("Mixed button");
    assert!(*mixed_availability);
    app.world_mut().commands().trigger(Activate {
        entity: *mixed_entity,
    });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetTunStack(TunStack::Mixed)]
    );
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
    assert!(subtree_has_text(
        app.world(),
        root,
        "已扫描 3 个 UWP AppContainer · 已豁免 2 个"
    ));
}

#[test]
fn test_app_routing_uwp_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let exempt_all = {
        let mut actions = app.world_mut().query::<(Entity, &UwpActionButton)>();
        actions
            .iter(app.world())
            .find(|(_, action)| action.0 == UwpAction::ExemptAll)
            .map(|(entity, _)| entity)
            .expect("UWP exempt-all action")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: exempt_all });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAllUwpExemptions { exempt: true }]
    );
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
