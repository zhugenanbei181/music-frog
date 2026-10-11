//! Narrow, scope-bound observations and writes for live core configuration.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::lan::LanCredentials;
use infiltrator_contract::tun::TunStack;
use infiltrator_domain::rules::RuleEntry;
use std::sync::Arc;

/// Adapter-authored revision for the exact configuration feature observed.
/// This is a compare token, not a timestamp or a cross-engine config version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConfigurationRevision(u64);

impl ConfigurationRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationObservation<T> {
    pub scope: CoreScope,
    pub revision: ConfigurationRevision,
    pub value: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationApply<T> {
    pub scope: CoreScope,
    pub expected_revision: ConfigurationRevision,
    pub update: T,
}

/// A field that the engine may omit from an otherwise valid feature response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedValue<T> {
    Unreported,
    Reported(T),
}

impl<T> Default for ObservedValue<T> {
    fn default() -> Self {
        Self::Unreported
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TunConfiguration {
    pub enabled: ObservedValue<bool>,
    pub stack: ObservedValue<String>,
    pub auto_route: ObservedValue<bool>,
    pub strict_route: ObservedValue<bool>,
    pub mtu: ObservedValue<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunConfigurationUpdate {
    SetEnabled(bool),
    SetStack(TunStack),
    SetMtu(u32),
    SetRouting {
        auto_route: bool,
        strict_route: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanSharingConfiguration {
    pub enabled: ObservedValue<bool>,
    pub mixed_port: ObservedValue<u16>,
    pub bind_address: ObservedValue<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanAuthenticationState {
    Unreported,
    Disabled,
    Enabled {
        user_count: usize,
        username: ObservedValue<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanSecurityConfiguration {
    pub allowed_ips: ObservedValue<Vec<String>>,
    pub disallowed_ips: ObservedValue<Vec<String>>,
    pub skip_auth_prefixes: ObservedValue<Vec<String>>,
    pub authentication: LanAuthenticationState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanAuthenticationUpdate {
    Disabled,
    Enabled(LanCredentials),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanSecurityUpdate {
    pub allowed_ips: Vec<String>,
    pub disallowed_ips: Vec<String>,
    pub skip_auth_prefixes: Vec<String>,
    pub authentication: LanAuthenticationUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanSharingUpdate {
    pub enabled: bool,
    pub mixed_port: u16,
    pub bind_address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv6Configuration {
    pub enabled: ObservedValue<bool>,
    pub tun_enabled: ObservedValue<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv6ConfigurationUpdate {
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnifferConfiguration {
    pub enabled: ObservedValue<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnifferConfigurationUpdate {
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyPortsConfiguration {
    pub http_port: ObservedValue<u16>,
    pub mixed_port: ObservedValue<u16>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsRuntimeConfiguration {
    pub nameserver: ObservedValue<Vec<String>>,
    pub fallback: ObservedValue<Vec<String>>,
    pub enhanced_mode: ObservedValue<String>,
}

#[async_trait]
pub trait TunConfigurationPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError>;

    async fn apply(
        &self,
        request: ConfigurationApply<TunConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError>;
}

#[async_trait]
pub trait LanConfigurationPort: Send + Sync {
    async fn observe_sharing(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError>;

    async fn apply_sharing(
        &self,
        request: ConfigurationApply<LanSharingUpdate>,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError>;

    async fn observe_security(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError>;

    async fn apply_security(
        &self,
        request: ConfigurationApply<LanSecurityUpdate>,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError>;
}

#[async_trait]
pub trait Ipv6ConfigurationPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError>;

    async fn apply(
        &self,
        request: ConfigurationApply<Ipv6ConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError>;
}

#[async_trait]
pub trait SnifferConfigurationPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError>;

    async fn apply(
        &self,
        request: ConfigurationApply<SnifferConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError>;
}

#[async_trait]
pub trait ProxyPortsReadPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ProxyPortsConfiguration>, PortError>;
}

#[async_trait]
pub trait RuntimeDnsConfigurationPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<DnsRuntimeConfiguration>, PortError>;
}

#[async_trait]
pub trait CoreLogLevelReadPort: Send + Sync {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ObservedValue<CoreLogLevel>>, PortError>;
}

/// PAC uses a separate rule read; it does not gain the broader runtime config
/// surface merely because both operations are implemented by one controller.
#[async_trait]
pub trait PacRuleReadPort: Send + Sync {
    async fn observe_rules(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Vec<RuleEntry>>, PortError>;
}

/// Typed port set provided by one active runtime adapter.
#[derive(Clone)]
pub struct RuntimeConfigurationPorts {
    pub tun: Arc<dyn TunConfigurationPort>,
    pub lan: Arc<dyn LanConfigurationPort>,
    pub ipv6: Arc<dyn Ipv6ConfigurationPort>,
    pub sniffer: Arc<dyn SnifferConfigurationPort>,
    pub proxy_ports: Arc<dyn ProxyPortsReadPort>,
    pub dns: Arc<dyn RuntimeDnsConfigurationPort>,
    pub core_log_level: Arc<dyn CoreLogLevelReadPort>,
    pub pac_rules: Arc<dyn PacRuleReadPort>,
}

impl RuntimeConfigurationPorts {
    pub fn new(
        tun: Arc<dyn TunConfigurationPort>,
        lan: Arc<dyn LanConfigurationPort>,
        ipv6: Arc<dyn Ipv6ConfigurationPort>,
        sniffer: Arc<dyn SnifferConfigurationPort>,
        proxy_ports: Arc<dyn ProxyPortsReadPort>,
        dns: Arc<dyn RuntimeDnsConfigurationPort>,
        core_log_level: Arc<dyn CoreLogLevelReadPort>,
        pac_rules: Arc<dyn PacRuleReadPort>,
    ) -> Self {
        Self {
            tun,
            lan,
            ipv6,
            sniffer,
            proxy_ports,
            dns,
            core_log_level,
            pac_rules,
        }
    }

    pub fn unsupported() -> Self {
        let port = Arc::new(UnsupportedRuntimeConfiguration);
        Self {
            tun: port.clone(),
            lan: port.clone(),
            ipv6: port.clone(),
            sniffer: port.clone(),
            proxy_ports: port.clone(),
            dns: port.clone(),
            core_log_level: port.clone(),
            pac_rules: port,
        }
    }
}

struct UnsupportedRuntimeConfiguration;

fn unsupported(capability: Capability, operation: &str) -> PortError {
    PortError::unsupported(
        capability,
        format!("{operation} is not composed for this engine"),
    )
}

#[async_trait]
impl TunConfigurationPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        Err(unsupported(Capability::Tun, "TUN configuration read"))
    }

    async fn apply(
        &self,
        _request: ConfigurationApply<TunConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        Err(unsupported(Capability::Tun, "TUN configuration write"))
    }
}

#[async_trait]
impl LanConfigurationPort for UnsupportedRuntimeConfiguration {
    async fn observe_sharing(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        Err(unsupported(
            Capability::LanAccessControl,
            "LAN sharing read",
        ))
    }

    async fn apply_sharing(
        &self,
        _request: ConfigurationApply<LanSharingUpdate>,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        Err(unsupported(
            Capability::LanAccessControl,
            "LAN sharing write",
        ))
    }

    async fn observe_security(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        Err(unsupported(
            Capability::LanAccessControl,
            "LAN security read",
        ))
    }

    async fn apply_security(
        &self,
        _request: ConfigurationApply<LanSecurityUpdate>,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        Err(unsupported(
            Capability::LanAccessControl,
            "LAN security write",
        ))
    }
}

#[async_trait]
impl Ipv6ConfigurationPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        Err(unsupported(
            Capability::Ipv6Routing,
            "IPv6 configuration read",
        ))
    }

    async fn apply(
        &self,
        _request: ConfigurationApply<Ipv6ConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        Err(unsupported(
            Capability::Ipv6Routing,
            "IPv6 configuration write",
        ))
    }
}

#[async_trait]
impl SnifferConfigurationPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        Err(unsupported(
            Capability::Profiles,
            "sniffer configuration read",
        ))
    }

    async fn apply(
        &self,
        _request: ConfigurationApply<SnifferConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        Err(unsupported(
            Capability::Profiles,
            "sniffer configuration write",
        ))
    }
}

#[async_trait]
impl ProxyPortsReadPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ProxyPortsConfiguration>, PortError> {
        Err(unsupported(Capability::SystemProxy, "proxy port read"))
    }
}

#[async_trait]
impl RuntimeDnsConfigurationPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<DnsRuntimeConfiguration>, PortError> {
        Err(unsupported(
            Capability::Dns,
            "runtime DNS configuration read",
        ))
    }
}

#[async_trait]
impl CoreLogLevelReadPort for UnsupportedRuntimeConfiguration {
    async fn observe(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ObservedValue<CoreLogLevel>>, PortError> {
        Err(unsupported(Capability::Logs, "core log-level read"))
    }
}

#[async_trait]
impl PacRuleReadPort for UnsupportedRuntimeConfiguration {
    async fn observe_rules(
        &self,
        _scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Vec<RuleEntry>>, PortError> {
        Err(unsupported(Capability::PacService, "PAC rule read"))
    }
}
