//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::RoutingIntentStatus;
use infiltrator_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::runtime_configuration::{
    ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration, Ipv6Configuration,
    LanAuthenticationState, LanSecurityConfiguration, LanSharingConfiguration, ObservedValue,
    ProxyPortsConfiguration, SnifferConfiguration, TunConfiguration,
};

fn unreported_runtime_readout() -> RuntimeConfigurationReadout {
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("engine"),
            CoreInstanceId::new("routing-intent-readout").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(93)),
    };
    let revision = ConfigurationRevision::new(1);
    RuntimeConfigurationReadout {
        tun: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: TunConfiguration::default(),
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
                tun_enabled: ObservedValue::Unreported,
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
        proxy_ports: ConfigurationObservation {
            scope: scope.clone(),
            revision,
            value: ProxyPortsConfiguration::default(),
        },
        core_log_level: ConfigurationObservation {
            scope,
            revision,
            value: ObservedValue::Unreported,
        },
    }
}

#[test]
fn runtime_config_refresh_does_not_replace_typed_routing_observation() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let initial = RoutingIntentSnapshot::demo_fixture();
    state
        .runtime
        .routing_intent_actions
        .observe(generation, 1, initial.clone());
    state.runtime.routing_intent_state = initial;
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);
    let readout = unreported_runtime_readout();

    let request = state
        .runtime
        .routing_intent_actions
        .begin(RoutingIntent::RouteAllThroughProxy)
        .unwrap();
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(readout.clone()),
        generation,
    ));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::FollowRules)
    );
    assert_eq!(
        state.runtime.routing_intent_actions.pending.as_ref(),
        Some(&request)
    );

    let _ = state.update(Message::RoutingIntentFinished {
        request,
        result: Ok(RoutingIntent::RouteAllThroughProxy),
    });
    let _ = state.update(Message::RuntimeConfigFetched(Ok(readout), generation));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
}

#[test]
fn failed_read_blocks_retry_until_typed_surface_observation_recovers() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let initial = RoutingIntentSnapshot::demo_fixture();
    state
        .runtime
        .routing_intent_actions
        .observe(generation, 1, initial.clone());
    state.runtime.routing_intent_state = initial;
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);

    let request = state
        .runtime
        .routing_intent_actions
        .begin(RoutingIntent::RouteAllThroughProxy)
        .unwrap();
    let write_failure = Failure::new(ErrorCode::Network, "routing write failed", true);
    let _ = state.update(Message::RoutingIntentFinished {
        request,
        result: Err(write_failure.clone()),
    });
    assert_eq!(
        state.runtime.routing_intent_actions.retry_target(),
        Some(RoutingIntent::RouteAllThroughProxy)
    );

    let read_failure = Failure::new(ErrorCode::Authentication, "controller read denied", false);
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("fixture surface"),
    );
    surface.generation = generation;
    surface.revision = state
        .runtime
        .routing_intent_actions
        .next_observation_revision();
    surface.core.lifecycle = CoreLifecycle::Running;
    surface.core.routing_intent = RoutingIntentSnapshot {
        current: None,
        status: RoutingIntentStatus::Failed,
        failure: Some(read_failure.clone()),
    };
    assert!(state.apply_shared_surface_snapshot(surface.clone()));
    assert_eq!(
        state.runtime.routing_intent_actions.retry_target(),
        None,
        "a failed typed read disables retries until current routing facts recover"
    );
    assert_eq!(
        state.runtime.routing_intent_actions.failure,
        Some(write_failure.clone()),
        "the original write failure remains visible"
    );
    assert_eq!(
        state.runtime.routing_intent_actions.observed.failure,
        Some(read_failure)
    );
    surface.revision += 1;
    surface.core.routing_intent = RoutingIntentSnapshot::demo_fixture();
    assert!(state.apply_shared_surface_snapshot(surface));
    assert_eq!(
        state.runtime.routing_intent_actions.retry_target(),
        Some(RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        state.runtime.routing_intent_actions.failure,
        Some(write_failure)
    );

    let _ = state.update(Message::DismissRoutingIntentFailure);
    assert!(state.runtime.routing_intent_actions.failure.is_none());
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::FollowRules)
    );
}

#[test]
fn mismatched_typed_receipt_preserves_observed_intent_and_exposes_retry() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    state.runtime.routing_intent_actions.observe(
        generation,
        1,
        RoutingIntentSnapshot::demo_fixture(),
    );
    let request = state
        .runtime
        .routing_intent_actions
        .begin(RoutingIntent::RouteAllThroughProxy)
        .unwrap();
    let _ = state.update(Message::RoutingIntentFinished {
        request,
        result: Ok(RoutingIntent::BypassProxy),
    });
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::BypassProxy)
    );
    assert_eq!(
        state
            .runtime
            .routing_intent_actions
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(
        state.runtime.routing_intent_actions.retry_target(),
        Some(RoutingIntent::RouteAllThroughProxy)
    );
    assert!(state.runtime.routing_intent_actions.pending.is_none());
    assert!(state.shell.error_msg.is_none());
}
