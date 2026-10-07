//! One fold for the controller result; missing reads never become disabled facts.
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::LanSecuritySnapshot;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_domain::runtime::ConfigSnapshot;
use infiltrator_ports::error::PortError;
use std::sync::{Arc, Mutex};

pub fn ipv6_observation(config: &ConfigSnapshot) -> Option<Ipv6RoutingSnapshot> {
    Some(Ipv6RoutingSnapshot::new(
        0,
        config.ipv6?,
        config.tun.as_ref()?.enable?,
    ))
}

pub fn lan_security_observation(config: &ConfigSnapshot) -> Option<LanSecuritySnapshot> {
    Some(LanSecuritySnapshot::new(
        0,
        config.lan_allowed_ips.clone()?,
        config.lan_disallowed_ips.clone()?,
        config.skip_auth_prefixes.clone()?,
        config.authentication_enabled?,
        config.authentication_user_count?,
        config.authentication_username.clone(),
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
        result: Option<&Result<ConfigSnapshot, PortError>>,
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
    result: Option<&Result<ConfigSnapshot, PortError>>,
) -> RuntimeControlSnapshot {
    match result {
        None => RuntimeControlSnapshot::default(),
        Some(Err(error)) => {
            let failure = Failure::from(error.clone());
            RuntimeControlSnapshot {
                status: if failure.code == ErrorCode::Unsupported {
                    RuntimeControlStatus::Unsupported { failure }
                } else {
                    RuntimeControlStatus::Failed { failure }
                },
                ..Default::default()
            }
        }
        Some(Ok(config)) => {
            let Some(mode) = ProxyMode::from_wire(&config.mode) else {
                return RuntimeControlSnapshot {
                    status: RuntimeControlStatus::Failed {
                        failure: Failure::new(
                            ErrorCode::InvalidInput,
                            "The controller reported an unknown proxy mode",
                            false,
                        ),
                    },
                    ..Default::default()
                };
            };
            RuntimeControlSnapshot {
                status: RuntimeControlStatus::Ready,
                mode: Some(mode),
                script_available: config.script.as_ref().map(|script| !script.is_null()),
                tun_enabled: config.tun.as_ref().and_then(|tun| tun.enable),
                mixed_port: Some(config.mixed_port),
                allow_lan: Some(config.allow_lan),
                log_level: (!config.log_level.is_empty()).then(|| config.log_level.clone()),
                ipv6_routing: ipv6_observation(config),
                lan_bind_address: config.bind_address.clone(),
                lan_security: lan_security_observation(config),
                tun_stack: config.tun.as_ref().and_then(|tun| tun.stack.clone()),
                tun_auto_route: config.tun.as_ref().and_then(|tun| tun.auto_route),
                tun_strict_route: config.tun.as_ref().and_then(|tun| tun.strict_route),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy_mode_application::ProxyModeApplication;
    use crate::system_toggle_application::SystemToggleApplication;
    use infiltrator_contract::proxy_mode::ProxyModeStatus;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
    use infiltrator_contract::system_toggle::SystemToggleState;
    use infiltrator_domain::runtime::TunSnapshot;

    #[test]
    fn unknown_failed_zero_and_generation_changes_remain_distinct_and_old_reads_cannot_replace_facts()
     {
        let owner = RuntimeControlApplication::default();
        let unknown = owner.observe(4, 1, None);
        assert_eq!(unknown.mode, None);
        assert_eq!(unknown.tun_enabled, None);
        assert_eq!(unknown.script_available, None);
        let config = ConfigSnapshot {
            mode: "global".into(),
            mixed_port: 0,
            allow_lan: false,
            tun: Some(TunSnapshot {
                enable: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        let observed = owner.observe(4, 2, Some(&Ok(config.clone())));
        assert_eq!(observed.mode, Some(ProxyMode::Global));
        assert_eq!(observed.tun_enabled, Some(false));
        assert_eq!(observed.mixed_port, Some(0));
        assert_eq!(observed.script_available, None);
        let denied = Failure::new(ErrorCode::Authentication, "read denied", false);
        let failed = owner.observe(4, 3, Some(&Err(PortError::Rejected(denied.clone()))));
        assert_eq!(
            failed.status,
            RuntimeControlStatus::Failed { failure: denied }
        );
        assert_eq!(failed.mode, observed.mode);
        assert_eq!(failed.tun_enabled, observed.tun_enabled);
        assert_eq!(owner.observe(4, 2, Some(&Ok(config.clone()))), failed);
        let mut invalid = config.clone();
        invalid.mode = "invented".into();
        assert!(matches!(runtime_control(Some(&Ok(invalid))).status,
            RuntimeControlStatus::Failed { failure } if failure.code == ErrorCode::InvalidInput));
        assert_eq!(owner.observe(5, 1, None), RuntimeControlSnapshot::default());
        assert_eq!(
            owner.observe(4, 99, Some(&Ok(config))),
            RuntimeControlSnapshot::default()
        );
    }

    #[test]
    fn preference_page_success_and_overview_presence_do_not_invent_mode_script_or_disabled_tun() {
        let mut surface = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::unsupported("uncomposed controller"),
        );
        surface.pages.settings.status = PageStatus::Ready;
        surface.core.lifecycle = CoreLifecycle::Running;
        let mode = ProxyModeApplication::from_surface(&surface);
        assert_eq!(mode.status, ProxyModeStatus::Unobserved);
        assert_eq!(mode.current, None);
        assert_eq!(mode.script_available, None);
        assert!(ProxyModeApplication::intent(&mode, ProxyMode::Global).is_err());
        assert_eq!(
            SystemToggleApplication::from_surface(&surface).tun,
            SystemToggleState::Unknown
        );
        let config = ConfigSnapshot {
            mode: "direct".into(),
            ..Default::default()
        };
        surface.runtime_control = runtime_control(Some(&Ok(config)));
        let mode = ProxyModeApplication::from_surface(&surface);
        assert_eq!(mode.current, Some(ProxyMode::Direct));
        assert_eq!(mode.script_available, None);
        assert!(ProxyModeApplication::intent(&mode, ProxyMode::Script).is_err());
        assert_eq!(
            SystemToggleApplication::from_surface(&surface).tun,
            SystemToggleState::Unknown
        );
        surface.core.lifecycle = CoreLifecycle::Stopped;
        assert_eq!(
            ProxyModeApplication::from_surface(&surface).status,
            ProxyModeStatus::Unobserved
        );
    }
}
