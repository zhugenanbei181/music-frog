//! Scope and revision fenced Mihomo configuration ports.

use crate::client::MihomoClient;
use crate::core_log_level::decode_mihomo_log_level;
use crate::runtime_gateway::network_error;
use crate::types::{ConfigResponse, Rule};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::mtu::{MAX_TUN_MTU_BYTES, MIN_TUN_MTU_BYTES};
use infiltrator_contract::tun::TunStack;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_configuration::{
    ConfigurationApply, ConfigurationObservation, ConfigurationRevision, CoreLogLevelReadPort,
    DnsRuntimeConfiguration, Ipv6Configuration, Ipv6ConfigurationPort, Ipv6ConfigurationUpdate,
    LanAuthenticationState, LanAuthenticationUpdate, LanConfigurationPort,
    LanSecurityConfiguration, LanSecurityUpdate, LanSharingConfiguration, LanSharingUpdate,
    ObservedValue, PacRuleReadPort, ProxyPortsConfiguration, ProxyPortsReadPort,
    RuntimeConfigurationPorts, RuntimeDnsConfigurationPort, SnifferConfiguration,
    SnifferConfigurationPort, SnifferConfigurationUpdate, TunConfiguration, TunConfigurationPort,
    TunConfigurationUpdate,
};
use serde_json::{Value, json};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type RuntimeScopeResolver = Arc<dyn Fn() -> Result<CoreScope, PortError> + Send + Sync>;

#[derive(Clone)]
pub struct MihomoRuntimeConfigurationAdapter {
    client: MihomoClient,
    scope_resolver: Option<RuntimeScopeResolver>,
    write_lock: Arc<Mutex<()>>,
}

impl MihomoRuntimeConfigurationAdapter {
    pub fn new(client: MihomoClient, scope_resolver: Option<RuntimeScopeResolver>) -> Self {
        let write_lock = client.configuration_write_lock();
        Self {
            client,
            scope_resolver,
            write_lock,
        }
    }

    pub fn ports(
        client: MihomoClient,
        scope_resolver: Option<RuntimeScopeResolver>,
    ) -> RuntimeConfigurationPorts {
        let adapter = Arc::new(Self::new(client, scope_resolver));
        let tun: Arc<dyn TunConfigurationPort> = adapter.clone();
        let lan: Arc<dyn LanConfigurationPort> = adapter.clone();
        let ipv6: Arc<dyn Ipv6ConfigurationPort> = adapter.clone();
        let sniffer: Arc<dyn SnifferConfigurationPort> = adapter.clone();
        let proxy_ports: Arc<dyn ProxyPortsReadPort> = adapter.clone();
        let dns: Arc<dyn RuntimeDnsConfigurationPort> = adapter.clone();
        let core_log_level: Arc<dyn CoreLogLevelReadPort> = adapter.clone();
        let pac_rules: Arc<dyn PacRuleReadPort> = adapter;
        RuntimeConfigurationPorts::new(
            tun,
            lan,
            ipv6,
            sniffer,
            proxy_ports,
            dns,
            core_log_level,
            pac_rules,
        )
    }

    fn validate_scope(&self, scope: &CoreScope, capability: Capability) -> Result<(), PortError> {
        if scope.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                capability,
                format!(
                    "Mihomo configuration cannot serve engine {}",
                    scope.identity.engine.as_str()
                ),
            ));
        }
        if let Some(resolve_scope) = &self.scope_resolver {
            let active = resolve_scope()?;
            if &active != scope {
                return Err(stale_scope());
            }
        }
        Ok(())
    }

    async fn read_config(
        &self,
        scope: &CoreScope,
        capability: Capability,
    ) -> Result<ConfigResponse, PortError> {
        self.validate_scope(scope, capability)?;
        let config = self.client.get_config().await.map_err(network_error)?;
        self.validate_scope(scope, capability)?;
        Ok(config)
    }

    async fn apply_config<T, U>(
        &self,
        scope: CoreScope,
        expected_revision: ConfigurationRevision,
        capability: Capability,
        update: U,
        patch: Value,
        read: fn(&ConfigResponse) -> T,
        update_value: fn(&T, &U) -> T,
    ) -> Result<ConfigurationObservation<T>, PortError>
    where
        T: Clone + PartialEq + Debug,
    {
        let _guard = self.write_lock.lock().await;
        let before_response = self.read_config(&scope, capability).await?;
        let before = read(&before_response);
        if revision(&before) != expected_revision {
            return Err(stale_revision());
        }
        let expected = update_value(&before, &update);
        self.validate_scope(&scope, capability)?;
        self.client
            .patch_config(patch)
            .await
            .map_err(network_error)?;
        let after_response = self.read_config(&scope, capability).await?;
        let observed = read(&after_response);
        if observed != expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "Mihomo configuration readback did not match the typed update",
                true,
            )));
        }
        Ok(ConfigurationObservation {
            scope,
            revision: revision(&observed),
            value: observed,
        })
    }
}

fn revision<T: Debug>(value: &T) -> ConfigurationRevision {
    let mut hasher = DefaultHasher::new();
    format!("{value:?}").hash(&mut hasher);
    ConfigurationRevision::new(hasher.finish())
}

fn stale_scope() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        "Mihomo configuration request belongs to a stale Core scope",
        false,
    ))
}

fn stale_revision() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        "Mihomo configuration revision changed before apply",
        false,
    ))
}

fn unreported<T>() -> ObservedValue<T> {
    ObservedValue::Unreported
}

fn observed<T>(value: Option<T>) -> ObservedValue<T> {
    match value {
        Some(value) => ObservedValue::Reported(value),
        None => unreported(),
    }
}

fn tun_configuration(config: &ConfigResponse) -> TunConfiguration {
    match config.tun.as_ref() {
        Some(tun) => TunConfiguration {
            enabled: observed(tun.enable),
            stack: observed(tun.stack.clone().map(canonical_tun_stack)),
            auto_route: observed(tun.auto_route),
            strict_route: observed(tun.strict_route),
            mtu: observed(tun.mtu),
        },
        None => TunConfiguration::default(),
    }
}

fn canonical_tun_stack(raw: String) -> String {
    TunStack::parse(&raw)
        .map(|stack| stack.as_str().to_owned())
        .unwrap_or(raw)
}

fn lan_sharing_configuration(config: &ConfigResponse) -> LanSharingConfiguration {
    LanSharingConfiguration {
        enabled: ObservedValue::Reported(config.allow_lan),
        mixed_port: ObservedValue::Reported(config.mixed_port),
        bind_address: observed(config.bind_address.clone()),
    }
}

fn lan_security_configuration(config: &ConfigResponse) -> LanSecurityConfiguration {
    let authentication = match config.authentication.as_ref() {
        None => LanAuthenticationState::Unreported,
        Some(users) if users.is_empty() => LanAuthenticationState::Disabled,
        Some(users) => LanAuthenticationState::Enabled {
            user_count: users.len(),
            username: users
                .first()
                .and_then(|credential| credential.split_once(':'))
                .map(|(username, _)| ObservedValue::Reported(username.to_owned()))
                .unwrap_or_else(unreported),
        },
    };
    LanSecurityConfiguration {
        allowed_ips: observed(config.lan_allowed_ips.clone()),
        disallowed_ips: observed(config.lan_disallowed_ips.clone()),
        skip_auth_prefixes: observed(config.skip_auth_prefixes.clone()),
        authentication,
    }
}

fn ipv6_configuration(config: &ConfigResponse) -> Ipv6Configuration {
    Ipv6Configuration {
        enabled: observed(config.ipv6),
        tun_enabled: config
            .tun
            .as_ref()
            .and_then(|tun| tun.enable)
            .map(ObservedValue::Reported)
            .unwrap_or_else(unreported),
    }
}

fn sniffer_configuration(config: &ConfigResponse) -> SnifferConfiguration {
    SnifferConfiguration {
        enabled: config
            .sniffer
            .as_ref()
            .map(|sniffer| ObservedValue::Reported(sniffer.enable))
            .unwrap_or_else(unreported),
    }
}

fn proxy_ports_configuration(config: &ConfigResponse) -> ProxyPortsConfiguration {
    ProxyPortsConfiguration {
        http_port: ObservedValue::Reported(config.port),
        mixed_port: ObservedValue::Reported(config.mixed_port),
    }
}

fn dns_runtime_configuration(config: &ConfigResponse) -> DnsRuntimeConfiguration {
    match config.dns.as_ref() {
        Some(dns) => DnsRuntimeConfiguration {
            nameserver: ObservedValue::Reported(dns.nameserver.clone()),
            fallback: observed(dns.fallback.clone()),
            enhanced_mode: ObservedValue::Reported(dns.enhanced_mode.clone()),
        },
        None => DnsRuntimeConfiguration::default(),
    }
}

fn core_log_level_configuration(config: &ConfigResponse) -> ObservedValue<CoreLogLevel> {
    match decode_mihomo_log_level(&config.log_level) {
        Some(level) => ObservedValue::Reported(level),
        None => ObservedValue::Unreported,
    }
}

fn pac_rules(rules: Vec<Rule>) -> Vec<RuleEntry> {
    rules
        .into_iter()
        .map(|rule| RuleEntry {
            rule: format!("{},{},{}", rule.rule_type, rule.payload, rule.proxy),
            enabled: true,
        })
        .collect()
}

fn update_tun(current: &TunConfiguration, update: &TunConfigurationUpdate) -> TunConfiguration {
    let mut result = current.clone();
    match update {
        TunConfigurationUpdate::SetEnabled(value) => {
            result.enabled = ObservedValue::Reported(*value);
        }
        TunConfigurationUpdate::SetStack(stack) => {
            result.stack = ObservedValue::Reported(stack.as_str().to_owned());
        }
        TunConfigurationUpdate::SetMtu(value) => {
            result.mtu = ObservedValue::Reported(*value);
        }
        TunConfigurationUpdate::SetRouting {
            auto_route,
            strict_route,
        } => {
            result.auto_route = ObservedValue::Reported(*auto_route);
            result.strict_route = ObservedValue::Reported(*strict_route);
        }
    }
    result
}

fn update_lan_sharing(
    _current: &LanSharingConfiguration,
    update: &LanSharingUpdate,
) -> LanSharingConfiguration {
    LanSharingConfiguration {
        enabled: ObservedValue::Reported(update.enabled),
        mixed_port: ObservedValue::Reported(update.mixed_port),
        bind_address: ObservedValue::Reported(update.bind_address.clone()),
    }
}

fn update_lan_security(
    _current: &LanSecurityConfiguration,
    update: &LanSecurityUpdate,
) -> LanSecurityConfiguration {
    let authentication = match &update.authentication {
        LanAuthenticationUpdate::Disabled => LanAuthenticationState::Disabled,
        LanAuthenticationUpdate::Enabled(credentials) => LanAuthenticationState::Enabled {
            user_count: 1,
            username: ObservedValue::Reported(credentials.username.trim().to_owned()),
        },
    };
    LanSecurityConfiguration {
        allowed_ips: ObservedValue::Reported(update.allowed_ips.clone()),
        disallowed_ips: ObservedValue::Reported(update.disallowed_ips.clone()),
        skip_auth_prefixes: ObservedValue::Reported(update.skip_auth_prefixes.clone()),
        authentication,
    }
}

fn update_ipv6(current: &Ipv6Configuration, update: &Ipv6ConfigurationUpdate) -> Ipv6Configuration {
    Ipv6Configuration {
        enabled: ObservedValue::Reported(update.enabled),
        tun_enabled: current.tun_enabled.clone(),
    }
}

fn update_sniffer(
    _current: &SnifferConfiguration,
    update: &SnifferConfigurationUpdate,
) -> SnifferConfiguration {
    SnifferConfiguration {
        enabled: ObservedValue::Reported(update.enabled),
    }
}

fn config_patch_for_tun(update: TunConfigurationUpdate) -> Result<Value, PortError> {
    let body = match update {
        TunConfigurationUpdate::SetEnabled(enable) => json!({ "tun": { "enable": enable } }),
        TunConfigurationUpdate::SetStack(stack) => {
            if !stack.is_live_supported() {
                return Err(PortError::unsupported(
                    Capability::Tun,
                    format!("TUN stack {} is reference-only", stack.as_str()),
                ));
            }
            json!({ "tun": { "stack": stack.as_str() } })
        }
        TunConfigurationUpdate::SetMtu(mtu) => {
            if !(MIN_TUN_MTU_BYTES..=MAX_TUN_MTU_BYTES).contains(&mtu) {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("TUN MTU {mtu} is outside the supported range"),
                    false,
                )));
            }
            json!({ "tun": { "mtu": mtu } })
        }
        TunConfigurationUpdate::SetRouting {
            auto_route,
            strict_route,
        } => json!({
            "tun": {
                "auto-route": auto_route,
                "strict-route": strict_route,
            }
        }),
    };
    Ok(body)
}

#[async_trait::async_trait]
impl TunConfigurationPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        let config = self.read_config(scope, Capability::Tun).await?;
        let value = tun_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }

    async fn apply(
        &self,
        request: ConfigurationApply<TunConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<TunConfiguration>, PortError> {
        let patch = config_patch_for_tun(request.update)?;
        self.apply_config(
            request.scope,
            request.expected_revision,
            Capability::Tun,
            request.update,
            patch,
            tun_configuration,
            update_tun,
        )
        .await
    }
}

#[async_trait::async_trait]
impl LanConfigurationPort for MihomoRuntimeConfigurationAdapter {
    async fn observe_sharing(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        let config = self
            .read_config(scope, Capability::LanAccessControl)
            .await?;
        let value = lan_sharing_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }

    async fn apply_sharing(
        &self,
        request: ConfigurationApply<LanSharingUpdate>,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, PortError> {
        if request.update.enabled && request.update.mixed_port == 0 {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                "Allow-LAN requires a non-zero mixed proxy port",
                false,
            )));
        }
        let patch = json!({
            "allow-lan": request.update.enabled,
            "mixed-port": request.update.mixed_port,
            "bind-address": request.update.bind_address,
        });
        self.apply_config(
            request.scope,
            request.expected_revision,
            Capability::LanAccessControl,
            request.update,
            patch,
            lan_sharing_configuration,
            update_lan_sharing,
        )
        .await
    }

    async fn observe_security(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let config = self
            .read_config(scope, Capability::LanAccessControl)
            .await?;
        let value = lan_security_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }

    async fn apply_security(
        &self,
        request: ConfigurationApply<LanSecurityUpdate>,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, PortError> {
        let credentials = match &request.update.authentication {
            LanAuthenticationUpdate::Disabled => Vec::new(),
            LanAuthenticationUpdate::Enabled(credentials) => {
                vec![format!(
                    "{}:{}",
                    credentials.username.trim(),
                    credentials.password
                )]
            }
        };
        let patch = json!({
            "lan-allowed-ips": request.update.allowed_ips,
            "lan-disallowed-ips": request.update.disallowed_ips,
            "skip-auth-prefixes": request.update.skip_auth_prefixes,
            "authentication": credentials,
        });
        self.apply_config(
            request.scope,
            request.expected_revision,
            Capability::LanAccessControl,
            request.update,
            patch,
            lan_security_configuration,
            update_lan_security,
        )
        .await
    }
}

#[async_trait::async_trait]
impl Ipv6ConfigurationPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        let config = self.read_config(scope, Capability::Ipv6Routing).await?;
        let value = ipv6_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }

    async fn apply(
        &self,
        request: ConfigurationApply<Ipv6ConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, PortError> {
        let patch = json!({ "ipv6": request.update.enabled });
        self.apply_config(
            request.scope,
            request.expected_revision,
            Capability::Ipv6Routing,
            request.update,
            patch,
            ipv6_configuration,
            update_ipv6,
        )
        .await
    }
}

#[async_trait::async_trait]
impl SnifferConfigurationPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        let config = self.read_config(scope, Capability::Profiles).await?;
        let value = sniffer_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }

    async fn apply(
        &self,
        request: ConfigurationApply<SnifferConfigurationUpdate>,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, PortError> {
        let patch = json!({ "sniffer": { "enable": request.update.enabled } });
        self.apply_config(
            request.scope,
            request.expected_revision,
            Capability::Profiles,
            request.update,
            patch,
            sniffer_configuration,
            update_sniffer,
        )
        .await
    }
}

#[async_trait::async_trait]
impl ProxyPortsReadPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ProxyPortsConfiguration>, PortError> {
        let config = self.read_config(scope, Capability::SystemProxy).await?;
        let value = proxy_ports_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }
}

#[async_trait::async_trait]
impl RuntimeDnsConfigurationPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<DnsRuntimeConfiguration>, PortError> {
        let config = self.read_config(scope, Capability::Dns).await?;
        let value = dns_runtime_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }
}

#[async_trait::async_trait]
impl CoreLogLevelReadPort for MihomoRuntimeConfigurationAdapter {
    async fn observe(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<ObservedValue<CoreLogLevel>>, PortError> {
        let config = self.read_config(scope, Capability::Logs).await?;
        let value = core_log_level_configuration(&config);
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&value),
            value,
        })
    }
}

#[async_trait::async_trait]
impl PacRuleReadPort for MihomoRuntimeConfigurationAdapter {
    async fn observe_rules(
        &self,
        scope: &CoreScope,
    ) -> Result<ConfigurationObservation<Vec<RuleEntry>>, PortError> {
        self.validate_scope(scope, Capability::PacService)?;
        let rules = pac_rules(self.client.get_rules().await.map_err(network_error)?);
        self.validate_scope(scope, Capability::PacService)?;
        Ok(ConfigurationObservation {
            scope: scope.clone(),
            revision: revision(&rules),
            value: rules,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DnsConfig, SnifferConfig, TunConfig};

    #[test]
    fn mihomo_config_mapping_preserves_absent_vs_explicit_false_empty_and_zero() {
        let missing = ConfigResponse::default();
        assert_eq!(
            tun_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            ipv6_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            sniffer_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            dns_runtime_configuration(&missing).nameserver,
            ObservedValue::Unreported
        );
        assert_eq!(
            lan_security_configuration(&missing).authentication,
            LanAuthenticationState::Unreported
        );
        assert_eq!(
            lan_sharing_configuration(&missing).bind_address,
            ObservedValue::Unreported
        );
        assert_eq!(
            core_log_level_configuration(&missing),
            ObservedValue::Unreported
        );

        let explicit = ConfigResponse {
            log_level: "info".to_owned(),
            allow_lan: false,
            mixed_port: 0,
            port: 0,
            ipv6: Some(false),
            bind_address: Some("*".to_owned()),
            lan_allowed_ips: Some(Vec::new()),
            lan_disallowed_ips: Some(Vec::new()),
            skip_auth_prefixes: Some(Vec::new()),
            authentication: Some(Vec::new()),
            tun: Some(TunConfig {
                enable: Some(false),
                strict_route: Some(false),
                ..Default::default()
            }),
            sniffer: Some(SnifferConfig {
                enable: false,
                ..Default::default()
            }),
            dns: Some(DnsConfig {
                nameserver: Vec::new(),
                fallback: Some(Vec::new()),
                enhanced_mode: "fake-ip".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            tun_configuration(&explicit).strict_route,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            ipv6_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            sniffer_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            dns_runtime_configuration(&explicit).nameserver,
            ObservedValue::Reported(Vec::new())
        );
        assert_eq!(
            lan_security_configuration(&explicit).allowed_ips,
            ObservedValue::Reported(Vec::new())
        );
        assert_eq!(
            lan_security_configuration(&explicit).authentication,
            LanAuthenticationState::Disabled
        );
        assert_eq!(
            proxy_ports_configuration(&explicit).http_port,
            ObservedValue::Reported(0)
        );
        assert_eq!(
            proxy_ports_configuration(&explicit).mixed_port,
            ObservedValue::Reported(0)
        );
        assert_eq!(
            core_log_level_configuration(&explicit),
            ObservedValue::Reported(CoreLogLevel::Info)
        );
        let warning = ConfigResponse {
            log_level: "warning".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            core_log_level_configuration(&warning),
            ObservedValue::Reported(CoreLogLevel::Warn)
        );
        let unknown = ConfigResponse {
            log_level: "trace".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            core_log_level_configuration(&unknown),
            ObservedValue::Unreported
        );

        let canonical_stack = ConfigResponse {
            tun: Some(TunConfig {
                stack: Some("System".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&canonical_stack).stack,
            ObservedValue::Reported("system".to_owned())
        );
        let unknown_stack = ConfigResponse {
            tun: Some(TunConfig {
                stack: Some("future-stack".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&unknown_stack).stack,
            ObservedValue::Reported("future-stack".to_owned())
        );
    }

    #[test]
    fn pac_rule_read_mapping_is_scoped_to_the_dedicated_read_contract() {
        let mapped = pac_rules(vec![Rule {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "example.org".to_owned(),
            proxy: "DIRECT".to_owned(),
            ..Default::default()
        }]);
        assert_eq!(
            mapped,
            vec![RuleEntry {
                rule: "DOMAIN-SUFFIX,example.org,DIRECT".to_owned(),
                enabled: true,
            }]
        );
    }
}
