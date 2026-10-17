use super::*;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::tun::TunStack;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_configuration::{
    ConfigurationRevision, Ipv6ConfigurationPort, LanAuthenticationState, LanConfigurationPort,
    LanSecurityConfiguration, LanSecurityUpdate, LanSharingConfiguration, LanSharingUpdate,
    SnifferConfigurationPort, TunConfigurationPort,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::session::SessionToken;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FakeBehavior {
    Normal,
    Unsupported,
    StaleObservation,
    DifferentScope,
    ChangedReadbackRevision,
}

struct FakeTunState {
    scope: CoreScope,
    revision: ConfigurationRevision,
    config: TunConfiguration,
    behavior: FakeBehavior,
    observation_count: usize,
    writes: usize,
}

#[derive(Clone)]
struct FakeTunPort(Arc<Mutex<FakeTunState>>);

impl FakeTunPort {
    fn new(scope: CoreScope, behavior: FakeBehavior) -> Self {
        Self(Arc::new(Mutex::new(FakeTunState {
            scope,
            revision: ConfigurationRevision::new(4),
            config: TunConfiguration {
                enabled: ObservedValue::Reported(false),
                stack: ObservedValue::Reported("system".to_owned()),
                auto_route: ObservedValue::Reported(true),
                strict_route: ObservedValue::Reported(false),
                mtu: ObservedValue::Reported(1500),
            },
            behavior,
            observation_count: 0,
            writes: 0,
        })))
    }

    fn writes(&self) -> usize {
        self.0.lock().expect("fake TUN lock").writes
    }

    fn set_config(&self, config: TunConfiguration) {
        self.0.lock().expect("fake TUN lock").config = config;
    }
}

#[async_trait]
impl TunConfigurationPort for FakeTunPort {
    async fn observe(
        &self,
        _requested_scope: &CoreScope,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        let mut state = self.0.lock().expect("fake TUN lock");
        state.observation_count += 1;
        match state.behavior {
            FakeBehavior::Unsupported => Err(PortError::unsupported(
                Capability::Tun,
                "TUN configuration is unsupported in this fake",
            )),
            _ => {
                let scope = if state.behavior == FakeBehavior::DifferentScope {
                    other_scope()
                } else {
                    state.scope.clone()
                };
                let revision = match state.behavior {
                    FakeBehavior::StaleObservation => {
                        ConfigurationRevision::new(state.revision.value().saturating_sub(1))
                    }
                    FakeBehavior::ChangedReadbackRevision if state.observation_count > 1 => {
                        ConfigurationRevision::new(state.revision.value().saturating_add(1))
                    }
                    _ => state.revision,
                };
                Ok(ConfigurationObservation {
                    scope,
                    revision,
                    value: state.config.clone(),
                })
            }
        }
    }

    async fn apply(
        &self,
        request: ConfigurationApply<TunConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        let mut state = self.0.lock().expect("fake TUN lock");
        if request.scope != state.scope || request.expected_revision != state.revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "stale or mismatched TUN configuration request",
                false,
            )));
        }
        match request.update {
            TunConfigurationUpdate::SetEnabled(enabled) => {
                state.config.enabled = ObservedValue::Reported(enabled);
            }
            TunConfigurationUpdate::SetStack(stack) => {
                if !stack.is_live_supported() {
                    return Err(PortError::unsupported(
                        Capability::Tun,
                        "stack is not live supported",
                    ));
                }
                state.config.stack = ObservedValue::Reported(stack.as_str().to_owned());
            }
            TunConfigurationUpdate::SetMtu(mtu) => {
                state.config.mtu = ObservedValue::Reported(mtu);
            }
            TunConfigurationUpdate::SetRouting {
                auto_route,
                strict_route,
            } => {
                state.config.auto_route = ObservedValue::Reported(auto_route);
                state.config.strict_route = ObservedValue::Reported(strict_route);
            }
        }
        state.writes += 1;
        state.revision = ConfigurationRevision::new(state.revision.value().saturating_add(1));
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.config.clone(),
        })
    }
}

fn application(scope: CoreScope, tun: FakeTunPort) -> RuntimeConfigurationApplication {
    let mut ports = RuntimeConfigurationPorts::unsupported();
    ports.tun = Arc::new(tun);
    RuntimeConfigurationApplication::new(scope, ports)
}

fn scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine"),
            CoreInstanceId::new("fake-core").expect("valid instance"),
            Some(CoreVersion::new("test").expect("valid version")),
        ),
        generation: CoreGeneration::new(8),
        session_token: Some(SessionToken::new(82)),
    }
}

fn other_scope() -> CoreScope {
    let mut other = scope();
    other.generation = CoreGeneration::new(9);
    other
}

#[tokio::test]
async fn unsupported_and_stale_tun_writes_have_no_side_effect() {
    let scope = scope();
    let unsupported = FakeTunPort::new(scope.clone(), FakeBehavior::Unsupported);
    let failure = application(scope.clone(), unsupported.clone())
        .set_tun_enabled(true)
        .await
        .expect_err("unsupported TUN must fail closed");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(unsupported.writes(), 0);

    let stale = FakeTunPort::new(scope.clone(), FakeBehavior::StaleObservation);
    let failure = application(scope, stale.clone())
        .set_tun_enabled(true)
        .await
        .expect_err("stale revision must be rejected");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(stale.writes(), 0);
}

#[tokio::test]
async fn scope_mismatch_and_unsupported_stack_are_rejected_before_write() {
    let scope = scope();
    let wrong_scope = FakeTunPort::new(scope.clone(), FakeBehavior::DifferentScope);
    let failure = application(scope.clone(), wrong_scope.clone())
        .set_tun_enabled(true)
        .await
        .expect_err("observation from another activation must be rejected");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(wrong_scope.writes(), 0);

    let tun = FakeTunPort::new(scope.clone(), FakeBehavior::Normal);
    let failure = application(scope, tun.clone())
        .set_tun_stack(TunStack::Lwip)
        .await
        .expect_err("reference-only stack must be rejected before I/O");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(tun.writes(), 0);
}

#[tokio::test]
async fn missing_routing_fields_and_invalid_mtu_are_rejected_before_write() {
    let scope = scope();
    let tun = FakeTunPort::new(scope.clone(), FakeBehavior::Normal);
    tun.set_config(TunConfiguration::default());
    let failure = application(scope.clone(), tun.clone())
        .set_tun_auto_route(false)
        .await
        .expect_err("missing strict-route state must not be guessed");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(tun.writes(), 0);

    let tun = FakeTunPort::new(scope.clone(), FakeBehavior::Normal);
    let failure = application(scope, tun.clone())
        .set_tun_mtu(1000)
        .await
        .expect_err("out-of-range MTU must be rejected before I/O");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(tun.writes(), 0);
}

#[tokio::test]
async fn apply_requires_exact_revision_and_value_readback() {
    let scope = scope();
    let tun = FakeTunPort::new(scope.clone(), FakeBehavior::Normal);
    let observed = application(scope.clone(), tun.clone())
        .set_tun_enabled(true)
        .await
        .expect("typed readback should match");
    assert_eq!(observed.scope, scope);
    assert_eq!(observed.revision, ConfigurationRevision::new(5));
    assert_eq!(observed.value.enabled, ObservedValue::Reported(true));
    assert_eq!(tun.writes(), 1);

    let changed = FakeTunPort::new(scope.clone(), FakeBehavior::ChangedReadbackRevision);
    let failure = application(scope, changed.clone())
        .set_tun_enabled(true)
        .await
        .expect_err("different revision readback must not report success");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(changed.writes(), 1);
}

struct FakeMutableState {
    scope: CoreScope,
    revision: ConfigurationRevision,
    sharing: LanSharingConfiguration,
    security: LanSecurityConfiguration,
    ipv6: Ipv6Configuration,
    sniffer: SnifferConfiguration,
    writes: usize,
}

#[derive(Clone)]
struct FakeMutableConfiguration(Arc<Mutex<FakeMutableState>>);

impl FakeMutableConfiguration {
    fn new(scope: CoreScope) -> Self {
        Self(Arc::new(Mutex::new(FakeMutableState {
            scope,
            revision: ConfigurationRevision::new(4),
            sharing: LanSharingConfiguration {
                enabled: ObservedValue::Reported(false),
                mixed_port: ObservedValue::Reported(7890),
                bind_address: ObservedValue::Reported("*".to_owned()),
            },
            security: LanSecurityConfiguration {
                allowed_ips: ObservedValue::Reported(vec!["192.168.0.0/16".to_owned()]),
                disallowed_ips: ObservedValue::Reported(Vec::new()),
                skip_auth_prefixes: ObservedValue::Reported(vec!["127.0.0.0/8".to_owned()]),
                authentication: LanAuthenticationState::Disabled,
            },
            ipv6: Ipv6Configuration {
                enabled: ObservedValue::Reported(false),
                tun_enabled: ObservedValue::Reported(true),
            },
            sniffer: SnifferConfiguration {
                enabled: ObservedValue::Reported(false),
            },
            writes: 0,
        })))
    }

    fn writes(&self) -> usize {
        self.0.lock().expect("mutable fake lock").writes
    }

    fn omit_ipv6_tun_state(&self) {
        self.0.lock().expect("mutable fake lock").ipv6.tun_enabled = ObservedValue::Unreported;
    }

    fn validate_request(
        state: &FakeMutableState,
        scope: &CoreScope,
        revision: ConfigurationRevision,
    ) -> Result<(), PortError> {
        if scope != &state.scope || revision != state.revision {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "stale or mismatched runtime configuration request",
                false,
            )));
        }
        Ok(())
    }

    fn committed(state: &mut FakeMutableState) {
        state.writes += 1;
        state.revision = ConfigurationRevision::new(state.revision.value() + 1);
    }
}

#[async_trait]
impl LanConfigurationPort for FakeMutableConfiguration {
    async fn observe_sharing(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        let state = self.0.lock().expect("mutable fake lock");
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.sharing.clone(),
        })
    }

    async fn apply_sharing(
        &self,
        request: ConfigurationApply<LanSharingUpdate>,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        let mut state = self.0.lock().expect("mutable fake lock");
        Self::validate_request(&state, &request.scope, request.expected_revision)?;
        state.sharing = LanSharingConfiguration {
            enabled: ObservedValue::Reported(request.update.enabled),
            mixed_port: ObservedValue::Reported(request.update.mixed_port),
            bind_address: ObservedValue::Reported(request.update.bind_address),
        };
        Self::committed(&mut state);
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.sharing.clone(),
        })
    }

    async fn observe_security(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let state = self.0.lock().expect("mutable fake lock");
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.security.clone(),
        })
    }

    async fn apply_security(
        &self,
        request: ConfigurationApply<LanSecurityUpdate>,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let mut state = self.0.lock().expect("mutable fake lock");
        Self::validate_request(&state, &request.scope, request.expected_revision)?;
        let authentication = match request.update.authentication {
            LanAuthenticationUpdate::Disabled => LanAuthenticationState::Disabled,
            LanAuthenticationUpdate::Enabled(credentials) => LanAuthenticationState::Enabled {
                user_count: 1,
                username: ObservedValue::Reported(credentials.username.trim().to_owned()),
            },
        };
        state.security = LanSecurityConfiguration {
            allowed_ips: ObservedValue::Reported(request.update.allowed_ips),
            disallowed_ips: ObservedValue::Reported(request.update.disallowed_ips),
            skip_auth_prefixes: ObservedValue::Reported(request.update.skip_auth_prefixes),
            authentication,
        };
        Self::committed(&mut state);
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.security.clone(),
        })
    }
}

#[async_trait]
impl Ipv6ConfigurationPort for FakeMutableConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        let state = self.0.lock().expect("mutable fake lock");
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.ipv6.clone(),
        })
    }

    async fn apply(
        &self,
        request: ConfigurationApply<Ipv6ConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        let mut state = self.0.lock().expect("mutable fake lock");
        Self::validate_request(&state, &request.scope, request.expected_revision)?;
        state.ipv6.enabled = ObservedValue::Reported(request.update.enabled);
        Self::committed(&mut state);
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.ipv6.clone(),
        })
    }
}

#[async_trait]
impl SnifferConfigurationPort for FakeMutableConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        let state = self.0.lock().expect("mutable fake lock");
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.sniffer.clone(),
        })
    }

    async fn apply(
        &self,
        request: ConfigurationApply<SnifferConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        let mut state = self.0.lock().expect("mutable fake lock");
        Self::validate_request(&state, &request.scope, request.expected_revision)?;
        state.sniffer.enabled = ObservedValue::Reported(request.update.enabled);
        Self::committed(&mut state);
        Ok(ConfigurationObservation {
            scope: state.scope.clone(),
            revision: state.revision,
            value: state.sniffer.clone(),
        })
    }
}

#[tokio::test]
async fn lan_ipv6_and_sniffer_use_revision_checked_typed_writes_and_redacted_readback() {
    let scope = scope();
    let fake = FakeMutableConfiguration::new(scope.clone());
    let mut ports = RuntimeConfigurationPorts::unsupported();
    ports.lan = Arc::new(fake.clone());
    ports.ipv6 = Arc::new(fake.clone());
    ports.sniffer = Arc::new(fake.clone());
    let app = RuntimeConfigurationApplication::new(scope, ports);

    let sharing = app
        .set_lan_sharing(true, 7890, "*")
        .await
        .expect("LAN sharing should be applied and read back");
    assert!(sharing.enabled);
    assert_eq!(sharing.mixed_port, 7890);
    assert_eq!(sharing.revision, 5);

    let security = app
        .set_lan_security(
            &["10.0.0.0/8".to_owned()],
            &[],
            &["127.0.0.0/8".to_owned()],
            LanAuthenticationUpdate::Enabled(LanCredentials {
                username: "alice".to_owned(),
                password: "secret".to_owned(),
            }),
        )
        .await
        .expect("LAN ACL and credentials should be applied and read back");
    assert!(security.authentication_enabled);
    assert_eq!(security.authentication_username.as_deref(), Some("alice"));
    assert_eq!(security.authentication_user_count, 1);
    assert_eq!(security.revision, 6);

    let ipv6 = app
        .set_ipv6_routing(true)
        .await
        .expect("IPv6 should be applied and read back");
    assert!(ipv6.enabled);
    assert!(ipv6.tun_enabled);
    assert_eq!(ipv6.revision, 7);

    let sniffer = app
        .set_sniffer_enabled(true)
        .await
        .expect("sniffer should be applied and read back");
    assert_eq!(sniffer.value.enabled, ObservedValue::Reported(true));
    assert_eq!(sniffer.revision, ConfigurationRevision::new(8));
    assert_eq!(fake.writes(), 4);
}

#[tokio::test]
async fn invalid_lan_updates_are_rejected_before_fake_adapter_writes() {
    let scope = scope();
    let fake = FakeMutableConfiguration::new(scope.clone());
    let mut ports = RuntimeConfigurationPorts::unsupported();
    ports.lan = Arc::new(fake.clone());
    let app = RuntimeConfigurationApplication::new(scope, ports);

    let sharing = app
        .set_lan_sharing(true, 0, "*")
        .await
        .expect_err("Allow-LAN cannot be enabled without a proxy port");
    assert_eq!(sharing.code, ErrorCode::InvalidInput);
    assert_eq!(fake.writes(), 0);

    let security = app
        .set_lan_security(&[], &[], &[], LanAuthenticationUpdate::Disabled)
        .await
        .expect_err("empty LAN allowlist must be rejected before adapter access");
    assert_eq!(security.code, ErrorCode::InvalidInput);
    assert_eq!(fake.writes(), 0);
}

#[tokio::test]
async fn ipv6_write_is_rejected_before_apply_when_required_tun_state_is_unreported() {
    let scope = scope();
    let fake = FakeMutableConfiguration::new(scope.clone());
    fake.omit_ipv6_tun_state();
    let mut ports = RuntimeConfigurationPorts::unsupported();
    ports.ipv6 = Arc::new(fake.clone());
    let app = RuntimeConfigurationApplication::new(scope, ports);

    let failure = app
        .set_ipv6_routing(true)
        .await
        .expect_err("IPv6 readback requires observed TUN state");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(fake.writes(), 0);
}
