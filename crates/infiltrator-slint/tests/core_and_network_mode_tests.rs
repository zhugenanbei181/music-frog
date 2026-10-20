//! SLINT-001.8: Core lifecycle and network mode behaviour of the real Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness. The routing mapping and the network mode sequence are
//! owned by `infiltrator_slint::surface::action`; these tests assert the exact
//! typed owner command the surface emits and the typed failure it raises.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SettingsPageSnapshot, SurfaceSnapshot};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness;
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

fn desktop_snapshot() -> SurfaceSnapshot {
    SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "ready", false),
    )
}

/// Asserts that the surface dispatched exactly one owner command.
fn single_command(outcome: harness::SurfaceOutcome) -> CommandIntent {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Command(intent) => intent.clone(),
                other => panic!("expected an owner command, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// The owner commands the surface dispatched.
///
/// A single command is dispatched on its own and several commands are
/// dispatched as one sequence, so the assertion reads the commands of an
/// outcome either way: the gesture is judged by the commands it asks the
/// owner for, not by the carrier the surface used.
fn dispatched_commands(outcome: harness::SurfaceOutcome) -> Vec<CommandIntent> {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => actions
            .iter()
            .flat_map(|action| match action {
                SlintAction::Command(intent) => vec![intent.clone()],
                SlintAction::Sequence(commands) => commands.clone(),
                other => panic!("expected owner commands, got {other:?}"),
            })
            .collect(),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected owner commands, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
fn rejection(outcome: harness::SurfaceOutcome) -> Failure {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        harness::SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

#[test]
fn routing_mode_dispatches_the_shared_routing_intent() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.set_route_mode("Rule"));
    assert_eq!(
        command,
        CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::FollowRules
        }
    );

    let command = single_command(surface.set_route_mode("Global"));
    assert_eq!(
        command,
        CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::RouteAllThroughProxy
        }
    );

    let command = single_command(surface.set_route_mode("Direct"));
    assert_eq!(
        command,
        CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::BypassProxy
        }
    );
}

#[test]
fn unknown_route_mode_is_refused_with_the_typed_failure() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_route_mode("Neon"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown routing mode");
    assert_eq!(surface.notice(), "Unknown routing mode");
}

#[test]
fn routing_mode_without_an_owner_snapshot_is_refused_as_not_ready() {
    let surface = harness::SurfaceHarness::new(None).unwrap();

    let failure = rejection(surface.set_route_mode("Rule"));
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        failure.message,
        "The shared owner has not published its first snapshot"
    );
}

#[test]
fn network_mode_switches_from_proxy_to_tun_while_running() {
    let mut snapshot = desktop_snapshot();
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(false);
    snapshot.pages.settings = PageData::ready(settings);

    let surface = harness::SurfaceHarness::new(Some(snapshot)).unwrap();

    match surface.set_network_mode("TUN") {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Sequence(commands) => assert_eq!(
                    commands,
                    &[
                        CommandIntent::StopProxy,
                        CommandIntent::SetSystemProxy { enabled: false },
                        CommandIntent::ToggleTun { enabled: true },
                        CommandIntent::StartProxy,
                    ]
                ),
                other => panic!("expected an owner command sequence, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command sequence, got the failure {failure:?}")
        }
    }
}

#[test]
fn network_mode_switches_to_core_only_while_stopped() {
    let mut snapshot = desktop_snapshot();
    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    snapshot.pages.settings = PageData::ready(settings);
    // The TUN device is only turned off when the runtime confirmed that it
    // runs, so the owner publishes the readback.
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(true),
        ..RuntimeControlSnapshot::default()
    };

    let surface = harness::SurfaceHarness::new(Some(snapshot)).unwrap();

    match surface.set_network_mode("Core only") {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Sequence(commands) => assert_eq!(
                    commands,
                    &[
                        CommandIntent::SetSystemProxy { enabled: false },
                        CommandIntent::ToggleTun { enabled: false },
                    ]
                ),
                other => panic!("expected an owner command sequence, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command sequence, got the failure {failure:?}")
        }
    }
}

#[test]
fn network_mode_leaves_an_unconfirmed_tun_device_alone() {
    let mut snapshot = desktop_snapshot();
    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    snapshot.pages.settings = PageData::ready(settings);

    let surface = harness::SurfaceHarness::new(Some(snapshot)).unwrap();

    // The runtime never confirmed the TUN device, so the surface does not
    // claim that it runs and does not ask the owner to turn it off. The
    // system proxy readback is the fact the traffic mode follows, so only
    // the system proxy is switched off.
    assert_eq!(
        dispatched_commands(surface.set_network_mode("Core only")),
        vec![CommandIntent::SetSystemProxy { enabled: false }],
        "the unconfirmed TUN device must not be switched off"
    );
}

#[test]
fn unknown_network_mode_is_refused_with_the_typed_failure() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let failure = rejection(surface.set_network_mode("Bridge"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown network mode");
    assert_eq!(surface.notice(), "Unknown network mode");
}

#[test]
fn core_lifecycle_projection_labels_come_from_the_shared_snapshot() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    for (lifecycle, label) in [
        (CoreLifecycle::Starting, "Starting"),
        (CoreLifecycle::Ready, "Running"),
        (CoreLifecycle::Running, "Running"),
        (CoreLifecycle::Stopping, "Stopping"),
        (CoreLifecycle::Stopped, "Stopped"),
        (CoreLifecycle::Failed, "Failed"),
    ] {
        let mut snapshot = desktop_snapshot();
        snapshot.core.lifecycle = lifecycle.clone();
        surface.publish_snapshot(snapshot);
        assert_eq!(surface.runtime_status(), label, "lifecycle {lifecycle:?}");
    }
}

#[test]
fn the_projection_labels_the_route_and_network_mode_from_the_shared_snapshot() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let mut snapshot = desktop_snapshot();
    snapshot.core.routing_intent.current = Some(RoutingIntent::FollowRules);
    snapshot.system_proxy.status = SystemProxyStatus::Disabled;
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(false);
    snapshot.pages.settings = PageData::ready(settings);
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.route_mode(), "Rule");
    assert_eq!(surface.network_mode(), "Core only");

    let mut snapshot = desktop_snapshot();
    snapshot.core.routing_intent.current = Some(RoutingIntent::RouteAllThroughProxy);
    snapshot.system_proxy.status = SystemProxyStatus::Enabled;
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    snapshot.pages.settings = PageData::ready(settings);
    // The traffic mode is the runtime control's readback, so the TUN device
    // is only reported once the runtime confirmed it.
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(true),
        ..RuntimeControlSnapshot::default()
    };
    surface.publish_snapshot(snapshot);
    assert_eq!(surface.route_mode(), "Global");
    assert_eq!(surface.network_mode(), "TUN");
}

#[test]
fn a_configured_tun_device_the_runtime_never_confirmed_is_not_a_traffic_mode() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let mut snapshot = desktop_snapshot();
    let mut settings = default_settings_snapshot();
    settings.tun_enabled = Some(true);
    snapshot.pages.settings = PageData::ready(settings);
    surface.publish_snapshot(snapshot);

    // The configuration asks for the TUN device, but the runtime published no
    // readback, so the surface reports no confirmed mode at all.
    assert_eq!(surface.network_mode(), "Unknown");
    assert!(surface.window().get_network_mode_status_warning());
}
