//! Shared command routing for inbound surfaces.
//!
//! CoreApplication owns lifecycle state and its private worker. This module
//! owns the rest of the application use-case dispatch and is installed by a
//! host composition when that surface wants the full command vocabulary.

use self::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, CoreLifecycleChildPermit,
    ValidatedCoreCommandScope,
};
use crate::configuration_application::ConfigurationApplication;
use crate::connection_application::ConnectionApplication;
use crate::core_capability_application::CoreCapabilityApplication;
use crate::dns_cache_application::DnsCacheApplication;
use crate::dns_latency_application::DnsLatencyApplication;
use crate::dns_leak_application::{DnsLeakApplication, NO_ECHO_PORT_REASON};
use crate::dns_query_application::DnsQueryApplication;
use crate::dns_workbench_application::dns_servers;
use crate::doctor_application::DoctorApplication;
use crate::geo_database_application::GeoDatabaseApplication;
use crate::log_application::LogApplication;
use crate::log_export_application::LogExportApplication;
use crate::mtu_application::MtuApplication;
use crate::network_roaming_application::NetworkRoamingApplication;
use crate::outbound_application::OutboundApplication;
use crate::outbound_application::preferences::OutboundUiPreferencesApplication;
use crate::pac_application::PacApplication;
use crate::port_conflict_application::PortConflictApplication;
use crate::privileged_network_application::PrivilegedNetworkApplication;
use crate::profile_application::ProfileApplication;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_options_application::ProfileOptionsApplication;
use crate::routing_application::RoutingApplication;
use crate::rule_list_application::RuleListApplication;
use crate::rule_provider_application::{RuleProviderApplication, RuleProviderRuntimeAccess};
use crate::rule_provider_runtime_application::RuleProviderRuntimeApplication;
use crate::rule_tracer_application::RuleTracerApplication;
use crate::runtime_query_application::RuntimeQueryApplication;
use crate::script_application::ScriptApplication;
use crate::script_export_application::ScriptExportApplication;
use crate::service_mode_application::ServiceModeApplication;
use crate::settings_application::SettingsApplication;
use crate::shortcut_application::ShortcutApplication;
use crate::snapshot_application::SnapshotApplication;
use crate::stun_probe_application::{NO_STUN_PORT_REASON, StunProbeApplication};
use crate::subscription_refresh_application::SubscriptionRefreshApplication;
use crate::sync_application::SyncApplication;
use crate::system_proxy_application::SystemProxyApplication;
use crate::uwp_loopback_application::UwpLoopbackApplication;
use crate::version_application::VersionApplication;
use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::capability::{Availability, Capability};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::{CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::mtu::MtuProbeState;
use infiltrator_contract::shortcuts::{ShortcutAction, ShortcutChord};
use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::version::{EngineReleaseChannel, ReleaseChannelId};
use infiltrator_domain::rules::provider_store::parse_rule_provider_declarations;
use infiltrator_domain::rules::{RuleEntry, edit};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use infiltrator_ports::dns_leak::DnsLeakProbePort;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::stun_probe::StunEgressProbePort;
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use infiltrator_ports::subscription_notification::SubscriptionNotificationPort;
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::sync::Arc;

mod composition;
mod dispatch;
pub mod handler;
mod output;
#[cfg(test)]
#[path = "command_routing_intent_tests.rs"]
mod routing_intent_tests;
mod runtime_probe_settings;
mod script_services;
mod snapshots;
mod vpn_start_result;

#[derive(Clone, Default)]
pub struct CommandApplication {
    core_capabilities: CoreCapabilityApplication,
    host_capabilities: Option<Arc<dyn CapabilityProvider>>,
    scripts: Option<ScriptApplication>,
    script_exports: Option<ScriptExportApplication>,
    logs: Option<LogApplication>,
    log_export: Option<LogExportApplication>,
    profile: Option<ProfileApplication>,
    runtime_configuration_ports: Option<RuntimeConfigurationPorts>,
    connections: Option<ConnectionApplication>,
    runtime_queries: Option<RuntimeQueryApplication>,
    profile_reference_applier: Option<Arc<dyn ActiveProfileReferenceApplier>>,
    /// DUAL-07-05/06: executor-neutral delay seam + single-flight refresh.
    application_runtime: Option<Arc<dyn ApplicationRuntime>>,
    subscription_source: Option<Arc<dyn SubscriptionSource>>,
    /// DUAL-07-01: host port for local-file / clipboard import channels.
    import_source: Option<Arc<dyn SubscriptionImportPort>>,
    /// DUAL-07-10: host port for subscription system notifications.
    notifier: Option<Arc<dyn SubscriptionNotificationPort>>,
    /// DUAL-05-13: host port that reads CA certificate bundles.
    certificate_authority: Option<Arc<dyn CertificateAuthorityPort>>,
    doctor: Option<DoctorApplication>,
    rule_list: Option<RuleListApplication>,
    routing: Option<RoutingApplication>,
    routing_intent_port: Option<Arc<dyn RoutingIntentPort>>,
    sync: Option<SyncApplication>,
    settings: Option<SettingsApplication>,
    snapshots: Option<SnapshotApplication>,
    versions: Option<VersionApplication>,
    service_mode: Option<ServiceModeApplication>,
    port_conflicts: Option<PortConflictApplication>,
    mtu: Option<MtuApplication>,
    system_proxy: Option<SystemProxyApplication>,
    uwp_loopback: Option<UwpLoopbackApplication>,
    pac: Option<PacApplication>,
    network_roaming: Option<NetworkRoamingApplication>,
    vpn: Option<VpnServiceApplication>,
    privileged_network: Option<PrivilegedNetworkApplication>,
    outbound: Option<OutboundApplication>,
    outbound_ui_preferences: Option<OutboundUiPreferencesApplication>,
    rule_tracer: Option<RuleTracerApplication>,
    configuration: Option<ConfigurationApplication>,
    dns_cache: Option<DnsCacheApplication>,
    dns_query: Option<DnsQueryApplication>,
    /// DUAL-14-10: the shared latency prober `TestDnsLatency` drives.
    dns_latency: Option<DnsLatencyApplication>,
    /// DUAL-14-08: the shared cross-source leak prober `TestDnsLeak` drives.
    dns_leak: Option<DnsLeakApplication>,
    /// DUAL-14-09 (re-scoped): the shared STUN UDP-egress prober `RunStunProbe`
    /// drives.
    stun_probe: Option<StunProbeApplication>,
    rule_provider: Option<RuleProviderApplication>,
    rule_provider_runtime: Option<RuleProviderRuntimeApplication>,
    geo_database: Option<GeoDatabaseApplication>,
}

impl CommandApplication {
    pub fn core_capability_application(&self) -> CoreCapabilityApplication {
        self.core_capabilities.clone()
    }

    /// Execute one inbound command through the shared dispatch table.
    pub async fn execute(&self, intent: CommandIntent) -> Result<(), Failure> {
        self.execute_output(intent).await.map(|_| ())
    }

    pub(super) fn authorize_host_capability(&self, capability: Capability) -> Result<(), Failure> {
        let provider = self.host_capabilities.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "host-only command has no composed host capability provider",
                true,
            )
        })?;
        let snapshot = provider.capabilities();
        if snapshot.host != provider.host_kind() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "host capability provider returned a snapshot for another host",
                false,
            ));
        }
        match snapshot.availability(capability) {
            Availability::Supported | Availability::Experimental => Ok(()),
            Availability::Unsupported { reason } | Availability::Unavailable { reason } => {
                Err(Failure::unsupported(reason))
            }
        }
    }

    /// DUAL-14-10: measure every configured nameserver through the shared
    /// latency application and publish the real report to both surfaces.
    ///
    /// The servers come from the same validated profile read the workbench
    /// publishes, and a host without a prober keeps the typed refusal instead
    /// of a fabricated number.
    async fn test_dns_latency(&self) -> Result<(), Failure> {
        let Some(application) = self.dns_latency.as_ref() else {
            return Err(Failure::unsupported(
                "this host injected no DNS latency prober",
            ));
        };
        let config = self.configuration()?.load_dns_config().await?;
        let servers = dns_servers(&config);
        application.probe(&servers).await.map(|_| ())
    }

    /// DUAL-14-08: run the shared cross-source leak probe. The application
    /// already owns the configured echo sources, so a host without one answers
    /// a typed unsupported instead of inventing a verdict.
    async fn test_dns_leak(&self) -> Result<(), Failure> {
        let Some(application) = self.dns_leak.as_ref() else {
            return Err(Failure::unsupported(NO_ECHO_PORT_REASON));
        };
        DnsLeakProbePort::probe(application)
            .await
            .map(|_| ())
            .map_err(Failure::from)
    }

    /// DUAL-14-09 (re-scoped): run the shared STUN UDP-egress probe. The
    /// application already owns the configured server and expected egress, so
    /// a host without a prober answers a typed unsupported instead of a
    /// fabricated mapping. This is the host's own UDP egress, not a browser
    /// WebRTC result.
    async fn run_stun_probe(&self) -> Result<(), Failure> {
        let Some(application) = self.stun_probe.as_ref() else {
            return Err(Failure::unsupported(NO_STUN_PORT_REASON));
        };
        StunEgressProbePort::probe(application)
            .await
            .map(|_| ())
            .map_err(Failure::from)
    }

    /// DUAL-11-06: read the active profile's declaration for `provider_name`,
    /// resolve its real rules through the shared unpack service and prepend
    /// them to the persisted rule list.
    async fn unpack_rule_provider(
        &self,
        scope: Option<&CoreScope>,
        provider_name: &str,
    ) -> Result<(), Failure> {
        let name = provider_name.trim();
        if name.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "rule provider name is empty",
                false,
            ));
        }
        let providers = self.configuration()?.load_rule_providers().await?;
        let declaration = parse_rule_provider_declarations(&providers)
            .into_iter()
            .find(|declaration| declaration.name == name)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::Configuration,
                    format!("the active profile declares no rule provider named {name}"),
                    false,
                )
            })?;
        let scope = scope.ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "rule-provider unpack has no validated Core scope",
                true,
            )
        })?;
        let runtime = self
            .rule_provider_runtime
            .as_ref()
            .map(|application| RuleProviderRuntimeAccess::Scoped {
                application: application.clone(),
                scope: scope.clone(),
            })
            .unwrap_or(RuleProviderRuntimeAccess::Unavailable);
        let plan = self
            .rule_provider_service()
            .deconstruct(&declaration, edit::DEFAULT_RULE_TARGET, runtime)
            .await?;
        self.edit_rules(move |rules| edit::prepend_rules(rules, plan.entries) > 0)
            .await
    }

    /// DUAL-11-07: purge the host's cached provider files. The port reports the
    /// real file count and byte total; a host without a cache location fails
    /// with a typed unsupported.
    async fn purge_rule_provider_cache(&self) -> Result<(), Failure> {
        self.rule_provider()?.purge().await.map(|_| ())
    }

    /// Unpacking without a host cache port is still meaningful: profile inline
    /// payloads and the controller payload need no local file.
    fn rule_provider_service(&self) -> RuleProviderApplication {
        self.rule_provider.clone().unwrap_or_default()
    }

    fn rule_provider(&self) -> Result<RuleProviderApplication, Failure> {
        self.rule_provider.clone().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "this host does not expose a rule-provider cache location",
                false,
            )
        })
    }

    fn rule_provider_runtime(&self) -> Result<RuleProviderRuntimeApplication, Failure> {
        self.rule_provider_runtime.clone().ok_or_else(|| {
            Failure::unsupported("rule-provider runtime application is not composed")
        })
    }

    fn geo_database(&self) -> Result<GeoDatabaseApplication, Failure> {
        self.geo_database
            .clone()
            .ok_or_else(|| Failure::unsupported("Geo database application is not composed"))
    }

    /// DUAL-11-09/10/11/12: apply a rule-list mutation to the active profile
    /// and persist it atomically. `mutate` reports whether it changed anything;
    /// an unchanged list is a no-op so a stale click never rewrites the file.
    async fn edit_rules<F>(&self, mutate: F) -> Result<(), Failure>
    where
        F: FnOnce(&mut Vec<RuleEntry>) -> bool,
    {
        let configuration = self.configuration()?;
        let mut rules = configuration.load_rules().await?;
        if !mutate(&mut rules) {
            return Ok(());
        }
        configuration.save_rules(rules).await.map(|_| ())
    }

    async fn update_setting(&self, key: &str, value: &str) -> Result<(), Failure> {
        let key = key.trim();
        let value = value.trim();
        if let Some(action_id) = key.strip_prefix("shortcut.") {
            let action = ShortcutAction::from_id(action_id).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("unknown shortcut action {action_id}"),
                    false,
                )
            })?;
            let chord = ShortcutChord::parse(value).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("invalid shortcut chord {value}"),
                    false,
                )
            })?;
            self.shortcuts()?.capture(action, chord).await?;
            return Ok(());
        }
        // DUAL-15-04: the Mini HUD placement persists through the same
        // validated write path; surfaces never write raw coordinates into the
        // settings file themselves.
        if let Some(field) = key.strip_prefix("mini_hud.") {
            let mut placement = self.settings()?.load().await?.mini_hud;
            match field {
                "pinned" => {
                    placement.pinned = value.parse::<bool>().map_err(|_| {
                        Failure::new(
                            ErrorCode::InvalidInput,
                            format!("invalid mini HUD pin value {value}"),
                            false,
                        )
                    })?;
                }
                "x" => {
                    placement.x = value.parse::<i32>().map_err(|_| {
                        Failure::new(
                            ErrorCode::InvalidInput,
                            format!("invalid mini HUD x coordinate {value}"),
                            false,
                        )
                    })?;
                }
                "y" => {
                    placement.y = value.parse::<i32>().map_err(|_| {
                        Failure::new(
                            ErrorCode::InvalidInput,
                            format!("invalid mini HUD y coordinate {value}"),
                            false,
                        )
                    })?;
                }
                other => {
                    return Err(Failure::new(
                        ErrorCode::InvalidInput,
                        format!("unknown mini HUD setting {other}"),
                        false,
                    ));
                }
            }
            self.settings()?
                .update(move |settings| settings.mini_hud = placement)
                .await?;
            return Ok(());
        }
        if !matches!(
            key,
            "language"
                | "theme"
                | "notifications_enabled"
                | "close_to_tray"
                | "refresh_current_profile_on_start"
                | "refresh_profile_on_start"
                | "custom_accent_color"
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown setting {key}"),
                false,
            ));
        }
        let parsed_bool = match key {
            "notifications_enabled"
            | "close_to_tray"
            | "refresh_current_profile_on_start"
            | "refresh_profile_on_start" => Some(value.parse::<bool>().map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("invalid boolean {value}"),
                    false,
                )
            })?),
            _ => None,
        };
        let custom_color = if key == "custom_accent_color" {
            let hex = value.trim().trim_start_matches('#');
            if hex.is_empty() || hex.eq_ignore_ascii_case("none") {
                Some(None)
            } else if hex.len() == 6 && u32::from_str_radix(hex, 16).is_ok() {
                Some(Some(format!("#{hex}")))
            } else {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("invalid hexadecimal color {value}"),
                    false,
                ));
            }
        } else {
            None
        };
        // A theme value is a shared appearance preference: reject a typo
        // instead of storing a string no surface can resolve, and persist the
        // canonical spelling so both ends read the same value back.
        if key == "theme" && ThemePreference::parse_strict(value).is_none() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown theme {value}"),
                false,
            ));
        }
        let language = if key == "language" {
            Some(LanguagePreference::parse(value)?)
        } else {
            None
        };
        self.settings()?
            .update(|settings| match key {
                "language" => {
                    settings.language = language
                        .expect("language validated above")
                        .as_setting()
                        .into()
                }
                "theme" => {
                    settings.theme = ThemePreference::from_setting(value)
                        .as_setting()
                        .to_string()
                }
                "notifications_enabled" => {
                    settings.notifications_enabled = parsed_bool.unwrap_or(false)
                }
                "close_to_tray" => settings.close_to_tray = parsed_bool.unwrap_or(false),
                "refresh_current_profile_on_start" | "refresh_profile_on_start" => {
                    settings.refresh_current_profile_on_start = parsed_bool.unwrap_or(false);
                }
                "custom_accent_color" => {
                    settings.custom_accent_color = custom_color.expect("color validated above");
                }
                _ => {}
            })
            .await
    }

    fn shortcuts(&self) -> Result<ShortcutApplication, Failure> {
        Ok(ShortcutApplication::new(self.settings()?))
    }

    fn profile(&self) -> Result<ProfileApplication, Failure> {
        self.profile
            .clone()
            .ok_or_else(|| missing("profile application"))
    }

    fn configuration(&self) -> Result<ConfigurationApplication, Failure> {
        self.configuration
            .clone()
            .ok_or_else(|| missing("configuration application"))
    }

    fn runtime_configuration_ports(&self) -> Result<RuntimeConfigurationPorts, Failure> {
        self.runtime_configuration_ports
            .clone()
            .ok_or_else(|| missing("runtime configuration ports"))
    }

    fn connections(&self) -> Result<ConnectionApplication, Failure> {
        self.connections
            .clone()
            .ok_or_else(|| missing("connection application"))
    }

    fn runtime_queries(&self) -> Result<RuntimeQueryApplication, Failure> {
        self.runtime_queries
            .clone()
            .ok_or_else(|| missing("runtime query application"))
    }

    fn subscription_source(&self) -> Result<Arc<dyn SubscriptionSource>, Failure> {
        self.subscription_source
            .clone()
            .ok_or_else(|| missing("subscription source"))
    }

    /// Build the shared refresh orchestration, carrying the optional
    /// DUAL-07-10 notifier so both surfaces' command paths notify identically.
    fn refresh_application(
        &self,
        profile: ProfileApplication,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> SubscriptionRefreshApplication {
        let refresh = SubscriptionRefreshApplication::with_default_policy(profile, runtime);
        // The shipped command path uses the same exact active workspace
        // transaction as profile editing.
        let refresh = match &self.profile_reference_applier {
            Some(managed) => refresh.with_profile_reference_applier(Arc::clone(managed)),
            None => refresh,
        };
        match &self.notifier {
            Some(notifier) => refresh.with_notifier(Arc::clone(notifier)),
            None => refresh,
        }
    }

    fn doctor(&self) -> Result<DoctorApplication, Failure> {
        self.doctor
            .clone()
            .ok_or_else(|| missing("doctor application"))
    }

    fn routing(&self) -> Result<RoutingApplication, Failure> {
        self.routing
            .clone()
            .ok_or_else(|| missing("routing application"))
    }

    fn sync(&self) -> Result<SyncApplication, Failure> {
        self.sync.clone().ok_or_else(|| missing("sync application"))
    }

    fn settings(&self) -> Result<SettingsApplication, Failure> {
        self.settings
            .clone()
            .ok_or_else(|| missing("settings application"))
    }

    /// DUAL-09-03/14: profile document use-cases over the same profile store.
    fn profile_document(&self) -> Result<ProfileDocumentApplication, Failure> {
        Ok(ProfileDocumentApplication::new(self.profile()?.clone()))
    }

    /// DUAL-09-14: the shared Mixin/filter sidecar use-case the editor panes
    /// call on both surfaces.
    fn profile_options(&self) -> Result<ProfileOptionsApplication, Failure> {
        Ok(ProfileOptionsApplication::new(self.profile()?))
    }

    fn snapshots(&self) -> Result<SnapshotApplication, Failure> {
        self.snapshots
            .clone()
            .ok_or_else(|| missing("snapshot application"))
    }

    fn versions(&self) -> Result<VersionApplication, Failure> {
        self.versions
            .clone()
            .ok_or_else(|| missing("version application"))
    }

    fn service_mode(&self) -> Result<ServiceModeApplication, Failure> {
        self.service_mode
            .clone()
            .ok_or_else(|| missing("service mode application"))
    }

    fn port_conflicts(&self) -> Result<PortConflictApplication, Failure> {
        self.port_conflicts
            .clone()
            .ok_or_else(|| missing("port conflict application"))
    }

    fn mtu(&self) -> Result<MtuApplication, Failure> {
        self.mtu.clone().ok_or_else(|| missing("MTU application"))
    }

    fn system_proxy(&self) -> Result<SystemProxyApplication, Failure> {
        self.system_proxy.clone().ok_or_else(|| {
            Failure::unsupported("system proxy control is not composed for this host")
        })
    }

    fn uwp_loopback(&self) -> Result<UwpLoopbackApplication, Failure> {
        self.uwp_loopback
            .clone()
            .ok_or_else(|| Failure::unsupported("UWP loopback is not configured for this host"))
    }

    fn pac(&self) -> Result<PacApplication, Failure> {
        self.pac
            .clone()
            .ok_or_else(|| Failure::unsupported("PAC service is not configured for this host"))
    }

    fn network_roaming(&self) -> Result<NetworkRoamingApplication, Failure> {
        self.network_roaming
            .clone()
            .ok_or_else(|| Failure::unsupported("network roaming is not configured for this host"))
    }

    fn vpn(&self) -> Result<VpnServiceApplication, Failure> {
        self.vpn
            .clone()
            .ok_or_else(|| Failure::unsupported("VpnService is not configured for this host"))
    }

    fn privileged_network(&self) -> Result<PrivilegedNetworkApplication, Failure> {
        self.privileged_network.clone().ok_or_else(|| {
            Failure::unsupported("privileged network regression is not configured for this host")
        })
    }

    fn outbound(&self) -> Result<OutboundApplication, Failure> {
        self.outbound
            .clone()
            .ok_or_else(|| Failure::unsupported("outbound application is not composed"))
    }

    fn outbound_ui_preferences(&self) -> Result<OutboundUiPreferencesApplication, Failure> {
        self.outbound_ui_preferences.clone().ok_or_else(|| {
            Failure::unsupported("outbound UI preference application is not composed")
        })
    }

    fn rule_tracer(&self) -> Result<RuleTracerApplication, Failure> {
        self.rule_tracer
            .clone()
            .ok_or_else(|| missing("rule tracer application"))
    }
}

/// Bounded concurrency for the shared "update all subscriptions" batch.
const BATCH_UPDATE_CONCURRENCY: usize = 5;

fn parse_release_channel(
    value: &str,
    engine_id: &EngineId,
) -> Result<EngineReleaseChannel, Failure> {
    let channel_id = ReleaseChannelId::new(value.trim().to_ascii_lowercase()).map_err(|error| {
        Failure::new(
            ErrorCode::InvalidInput,
            format!("invalid core release channel ID: {error}"),
            false,
        )
    })?;
    Ok(EngineReleaseChannel::new(engine_id.clone(), channel_id))
}

fn missing(capability: &str) -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        format!("{capability} is not configured for this host"),
        false,
    )
}

fn unsupported() -> Failure {
    Failure::unsupported("command has no host port in this composition")
}

#[cfg(test)]
#[path = "command_application_tests.rs"]
mod rule_edit_tests;

#[cfg(test)]
#[path = "command_application_settings_tests.rs"]
mod settings_write_tests;
