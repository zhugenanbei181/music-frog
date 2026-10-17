//! Scope-bound live configuration use-cases.

use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use infiltrator_contract::mtu::{MAX_TUN_MTU_BYTES, MIN_TUN_MTU_BYTES};
use infiltrator_contract::tun::TunStack;
use infiltrator_domain::lan_security::{normalize_cidrs, validate_credentials};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::runtime_configuration::{
    ConfigurationApply, ConfigurationObservation, DnsRuntimeConfiguration, Ipv6Configuration,
    Ipv6ConfigurationUpdate, LanAuthenticationState, LanAuthenticationUpdate,
    LanSecurityConfiguration, LanSecurityUpdate, LanSharingConfiguration, LanSharingUpdate,
    ObservedValue, ProxyPortsConfiguration, RuntimeConfigurationPorts, SnifferConfiguration,
    SnifferConfigurationUpdate, TunConfiguration, TunConfigurationUpdate,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use std::fmt::Debug;
use std::net::IpAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfigurationReadout {
    pub tun: ConfigurationObservation<TunConfiguration>,
    pub lan_sharing: ConfigurationObservation<LanSharingConfiguration>,
    pub lan_security: ConfigurationObservation<LanSecurityConfiguration>,
    pub ipv6: ConfigurationObservation<Ipv6Configuration>,
    pub sniffer: ConfigurationObservation<SnifferConfiguration>,
    pub dns: ConfigurationObservation<DnsRuntimeConfiguration>,
    pub proxy_ports: ConfigurationObservation<ProxyPortsConfiguration>,
    pub core_log_level: ConfigurationObservation<ObservedValue<CoreLogLevel>>,
}

#[derive(Clone)]
pub struct RuntimeConfigurationApplication {
    scope: CoreScope,
    ports: RuntimeConfigurationPorts,
}

impl RuntimeConfigurationApplication {
    pub fn new(scope: CoreScope, ports: RuntimeConfigurationPorts) -> Self {
        Self { scope, ports }
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub async fn observe_readout(&self) -> Result<RuntimeConfigurationReadout, Failure> {
        let (tun, lan_sharing, lan_security, ipv6, sniffer, dns, proxy_ports, core_log_level) = futures_util::join!(
            self.observe_tun(),
            self.observe_lan_sharing(),
            self.observe_lan_security(),
            self.observe_ipv6(),
            self.observe_sniffer(),
            self.observe_dns(),
            self.observe_proxy_ports(),
            self.observe_core_log_level(),
        );
        Ok(RuntimeConfigurationReadout {
            tun: tun?,
            lan_sharing: lan_sharing?,
            lan_security: lan_security?,
            ipv6: ipv6?,
            sniffer: sniffer?,
            dns: dns?,
            proxy_ports: proxy_ports?,
            core_log_level: core_log_level?,
        })
    }

    pub async fn observe_tun(&self) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        let observed = self
            .ports
            .tun
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn set_tun_enabled(
        &self,
        enabled: bool,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        self.apply_tun_update(TunConfigurationUpdate::SetEnabled(enabled))
            .await
    }

    pub async fn set_tun_stack(
        &self,
        stack: TunStack,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        if !stack.is_live_supported() {
            return Err(Failure::unsupported(format!(
                "TUN stack {} is reference-only and not accepted by the live core",
                stack.as_str()
            )));
        }
        self.apply_tun_update(TunConfigurationUpdate::SetStack(stack))
            .await
    }

    pub async fn set_tun_mtu(
        &self,
        mtu: u32,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        if !(MIN_TUN_MTU_BYTES..=MAX_TUN_MTU_BYTES).contains(&mtu) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!(
                    "TUN MTU {mtu} is outside the supported range {}..={}",
                    MIN_TUN_MTU_BYTES, MAX_TUN_MTU_BYTES
                ),
                false,
            ));
        }
        self.apply_tun_update(TunConfigurationUpdate::SetMtu(mtu))
            .await
    }

    pub async fn set_tun_auto_route(
        &self,
        enabled: bool,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        let current = self.observe_tun().await?;
        let strict_route = reported(&current.value.strict_route, "TUN strict-route")?;
        self.apply_tun_update_from(
            current,
            TunConfigurationUpdate::SetRouting {
                auto_route: enabled,
                strict_route: enabled && strict_route,
            },
        )
        .await
    }

    pub async fn set_tun_strict_route(
        &self,
        enabled: bool,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        let current = self.observe_tun().await?;
        let auto_route = reported(&current.value.auto_route, "TUN auto-route")?;
        self.apply_tun_update_from(
            current,
            TunConfigurationUpdate::SetRouting {
                auto_route: auto_route || enabled,
                strict_route: enabled,
            },
        )
        .await
    }

    pub async fn observe_lan_sharing(
        &self,
    ) -> Result<ConfigurationObservation<LanSharingConfiguration>, Failure> {
        let observed = self
            .ports
            .lan
            .observe_sharing(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn observe_lan_security(
        &self,
    ) -> Result<ConfigurationObservation<LanSecurityConfiguration>, Failure> {
        let observed = self
            .ports
            .lan
            .observe_security(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn set_lan_sharing(
        &self,
        enabled: bool,
        mixed_port: u16,
        bind_address: &str,
    ) -> Result<LanSharingSnapshot, Failure> {
        if enabled && mixed_port == 0 {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Allow-LAN requires a non-zero mixed proxy port",
                false,
            ));
        }
        let update = LanSharingUpdate {
            enabled,
            mixed_port,
            bind_address: canonical_bind_address(bind_address)?,
        };
        let current = self.observe_lan_sharing().await?;
        let expected = LanSharingConfiguration {
            enabled: ObservedValue::Reported(update.enabled),
            mixed_port: ObservedValue::Reported(update.mixed_port),
            bind_address: ObservedValue::Reported(update.bind_address.clone()),
        };
        let applied = self
            .ports
            .lan
            .apply_sharing(ConfigurationApply {
                scope: self.scope.clone(),
                expected_revision: current.revision,
                update,
            })
            .await
            .map_err(Failure::from)?;
        let readback = self
            .ports
            .lan
            .observe_sharing(&self.scope)
            .await
            .map_err(Failure::from)?;
        let observed =
            verify_apply_readback(&self.scope, &expected, applied, readback, "LAN sharing")?;
        Ok(LanSharingSnapshot::new(
            observed.revision.value(),
            reported(&observed.value.enabled, "allow-lan")?,
            reported(&observed.value.mixed_port, "mixed-port")?,
            reported(&observed.value.bind_address, "bind-address")?,
        ))
    }

    pub async fn set_lan_security(
        &self,
        allowed_ips: &[String],
        disallowed_ips: &[String],
        skip_auth_prefixes: &[String],
        authentication: LanAuthenticationUpdate,
    ) -> Result<LanSecuritySnapshot, Failure> {
        let allowed_ips = normalize_requested_cidrs(allowed_ips, "lan-allowed-ips")?;
        if allowed_ips.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "lan-allowed-ips must contain at least one CIDR",
                false,
            ));
        }
        let disallowed_ips = normalize_requested_cidrs(disallowed_ips, "lan-disallowed-ips")?;
        let skip_auth_prefixes =
            normalize_requested_cidrs(skip_auth_prefixes, "skip-auth-prefixes")?;
        if let LanAuthenticationUpdate::Enabled(credentials) = &authentication {
            validate_credentials(&credentials.username, &credentials.password)
                .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        }
        let update = LanSecurityUpdate {
            allowed_ips: allowed_ips.clone(),
            disallowed_ips: disallowed_ips.clone(),
            skip_auth_prefixes: skip_auth_prefixes.clone(),
            authentication,
        };
        let current = self.observe_lan_security().await?;
        let expected = expected_lan_security(&update);
        let applied = self
            .ports
            .lan
            .apply_security(ConfigurationApply {
                scope: self.scope.clone(),
                expected_revision: current.revision,
                update,
            })
            .await
            .map_err(Failure::from)?;
        let readback = self
            .ports
            .lan
            .observe_security(&self.scope)
            .await
            .map_err(Failure::from)?;
        let observed =
            verify_apply_readback(&self.scope, &expected, applied, readback, "LAN security")?;
        let auth = match observed.value.authentication {
            LanAuthenticationState::Disabled => (false, 0, None),
            LanAuthenticationState::Enabled {
                user_count,
                username,
            } => (true, user_count, reported_optional(username)),
            LanAuthenticationState::Unreported => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "LAN authentication state was not reported by the active core",
                    true,
                ));
            }
        };
        Ok(LanSecuritySnapshot::new(
            observed.revision.value(),
            reported(&observed.value.allowed_ips, "lan-allowed-ips")?,
            reported(&observed.value.disallowed_ips, "lan-disallowed-ips")?,
            reported(&observed.value.skip_auth_prefixes, "skip-auth-prefixes")?,
            auth.0,
            auth.1,
            auth.2,
        ))
    }

    pub async fn observe_ipv6(
        &self,
    ) -> Result<ConfigurationObservation<Ipv6Configuration>, Failure> {
        let observed = self
            .ports
            .ipv6
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn set_ipv6_routing(&self, enabled: bool) -> Result<Ipv6RoutingSnapshot, Failure> {
        let current = self.observe_ipv6().await?;
        let tun_enabled = reported(&current.value.tun_enabled, "TUN enable")?;
        let expected = Ipv6Configuration {
            enabled: ObservedValue::Reported(enabled),
            tun_enabled: ObservedValue::Reported(tun_enabled),
        };
        let applied = self
            .ports
            .ipv6
            .apply(ConfigurationApply {
                scope: self.scope.clone(),
                expected_revision: current.revision,
                update: Ipv6ConfigurationUpdate { enabled },
            })
            .await
            .map_err(Failure::from)?;
        let readback = self
            .ports
            .ipv6
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        let observed =
            verify_apply_readback(&self.scope, &expected, applied, readback, "IPv6 routing")?;
        Ok(Ipv6RoutingSnapshot::new(
            observed.revision.value(),
            reported(&observed.value.enabled, "IPv6")?,
            reported(&observed.value.tun_enabled, "TUN enable")?,
        ))
    }

    pub async fn observe_sniffer(
        &self,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, Failure> {
        let observed = self
            .ports
            .sniffer
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn set_sniffer_enabled(
        &self,
        enabled: bool,
    ) -> Result<ConfigurationObservation<SnifferConfiguration>, Failure> {
        let current = self.observe_sniffer().await?;
        let expected = SnifferConfiguration {
            enabled: ObservedValue::Reported(enabled),
        };
        let applied = self
            .ports
            .sniffer
            .apply(ConfigurationApply {
                scope: self.scope.clone(),
                expected_revision: current.revision,
                update: SnifferConfigurationUpdate { enabled },
            })
            .await
            .map_err(Failure::from)?;
        let readback = self
            .ports
            .sniffer
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_apply_readback(&self.scope, &expected, applied, readback, "sniffer")
    }

    pub async fn observe_proxy_ports(
        &self,
    ) -> Result<ConfigurationObservation<ProxyPortsConfiguration>, Failure> {
        let observed = self
            .ports
            .proxy_ports
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn observe_dns(
        &self,
    ) -> Result<ConfigurationObservation<DnsRuntimeConfiguration>, Failure> {
        let observed = self
            .ports
            .dns
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn observe_core_log_level(
        &self,
    ) -> Result<ConfigurationObservation<ObservedValue<CoreLogLevel>>, Failure> {
        let observed = self
            .ports
            .core_log_level
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    pub async fn observe_pac_rules(
        &self,
    ) -> Result<ConfigurationObservation<Vec<RuleEntry>>, Failure> {
        let observed = self
            .ports
            .pac_rules
            .observe_rules(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &observed.scope)?;
        Ok(observed)
    }

    async fn apply_tun_update(
        &self,
        update: TunConfigurationUpdate,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        let current = self.observe_tun().await?;
        self.apply_tun_update_from(current, update).await
    }

    async fn apply_tun_update_from(
        &self,
        current: ConfigurationObservation<TunConfiguration>,
        update: TunConfigurationUpdate,
    ) -> Result<ConfigurationObservation<TunConfiguration>, Failure> {
        let expected_value = apply_tun_update(&current.value, update);
        let applied = self
            .ports
            .tun
            .apply(ConfigurationApply {
                scope: self.scope.clone(),
                expected_revision: current.revision,
                update,
            })
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &applied.scope)?;
        if applied.value != expected_value {
            return Err(readback_mismatch(
                "TUN configuration",
                &expected_value,
                &applied.value,
            ));
        }
        let readback = self
            .ports
            .tun
            .observe(&self.scope)
            .await
            .map_err(Failure::from)?;
        verify_scope(&self.scope, &readback.scope)?;
        if readback.revision != applied.revision || readback.value != applied.value {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "TUN readback revision or value changed after apply",
                true,
            ));
        }
        Ok(readback)
    }
}

fn apply_tun_update(before: &TunConfiguration, update: TunConfigurationUpdate) -> TunConfiguration {
    let mut after = before.clone();
    match update {
        TunConfigurationUpdate::SetEnabled(enabled) => {
            after.enabled = ObservedValue::Reported(enabled);
        }
        TunConfigurationUpdate::SetStack(stack) => {
            after.stack = ObservedValue::Reported(stack.as_str().to_owned());
        }
        TunConfigurationUpdate::SetMtu(mtu) => {
            after.mtu = ObservedValue::Reported(mtu);
        }
        TunConfigurationUpdate::SetRouting {
            auto_route,
            strict_route,
        } => {
            after.auto_route = ObservedValue::Reported(auto_route);
            after.strict_route = ObservedValue::Reported(strict_route);
        }
    }
    after
}

fn expected_lan_security(update: &LanSecurityUpdate) -> LanSecurityConfiguration {
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

fn verify_apply_readback<T: Clone + Debug + PartialEq>(
    scope: &CoreScope,
    expected: &T,
    applied: ConfigurationObservation<T>,
    readback: ConfigurationObservation<T>,
    name: &str,
) -> Result<ConfigurationObservation<T>, Failure> {
    verify_scope(scope, &applied.scope)?;
    verify_scope(scope, &readback.scope)?;
    if &applied.value != expected {
        return Err(readback_mismatch(name, expected, &applied.value));
    }
    if readback.revision != applied.revision || readback.value != applied.value {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            format!("{name} readback revision or value changed after apply"),
            true,
        ));
    }
    Ok(readback)
}

fn verify_scope(expected: &CoreScope, observed: &CoreScope) -> Result<(), Failure> {
    if expected == observed {
        Ok(())
    } else {
        Err(Failure::new(
            ErrorCode::InvalidState,
            "configuration observation belongs to a stale or different Core scope",
            false,
        ))
    }
}

fn reported<T: Clone>(value: &ObservedValue<T>, field: &str) -> Result<T, Failure> {
    match value {
        ObservedValue::Reported(value) => Ok(value.clone()),
        ObservedValue::Unreported => Err(Failure::new(
            ErrorCode::NotReady,
            format!("{field} was not reported by the active core"),
            true,
        )),
    }
}

fn reported_optional<T>(value: ObservedValue<T>) -> Option<T> {
    match value {
        ObservedValue::Unreported => None,
        ObservedValue::Reported(value) => Some(value),
    }
}

fn normalize_requested_cidrs(values: &[String], field: &str) -> Result<Vec<String>, Failure> {
    normalize_cidrs(values, field)
        .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))
}

fn canonical_bind_address(raw: &str) -> Result<String, Failure> {
    let value = raw.trim();
    if value == "*" {
        return Ok(value.to_owned());
    }
    let unbracketed = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    let address = unbracketed.parse::<IpAddr>().map_err(|_| {
        Failure::new(
            ErrorCode::InvalidInput,
            format!("invalid bind address: {value}"),
            false,
        )
    })?;
    Ok(match address {
        IpAddr::V4(address) => address.to_string(),
        IpAddr::V6(address) => format!("[{address}]"),
    })
}

fn readback_mismatch<T: Debug>(name: &str, expected: &T, observed: &T) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("{name} readback mismatch: expected {expected:?}, observed {observed:?}"),
        true,
    )
}

#[cfg(test)]
#[path = "runtime_configuration_application_tests.rs"]
mod tests;
