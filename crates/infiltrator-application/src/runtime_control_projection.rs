//! One fold for typed configuration observations; missing reads never become disabled facts.

use crate::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::LanSecuritySnapshot;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_ports::runtime_configuration::{LanAuthenticationState, ObservedValue};
use std::sync::{Arc, Mutex};

pub fn ipv6_observation(readout: &RuntimeConfigurationReadout) -> Option<Ipv6RoutingSnapshot> {
    Some(Ipv6RoutingSnapshot::new(
        readout.ipv6.revision.value(),
        reported(&readout.ipv6.value.enabled)?,
        reported(&readout.ipv6.value.tun_enabled)?,
    ))
}

pub fn lan_security_observation(
    readout: &RuntimeConfigurationReadout,
) -> Option<LanSecuritySnapshot> {
    let (authentication_enabled, authentication_user_count, authentication_username) =
        match &readout.lan_security.value.authentication {
            LanAuthenticationState::Unreported => return None,
            LanAuthenticationState::Disabled => (false, 0, None),
            LanAuthenticationState::Enabled {
                user_count,
                username,
            } => (*user_count > 0, *user_count, reported_optional(username)),
        };
    Some(LanSecuritySnapshot::new(
        readout.lan_security.revision.value(),
        reported(&readout.lan_security.value.allowed_ips)?,
        reported(&readout.lan_security.value.disallowed_ips)?,
        reported(&readout.lan_security.value.skip_auth_prefixes)?,
        authentication_enabled,
        authentication_user_count,
        authentication_username,
    ))
}

#[derive(Clone, Default)]
pub struct RuntimeControlApplication {
    state: Arc<Mutex<(u64, u64, RuntimeControlSnapshot)>>,
}

impl RuntimeControlApplication {
    pub fn observe(
        &self,
        generation: u64,
        revision: u64,
        result: Option<&Result<RuntimeConfigurationReadout, Failure>>,
    ) -> RuntimeControlSnapshot {
        let mut state = self.state.lock().expect("runtime control observation");
        if (generation, revision) < (state.0, state.1) {
            return state.2.clone();
        }
        if state.0 != generation {
            state.2 = RuntimeControlSnapshot::default();
        }
        let incoming = runtime_control(result);
        if incoming.status == RuntimeControlStatus::Ready {
            state.2 = incoming;
        } else {
            state.2.status = incoming.status;
        }
        state.0 = generation;
        state.1 = revision;
        state.2.clone()
    }
}

pub fn runtime_control(
    result: Option<&Result<RuntimeConfigurationReadout, Failure>>,
) -> RuntimeControlSnapshot {
    match result {
        None => RuntimeControlSnapshot::default(),
        Some(Err(failure)) => RuntimeControlSnapshot {
            status: if failure.code == ErrorCode::Unsupported {
                RuntimeControlStatus::Unsupported {
                    failure: failure.clone(),
                }
            } else {
                RuntimeControlStatus::Failed {
                    failure: failure.clone(),
                }
            },
            ..Default::default()
        },
        Some(Ok(readout)) => RuntimeControlSnapshot {
            status: RuntimeControlStatus::Ready,
            tun_enabled: reported(&readout.tun.value.enabled),
            mixed_port: reported(&readout.lan_sharing.value.mixed_port),
            allow_lan: reported(&readout.lan_sharing.value.enabled),
            log_level: match &readout.core_log_level.value {
                ObservedValue::Reported(level) => Some(level.as_key().to_owned()),
                ObservedValue::Unreported => None,
            },
            ipv6_routing: ipv6_observation(readout),
            lan_bind_address: reported(&readout.lan_sharing.value.bind_address),
            lan_security: lan_security_observation(readout),
            tun_stack: reported(&readout.tun.value.stack),
            tun_auto_route: reported(&readout.tun.value.auto_route),
            tun_strict_route: reported(&readout.tun.value.strict_route),
        },
    }
}

fn reported<T: Clone>(value: &ObservedValue<T>) -> Option<T> {
    match value {
        ObservedValue::Unreported => None,
        ObservedValue::Reported(value) => Some(value.clone()),
    }
}

fn reported_optional<T: Clone>(value: &ObservedValue<T>) -> Option<T> {
    reported(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing_intent_application::RoutingIntentApplication;
    use crate::runtime_configuration_application::RuntimeConfigurationReadout;
    use crate::system_toggle_application::SystemToggleApplication;
    use infiltrator_contract::command::CoreLogLevel;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use infiltrator_contract::routing_intent::{
        RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
    };
    use infiltrator_contract::session::SessionToken;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
    use infiltrator_contract::system_toggle::SystemToggleState;
    use infiltrator_ports::runtime_configuration::{
        ConfigurationObservation, ConfigurationRevision, DnsRuntimeConfiguration,
        Ipv6Configuration, LanAuthenticationState, LanSecurityConfiguration,
        LanSharingConfiguration, ObservedValue, ProxyPortsConfiguration, SnifferConfiguration,
        TunConfiguration,
    };

    fn scope() -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("runtime-control").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: Some(SessionToken::new(1)),
        }
    }

    fn readout(tun_enabled: bool, lan_port: u16) -> RuntimeConfigurationReadout {
        let scope = scope();
        let revision = ConfigurationRevision::new(1);
        RuntimeConfigurationReadout {
            tun: ConfigurationObservation {
                scope: scope.clone(),
                revision,
                value: TunConfiguration {
                    enabled: ObservedValue::Reported(tun_enabled),
                    ..Default::default()
                },
            },
            lan_sharing: ConfigurationObservation {
                scope: scope.clone(),
                revision,
                value: LanSharingConfiguration {
                    enabled: ObservedValue::Reported(false),
                    mixed_port: ObservedValue::Reported(lan_port),
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

    #[test]
    fn failed_zero_and_generation_changes_remain_distinct_and_old_reads_cannot_replace_facts() {
        let owner = RuntimeControlApplication::default();
        let unknown = owner.observe(4, 1, None);
        assert_eq!(unknown.tun_enabled, None);
        let config = readout(false, 0);
        let observed = owner.observe(4, 2, Some(&Ok(config.clone())));
        assert_eq!(observed.tun_enabled, Some(false));
        assert_eq!(observed.mixed_port, Some(0));
        let denied = Failure::new(ErrorCode::Authentication, "read denied", false);
        let failed = owner.observe(4, 3, Some(&Err(denied.clone())));
        assert_eq!(
            failed.status,
            RuntimeControlStatus::Failed { failure: denied }
        );
        assert_eq!(failed.tun_enabled, observed.tun_enabled);
        assert_eq!(owner.observe(4, 2, Some(&Ok(config.clone()))), failed);
        assert_eq!(owner.observe(5, 1, None), RuntimeControlSnapshot::default());
        assert_eq!(
            owner.observe(4, 99, Some(&Ok(config))),
            RuntimeControlSnapshot::default()
        );
    }

    #[test]
    fn typed_log_level_readout_restores_the_existing_settings_projection() {
        let mut readout = readout(false, 7890);
        readout.core_log_level.value = ObservedValue::Reported(CoreLogLevel::Warn);
        assert_eq!(
            runtime_control(Some(&Ok(readout))).log_level.as_deref(),
            Some("warn")
        );
    }

    #[test]
    fn preference_page_success_and_overview_presence_do_not_invent_route_or_disabled_tun() {
        let mut surface = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::unsupported("uncomposed controller"),
        );
        surface.pages.settings.status = PageStatus::Ready;
        surface.core.lifecycle = CoreLifecycle::Running;
        assert_eq!(
            surface.core.routing_intent.status,
            RoutingIntentStatus::Unobserved
        );
        assert_eq!(surface.core.routing_intent.current, None);
        assert_eq!(
            SystemToggleApplication::from_surface(&surface).tun,
            SystemToggleState::Unknown
        );
        surface.runtime_control = runtime_control(Some(&Ok(readout(false, 0))));
        let routing_intent = RoutingIntentApplication::from_surface(&surface);
        assert_eq!(routing_intent.current, None);
        assert_eq!(
            SystemToggleApplication::from_surface(&surface).tun,
            SystemToggleState::Disabled
        );
        surface.core.routing_intent = RoutingIntentSnapshot::demo_fixture();
        surface.core.lifecycle = CoreLifecycle::Stopped;
        assert_eq!(
            RoutingIntentApplication::from_surface(&surface).status,
            RoutingIntentStatus::Unobserved
        );
        assert_eq!(
            RoutingIntentApplication::from_surface(&surface).current,
            Some(RoutingIntent::FollowRules)
        );
    }
}
