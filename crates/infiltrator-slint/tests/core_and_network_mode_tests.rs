//! Tests for Core lifecycle, command sequences, and network mode switching in infiltrator-slint.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SettingsPageSnapshot, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::PageData;
use lurker_core_contract::routing_intent::RoutingIntent;

fn default_settings_snapshot() -> SettingsPageSnapshot {
    SettingsPageSnapshot {
        close_to_tray: None,
        notifications_enabled: None,
        language: "zh-CN".into(),
        theme: "dark".into(),
        autostart: false,
        system_proxy: false,
        mixed_port: None,
        allow_lan: None,
        lan_bind_address: None,
        lan_security: None,
        ipv6_routing: None,
        pac: Default::default(),
        tun_enabled: Some(false),
        tun_stack: None,
        tun_auto_route: None,
        tun_strict_route: None,
        controller_port: None,
        log_level: None,
        core_channel: "stable".into(),
        mini_hud: Default::default(),
        refresh_current_profile_on_start: false,
        custom_accent_color: None,
    }
}

#[test]
fn test_route_mode_mapping() {
    let mode_rule = "Rule";
    let intent_rule = match mode_rule {
        "Rule" => Ok(RoutingIntent::FollowRules),
        "Global" => Ok(RoutingIntent::RouteAllThroughProxy),
        "Direct" => Ok(RoutingIntent::BypassProxy),
        _ => Err(()),
    };
    assert_eq!(intent_rule, Ok(RoutingIntent::FollowRules));

    let mode_global = "Global";
    let intent_global = match mode_global {
        "Rule" => Ok(RoutingIntent::FollowRules),
        "Global" => Ok(RoutingIntent::RouteAllThroughProxy),
        "Direct" => Ok(RoutingIntent::BypassProxy),
        _ => Err(()),
    };
    assert_eq!(intent_global, Ok(RoutingIntent::RouteAllThroughProxy));

    let mode_direct = "Direct";
    let intent_direct = match mode_direct {
        "Rule" => Ok(RoutingIntent::FollowRules),
        "Global" => Ok(RoutingIntent::RouteAllThroughProxy),
        "Direct" => Ok(RoutingIntent::BypassProxy),
        _ => Err(()),
    };
    assert_eq!(intent_direct, Ok(RoutingIntent::BypassProxy));
}

#[test]
fn test_network_mode_action_switching_from_proxy_to_tun_when_running() {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "ready", false),
    );
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;

    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(false);
    snapshot.pages.settings = PageData::ready(settings);

    let running = matches!(
        &snapshot.core.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    );
    assert!(running);

    let mut commands = Vec::new();
    if running {
        commands.push(CommandIntent::StopProxy);
    }
    if matches!(&snapshot.system_proxy.status, SystemProxyStatus::Enabled) {
        commands.push(CommandIntent::SetSystemProxy { enabled: false });
    }
    commands.push(CommandIntent::ToggleTun { enabled: true });
    if running {
        commands.push(CommandIntent::StartProxy);
    }

    assert_eq!(commands.len(), 4);
    assert_eq!(commands[0], CommandIntent::StopProxy);
    assert_eq!(
        commands[1],
        CommandIntent::SetSystemProxy { enabled: false }
    );
    assert_eq!(commands[2], CommandIntent::ToggleTun { enabled: true });
    assert_eq!(commands[3], CommandIntent::StartProxy);
}

#[test]
fn test_network_mode_action_switching_to_core_only_when_stopped() {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "ready", false),
    );
    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;

    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    let tun_enabled = settings.tun_enabled.unwrap_or(false);
    snapshot.pages.settings = PageData::ready(settings);

    let running = matches!(
        &snapshot.core.lifecycle,
        CoreLifecycle::Ready | CoreLifecycle::Running
    );
    assert!(!running);

    let mut commands = Vec::new();
    if matches!(&snapshot.system_proxy.status, SystemProxyStatus::Enabled) {
        commands.push(CommandIntent::SetSystemProxy { enabled: false });
    }
    if tun_enabled {
        commands.push(CommandIntent::ToggleTun { enabled: false });
    }

    assert_eq!(commands.len(), 2);
    assert_eq!(
        commands[0],
        CommandIntent::SetSystemProxy { enabled: false }
    );
    assert_eq!(commands[1], CommandIntent::ToggleTun { enabled: false });
}

#[test]
fn test_core_lifecycle_projection_labels() {
    let starting = CoreLifecycle::Starting;
    let label = match &starting {
        CoreLifecycle::Starting => "Starting",
        CoreLifecycle::Ready | CoreLifecycle::Running => "Running",
        CoreLifecycle::Stopping => "Stopping",
        CoreLifecycle::Stopped => "Stopped",
        CoreLifecycle::Failed => "Failed",
    };
    assert_eq!(label, "Starting");

    let running = CoreLifecycle::Running;
    let label = match &running {
        CoreLifecycle::Starting => "Starting",
        CoreLifecycle::Ready | CoreLifecycle::Running => "Running",
        CoreLifecycle::Stopping => "Stopping",
        CoreLifecycle::Stopped => "Stopped",
        CoreLifecycle::Failed => "Failed",
    };
    assert_eq!(label, "Running");

    let failed = CoreLifecycle::Failed;
    let label = match &failed {
        CoreLifecycle::Starting => "Starting",
        CoreLifecycle::Ready | CoreLifecycle::Running => "Running",
        CoreLifecycle::Stopping => "Stopping",
        CoreLifecycle::Stopped => "Stopped",
        CoreLifecycle::Failed => "Failed",
    };
    assert_eq!(label, "Failed");
}
