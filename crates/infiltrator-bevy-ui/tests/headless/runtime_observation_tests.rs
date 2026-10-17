//! test-intent: behavior
//! Native shell controls distinguish missing, observed and failed controller facts.
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::feathers::theme::{SurfaceLevel, UiTheme};
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::{InlineImage, Text};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_bevy_ui::app::{
    GlobalStatusDot, PendingRoutingIntentRequest, RoutingIntentStatusCapsule, ShellPlugin,
    SidebarTunToggle,
};
use infiltrator_bevy_ui::pages::overview::OverviewRoutingIntentPill;
use infiltrator_bevy_ui::route::{OverviewSourceHandle, PagesPlugin};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::RoutingIntentStatus;
use infiltrator_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::runtime_configuration::{
    ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration, Ipv6Configuration,
    LanAuthenticationState, LanSecurityConfiguration, LanSharingConfiguration, ObservedValue,
    ProxyPortsConfiguration, SnifferConfiguration, TunConfiguration,
};
use lurker_core_contract::session::SessionToken;

fn tun_readout() -> RuntimeConfigurationReadout {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("bevy-runtime-observation").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let revision = ConfigurationRevision::new(1);
    RuntimeConfigurationReadout {
        tun: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: TunConfiguration {
                enabled: ObservedValue::Reported(false),
                ..Default::default()
            },
        },
        lan_sharing: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSharingConfiguration {
                enabled: ObservedValue::Unreported,
                mixed_port: ObservedValue::Unreported,
                bind_address: ObservedValue::Unreported,
            },
        },
        lan_security: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: LanSecurityConfiguration {
                allowed_ips: ObservedValue::Unreported,
                disallowed_ips: ObservedValue::Unreported,
                skip_auth_prefixes: ObservedValue::Unreported,
                authentication: LanAuthenticationState::Unreported,
            },
        },
        ipv6: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: Ipv6Configuration {
                enabled: ObservedValue::Unreported,
                tun_enabled: ObservedValue::Reported(false),
            },
        },
        sniffer: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: SnifferConfiguration {
                enabled: ObservedValue::Unreported,
            },
        },
        dns: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: DnsRuntimeConfiguration::default(),
        },
        core_log_level: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: ObservedValue::Unreported,
        },
        proxy_ports: ConfigurationObservation {
            scope,
            revision,
            value: ProxyPortsConfiguration::default(),
        },
    }
}

fn caption(app: &App, capsule: Entity) -> String {
    app.world()
        .get::<Children>(capsule)
        .unwrap()
        .iter()
        .find_map(|child| app.world().get::<Text>(*child).map(|text| text.0.clone()))
        .unwrap()
}

#[test]
fn native_routing_and_tun_controls_remain_unknown_until_observed_and_failed_reads_block_actions() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ButtonPlugin, ShellPlugin::default(), PagesPlugin::default()));
    app.update();
    let capsule = app
        .world_mut()
        .query_filtered::<Entity, With<RoutingIntentStatusCapsule>>()
        .single(app.world())
        .unwrap();
    let route_all = app
        .world_mut()
        .query::<(Entity, &OverviewRoutingIntentPill)>()
        .iter(app.world())
        .find(|(_, pill)| pill.0 == RoutingIntent::RouteAllThroughProxy)
        .unwrap();
    let route_all = route_all.0;
    let tun = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarTunToggle>>()
        .single(app.world())
        .unwrap();
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let observations = RuntimeControlApplication::default();
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.routing_intent = RoutingIntentSnapshot::default();
    snapshot
        .pages
        .overview
        .data
        .as_mut()
        .unwrap()
        .routing_intent = RoutingIntentSnapshot::default();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    snapshot.runtime_control = observations.observe(snapshot.generation, snapshot.revision, None);
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(caption(&app, capsule), "未观测");
    assert!(app.world().get::<ButtonDisabled>(route_all).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(route_all).is_some());
    assert!(app.world().get::<ButtonDisabled>(tun).unwrap().0);
    let before = app
        .world()
        .resource::<OverviewSourceHandle>()
        .0
        .current()
        .routing_intent
        .current;
    click_entity(&mut app, route_all);
    assert!(
        app.world()
            .resource::<PendingRoutingIntentRequest>()
            .0
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<OverviewSourceHandle>()
            .0
            .current()
            .routing_intent
            .current,
        before
    );
    let config = tun_readout();
    snapshot.revision += 1;
    snapshot.runtime_control = observations.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Ok(config.clone())),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(caption(&app, capsule), "未观测");
    assert!(app.world().get::<ButtonDisabled>(route_all).unwrap().0);
    let ready_route = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    snapshot.revision += 1;
    snapshot.core.routing_intent = ready_route.clone();
    snapshot
        .pages
        .overview
        .data
        .as_mut()
        .unwrap()
        .routing_intent = ready_route.clone();
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(caption(&app, capsule), "全部流量经代理");
    let global = app
        .world_mut()
        .query::<(Entity, &OverviewRoutingIntentPill)>()
        .iter(app.world())
        .find(|(_, mode)| mode.0 == RoutingIntent::RouteAllThroughProxy)
        .unwrap()
        .0;
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_none());
    assert!(
        !app.world()
            .get::<ButtonDisabled>(tun)
            .is_some_and(|disabled| disabled.0)
    );
    assert_eq!(
        app.world_mut()
            .query::<&OverviewRoutingIntentPill>()
            .iter(app.world())
            .count(),
        3,
        "only the three shared routing choices are presented"
    );
    let denied = Failure::new(ErrorCode::Authentication, "controller read denied", false);
    snapshot.revision += 1;
    snapshot.runtime_control = observations.observe(
        snapshot.generation,
        snapshot.revision,
        Some(&Err(denied.clone())),
    );
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<(Entity, &OverviewRoutingIntentPill)>()
            .iter(app.world())
            .filter(|(entity, _)| !app.world().get::<ButtonDisabled>(*entity).unwrap().0)
            .count(),
        3,
        "a runtime config failure does not invalidate typed routing observation"
    );
    assert!(app.world().get::<ButtonDisabled>(tun).unwrap().0);
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_none());
    assert_eq!(caption(&app, capsule), "全部流量经代理");
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .runtime_control
            .status,
        RuntimeControlStatus::Failed {
            failure: denied.clone(),
        }
    );
    let route_failure = RoutingIntentSnapshot {
        current: None,
        status: RoutingIntentStatus::Failed,
        failure: Some(denied.clone()),
    };
    snapshot.revision += 1;
    snapshot.core.routing_intent = route_failure.clone();
    snapshot
        .pages
        .overview
        .data
        .as_mut()
        .unwrap()
        .routing_intent = route_failure;
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert!(!app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_some());
    assert!(app.world().get::<ButtonDisabled>(global).unwrap().0);
    assert_ne!(caption(&app, capsule), "全部流量经代理");
    assert_eq!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .core
            .routing_intent
            .status,
        RoutingIntentStatus::Failed
    );

    snapshot.revision += 1;
    snapshot.core.routing_intent = ready_route.clone();
    snapshot
        .pages
        .overview
        .data
        .as_mut()
        .unwrap()
        .routing_intent = ready_route;
    app.world_mut()
        .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
    app.update();
    app.update();
    assert!(app.world().get::<ControlVisual>(global).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(global).is_none());
    assert!(!app.world().get::<ButtonDisabled>(global).unwrap().0);
    assert_eq!(caption(&app, capsule), "全部流量经代理");
}

#[test]
fn native_global_status_dot_replays_each_actual_lifecycle_without_remounting() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::default()));
    app.update();
    let dot = app
        .world_mut()
        .query_filtered::<Entity, With<GlobalStatusDot>>()
        .single(app.world())
        .unwrap();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    for (lifecycle, token) in [
        (CoreLifecycle::Stopped, Token::InkDim),
        (CoreLifecycle::Starting, Token::Warning),
        (CoreLifecycle::Ready, Token::Success),
        (CoreLifecycle::Running, Token::Success),
        (CoreLifecycle::Stopping, Token::Warning),
        (CoreLifecycle::Failed, Token::Danger),
    ] {
        snapshot.revision += 1;
        snapshot.core.lifecycle = lifecycle;
        app.world_mut()
            .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
        app.update();
        app.update();
        let expected_color = app
            .world()
            .resource::<UiTheme>()
            .context_color(&token.theme_token(), SurfaceLevel::Base);
        assert_eq!(
            app.world().get::<InlineImage>(dot).unwrap().color,
            expected_color
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<GlobalStatusDot>>()
                .single(app.world())
                .unwrap(),
            dot
        );
    }
}
