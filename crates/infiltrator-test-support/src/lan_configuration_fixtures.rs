//! Isolated LAN configuration port fixture.
//!
//! The fixture publishes the live LAN sharing and LAN security facts of one
//! engine scope. Every write is fenced by the configuration revision the
//! caller observed: a stale revision is refused with a typed `NotReady`
//! failure instead of overwriting a newer configuration.

use async_trait::async_trait;
use infiltrator_contract::lan::LanCredentials;
use infiltrator_ports::runtime_configuration::{
    ConfigurationApply, ConfigurationObservation, ConfigurationRevision, LanAuthenticationState,
    LanAuthenticationUpdate, LanConfigurationPort, LanSecurityConfiguration, LanSecurityUpdate,
    LanSharingConfiguration, LanSharingUpdate, ObservedValue,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// The mixed proxy port the fixture publishes.
pub const CAPTURE_MIXED_PORT: u16 = 7890;
/// The bind address the fixture publishes.
pub const CAPTURE_BIND_ADDRESS: &str = "192.168.1.10";
/// The LAN peer the fixture allows.
pub const CAPTURE_ALLOWED_IP: &str = "192.168.0.0/16";

/// An isolated LAN configuration port that answers with real LAN facts.
pub struct LanConfigurationCapturePort {
    sharing: Mutex<ConfigurationObservation<LanSharingConfiguration>>,
    security: Mutex<ConfigurationObservation<LanSecurityConfiguration>>,
}

impl LanConfigurationCapturePort {
    /// A capture that publishes LAN sharing on and LAN security enabled.
    pub fn new(scope: CoreScope) -> Self {
        Self {
            sharing: Mutex::new(ConfigurationObservation {
                scope: scope.clone(),
                revision: ConfigurationRevision::new(1),
                value: LanSharingConfiguration {
                    enabled: ObservedValue::Reported(true),
                    mixed_port: ObservedValue::Reported(CAPTURE_MIXED_PORT),
                    bind_address: ObservedValue::Reported(CAPTURE_BIND_ADDRESS.to_owned()),
                },
            }),
            security: Mutex::new(ConfigurationObservation {
                scope,
                revision: ConfigurationRevision::new(1),
                value: LanSecurityConfiguration {
                    allowed_ips: ObservedValue::Reported(vec![CAPTURE_ALLOWED_IP.to_owned()]),
                    disallowed_ips: ObservedValue::Reported(Vec::new()),
                    skip_auth_prefixes: ObservedValue::Reported(Vec::new()),
                    authentication: LanAuthenticationState::Enabled {
                        user_count: 1,
                        username: ObservedValue::Reported("lan-user".to_owned()),
                    },
                },
            }),
        }
    }

    /// The LAN sharing observation the port currently publishes.
    pub fn current_sharing(&self) -> ConfigurationObservation<LanSharingConfiguration> {
        self.sharing
            .lock()
            .expect("LAN sharing capture state")
            .clone()
    }

    /// The LAN security observation the port currently publishes.
    pub fn current_security(&self) -> ConfigurationObservation<LanSecurityConfiguration> {
        self.security
            .lock()
            .expect("LAN security capture state")
            .clone()
    }
}

fn require_scope(observed: &CoreScope, requested: &CoreScope) -> Result<(), PortError> {
    if observed.identity.engine == requested.identity.engine {
        return Ok(());
    }
    Err(PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        format!(
            "capture LAN configuration owns engine {} and cannot serve {}",
            observed.identity.engine.as_str(),
            requested.identity.engine.as_str()
        ),
        false,
    )))
}

fn require_revision(
    expected: ConfigurationRevision,
    observed: ConfigurationRevision,
) -> Result<(), PortError> {
    if expected == observed {
        return Ok(());
    }
    Err(PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        format!(
            "stale LAN configuration revision: expected {}, observed {}",
            expected.value(),
            observed.value()
        ),
        true,
    )))
}

#[async_trait]
impl LanConfigurationPort for LanConfigurationCapturePort {
    async fn observe_sharing(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        let observation = self.current_sharing();
        require_scope(&observation.scope, scope)?;
        Ok(observation)
    }

    async fn apply_sharing(
        &self,
        request: ConfigurationApply<LanSharingUpdate>,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        let mut observation = self.sharing.lock().expect("LAN sharing capture state");
        require_scope(&observation.scope, &request.scope)?;
        require_revision(request.expected_revision, observation.revision)?;
        observation.value = LanSharingConfiguration {
            enabled: ObservedValue::Reported(request.update.enabled),
            mixed_port: ObservedValue::Reported(request.update.mixed_port),
            bind_address: ObservedValue::Reported(request.update.bind_address),
        };
        observation.revision = ConfigurationRevision::new(observation.revision.value() + 1);
        Ok(observation.clone())
    }

    async fn observe_security(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let observation = self.current_security();
        require_scope(&observation.scope, scope)?;
        Ok(observation)
    }

    async fn apply_security(
        &self,
        request: ConfigurationApply<LanSecurityUpdate>,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let mut observation = self.security.lock().expect("LAN security capture state");
        require_scope(&observation.scope, &request.scope)?;
        require_revision(request.expected_revision, observation.revision)?;
        let authentication = match request.update.authentication {
            LanAuthenticationUpdate::Disabled => LanAuthenticationState::Disabled,
            LanAuthenticationUpdate::Enabled(LanCredentials { username, .. }) => {
                LanAuthenticationState::Enabled {
                    user_count: 1,
                    username: ObservedValue::Reported(username),
                }
            }
        };
        observation.value = LanSecurityConfiguration {
            allowed_ips: ObservedValue::Reported(request.update.allowed_ips),
            disallowed_ips: ObservedValue::Reported(request.update.disallowed_ips),
            skip_auth_prefixes: ObservedValue::Reported(request.update.skip_auth_prefixes),
            authentication,
        };
        observation.revision = ConfigurationRevision::new(observation.revision.value() + 1);
        Ok(observation.clone())
    }
}
