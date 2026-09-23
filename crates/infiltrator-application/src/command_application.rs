//! Shared command routing for inbound surfaces.
//!
//! CoreApplication owns lifecycle state and its private worker. This module
//! owns the rest of the application use-case dispatch and is installed by a
//! host composition when that surface wants the full command vocabulary.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::mtu::MtuProbeState;
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use infiltrator_contract::version::CoreReleaseChannel;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::edit;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway};
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use infiltrator_ports::subscription_notification::SubscriptionNotificationPort;
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::doctor_application::DoctorApplication;
use crate::mtu_application::MtuApplication;
use crate::network_roaming_application::NetworkRoamingApplication;
use crate::pac_application::PacApplication;
use crate::port_conflict_application::PortConflictApplication;
use crate::privileged_network_application::PrivilegedNetworkApplication;
use crate::profile_application::ProfileApplication;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_options_application::ProfileOptionsApplication;
use crate::routing_application::RoutingApplication;
use crate::runtime_query_application::RuntimeQueryApplication;
use crate::service_mode_application::ServiceModeApplication;
use crate::settings_application::SettingsApplication;
use crate::shortcut_application::ShortcutApplication;
use crate::snapshot_application::SnapshotApplication;
use crate::subscription_refresh_application::SubscriptionRefreshApplication;
use crate::sync_application::SyncApplication;
use crate::system_proxy_application::SystemProxyApplication;
use crate::uwp_loopback_application::UwpLoopbackApplication;
use crate::version_application::VersionApplication;
use crate::vpn_application::VpnServiceApplication;

mod dispatch;

pub type CommandFuture = Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'static>>;

/// Extension point consumed by CoreApplication for commands beyond lifecycle
/// and proxy mode.
pub trait CommandHandler: Send + Sync {
    fn handle(&self, intent: CommandIntent) -> CommandFuture;
}

#[derive(Clone, Default)]
pub struct CommandApplication {
    profile: Option<ProfileApplication>,
    runtime: Option<Arc<dyn RuntimeGateway>>,
    managed_runtime: Option<Arc<dyn ManagedRuntime>>,
    /// DUAL-07-05/06: executor-neutral delay seam + single-flight refresh.
    application_runtime: Option<Arc<dyn ApplicationRuntime>>,
    subscription_source: Option<Arc<dyn SubscriptionSource>>,
    /// DUAL-07-01: host port for local-file / clipboard import channels.
    import_source: Option<Arc<dyn SubscriptionImportPort>>,
    /// DUAL-07-10: host port for subscription system notifications.
    notifier: Option<Arc<dyn SubscriptionNotificationPort>>,
    /// DUAL-05-13: host port that reads CA certificate bundles.
    certificate_authority:
        Option<Arc<dyn infiltrator_ports::certificate_authority::CertificateAuthorityPort>>,
    doctor: Option<DoctorApplication>,
    routing: Option<RoutingApplication>,
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
    speedtest: Option<crate::speedtest_application::SpeedtestApplication>,
    proxy_preferences: Option<crate::proxy_preferences_application::ProxyPreferencesApplication>,
    rule_tracer: Option<crate::rule_tracer_application::RuleTracerApplication>,
    configuration: Option<crate::configuration_application::ConfigurationApplication>,
    dns_cache: Option<crate::dns_cache_application::DnsCacheApplication>,
    /// DUAL-14-10: the shared latency prober `TestDnsLatency` drives.
    dns_latency: Option<crate::dns_latency_application::DnsLatencyApplication>,
    /// DUAL-14-08: the shared cross-source leak prober `TestDnsLeak` drives.
    dns_leak: Option<crate::dns_leak_application::DnsLeakApplication>,
    /// DUAL-14-09 (re-scoped): the shared STUN UDP-egress prober `RunStunProbe`
    /// drives.
    stun_probe: Option<crate::stun_probe_application::StunProbeApplication>,
    rule_provider: Option<crate::rule_provider_application::RuleProviderApplication>,
}

impl CommandApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_profile(mut self, application: ProfileApplication) -> Self {
        self.profile = Some(application);
        self
    }

    pub fn with_runtime(mut self, runtime: Arc<dyn RuntimeGateway>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_managed_runtime(mut self, runtime: Arc<dyn ManagedRuntime>) -> Self {
        self.managed_runtime = Some(runtime);
        self
    }

    pub fn with_subscription_source(mut self, source: Arc<dyn SubscriptionSource>) -> Self {
        self.subscription_source = Some(source);
        self
    }

    /// DUAL-07-01: install the host import port backing the local-file /
    /// clipboard import channels.
    pub fn with_import_source(mut self, port: Arc<dyn SubscriptionImportPort>) -> Self {
        self.import_source = Some(port);
        self
    }

    /// DUAL-07-10: install the host port that dispatches subscription
    /// success/failure system notifications.
    pub fn with_subscription_notifier(
        mut self,
        notifier: Arc<dyn SubscriptionNotificationPort>,
    ) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// DUAL-05-13: install the host CA-bundle reader.
    ///
    /// Hosts without one keep the field empty and the shared application
    /// reports the request as typed unsupported instead of claiming a load.
    pub fn with_certificate_authority(
        mut self,
        port: Arc<dyn infiltrator_ports::certificate_authority::CertificateAuthorityPort>,
    ) -> Self {
        self.certificate_authority = Some(port);
        self
    }

    /// Install the executor-neutral delay seam used by subscription
    /// retry/backoff (DUAL-07-05) and the shared single-flight refresh
    /// (DUAL-07-06). A host that does not compose one keeps the plain path.
    pub fn with_application_runtime(mut self, runtime: Arc<dyn ApplicationRuntime>) -> Self {
        self.application_runtime = Some(runtime);
        self
    }

    pub fn with_doctor(mut self, application: DoctorApplication) -> Self {
        self.doctor = Some(application);
        self
    }

    pub fn with_routing(mut self, application: RoutingApplication) -> Self {
        self.routing = Some(application);
        self
    }

    pub fn with_sync(mut self, application: SyncApplication) -> Self {
        self.sync = Some(application);
        self
    }

    pub fn with_settings(mut self, application: SettingsApplication) -> Self {
        self.settings = Some(application);
        self
    }

    pub fn with_snapshots(mut self, application: SnapshotApplication) -> Self {
        self.snapshots = Some(application);
        self
    }

    pub fn with_versions(mut self, application: VersionApplication) -> Self {
        self.versions = Some(application);
        self
    }

    pub fn with_service_mode(mut self, application: ServiceModeApplication) -> Self {
        self.service_mode = Some(application);
        self
    }

    pub fn with_port_conflicts(mut self, application: PortConflictApplication) -> Self {
        self.port_conflicts = Some(application);
        self
    }

    pub fn with_mtu(mut self, application: MtuApplication) -> Self {
        self.mtu = Some(application);
        self
    }

    pub fn with_system_proxy(mut self, application: SystemProxyApplication) -> Self {
        self.system_proxy = Some(application);
        self
    }

    pub fn with_uwp_loopback(mut self, application: UwpLoopbackApplication) -> Self {
        self.uwp_loopback = Some(application);
        self
    }

    pub fn with_pac(mut self, application: PacApplication) -> Self {
        self.pac = Some(application);
        self
    }

    pub fn with_network_roaming(mut self, application: NetworkRoamingApplication) -> Self {
        self.network_roaming = Some(application);
        self
    }

    pub fn with_vpn(mut self, application: VpnServiceApplication) -> Self {
        self.vpn = Some(application);
        self
    }

    pub fn with_privileged_network(mut self, application: PrivilegedNetworkApplication) -> Self {
        self.privileged_network = Some(application);
        self
    }

    pub fn with_speedtest(
        mut self,
        speedtest: crate::speedtest_application::SpeedtestApplication,
    ) -> Self {
        self.speedtest = Some(speedtest);
        self
    }

    pub fn with_proxy_preferences(
        mut self,
        preferences: crate::proxy_preferences_application::ProxyPreferencesApplication,
    ) -> Self {
        self.proxy_preferences = Some(preferences);
        self
    }

    pub fn with_rule_tracer(
        mut self,
        rule_tracer: crate::rule_tracer_application::RuleTracerApplication,
    ) -> Self {
        self.rule_tracer = Some(rule_tracer);
        self
    }

    /// Share the DNS cache flush application with the surface reader so the
    /// honest Fake-IP / OS-cache report reaches both surfaces.
    pub fn with_dns_cache(
        mut self,
        dns_cache: crate::dns_cache_application::DnsCacheApplication,
    ) -> Self {
        self.dns_cache = Some(dns_cache);
        self
    }

    /// DUAL-14-10: share the per-nameserver latency application so
    /// `TestDnsLatency` runs the real host probe and both surfaces publish the
    /// same report.
    pub fn with_dns_latency(
        mut self,
        dns_latency: crate::dns_latency_application::DnsLatencyApplication,
    ) -> Self {
        self.dns_latency = Some(dns_latency);
        self
    }

    /// DUAL-14-08: share the cross-source leak application so `TestDnsLeak`
    /// runs the real host probe and both surfaces publish the same report.
    pub fn with_dns_leak(
        mut self,
        dns_leak: crate::dns_leak_application::DnsLeakApplication,
    ) -> Self {
        self.dns_leak = Some(dns_leak);
        self
    }

    /// DUAL-14-09 (re-scoped): share the STUN UDP-egress application so
    /// `RunStunProbe` runs the real host probe and both surfaces publish the
    /// same report.
    pub fn with_stun_probe(
        mut self,
        stun_probe: crate::stun_probe_application::StunProbeApplication,
    ) -> Self {
        self.stun_probe = Some(stun_probe);
        self
    }

    /// DUAL-11-06/07: install the host's rule-provider cache access. Without
    /// it the unpack can still serve inline payloads and the controller
    /// payload, while a purge answers a typed unsupported instead of claiming
    /// a cleanup.
    pub fn with_rule_provider_cache(
        mut self,
        cache: Arc<dyn infiltrator_ports::rule_provider_cache::RuleProviderCachePort>,
    ) -> Self {
        self.rule_provider =
            Some(crate::rule_provider_application::RuleProviderApplication::new(Some(cache)));
        self
    }

    /// Install the validated profile configuration application so surfaces can
    /// submit their shared DNS workbench edits through the same write path.
    pub fn with_configuration(
        mut self,
        configuration: crate::configuration_application::ConfigurationApplication,
    ) -> Self {
        self.configuration = Some(configuration);
        self
    }

    /// Execute one inbound command through the shared dispatch table.
    pub async fn execute(&self, intent: CommandIntent) -> Result<(), Failure> {
        self.execute_dispatch(intent).await
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
        let servers = crate::dns_workbench_application::dns_servers(&config);
        application.probe(&servers).await.map(|_| ())
    }

    /// DUAL-14-08: run the shared cross-source leak probe. The application
    /// already owns the configured echo sources, so a host without one answers
    /// a typed unsupported instead of inventing a verdict.
    async fn test_dns_leak(&self) -> Result<(), Failure> {
        let Some(application) = self.dns_leak.as_ref() else {
            return Err(Failure::unsupported(
                crate::dns_leak_application::NO_ECHO_PORT_REASON,
            ));
        };
        infiltrator_ports::dns_leak::DnsLeakProbePort::probe(application)
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
            return Err(Failure::unsupported(
                crate::stun_probe_application::NO_STUN_PORT_REASON,
            ));
        };
        infiltrator_ports::stun_probe::StunEgressProbePort::probe(application)
            .await
            .map(|_| ())
            .map_err(Failure::from)
    }

    /// DUAL-11-06: read the active profile's declaration for `provider_name`,
    /// resolve its real rules through the shared unpack service and prepend
    /// them to the persisted rule list.
    async fn unpack_rule_provider(&self, provider_name: &str) -> Result<(), Failure> {
        let name = provider_name.trim();
        if name.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "rule provider name is empty",
                false,
            ));
        }
        let providers = self.configuration()?.load_rule_providers().await?;
        let declaration =
            infiltrator_domain::rules::provider_store::parse_rule_provider_declarations(&providers)
                .into_iter()
                .find(|declaration| declaration.name == name)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::Configuration,
                        format!("the active profile declares no rule provider named {name}"),
                        false,
                    )
                })?;
        let plan = self
            .rule_provider_service()
            .deconstruct(
                &declaration,
                edit::DEFAULT_RULE_TARGET,
                self.runtime.as_deref(),
            )
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
    fn rule_provider_service(&self) -> crate::rule_provider_application::RuleProviderApplication {
        self.rule_provider.clone().unwrap_or_default()
    }

    fn rule_provider(
        &self,
    ) -> Result<crate::rule_provider_application::RuleProviderApplication, Failure> {
        self.rule_provider.clone().ok_or_else(|| {
            Failure::new(
                ErrorCode::Unsupported,
                "this host does not expose a rule-provider cache location",
                false,
            )
        })
    }

    /// DUAL-11-09/10/11/12: apply a rule-list mutation to the active profile
    /// and persist it atomically. `mutate` reports whether it changed anything;
    /// an unchanged list is a no-op so a stale click never rewrites the file.
    async fn edit_rules<F>(&self, mutate: F) -> Result<(), Failure>
    where
        F: FnOnce(&mut Vec<infiltrator_domain::rules::RuleEntry>) -> bool,
    {
        let profile = self.profile()?;
        let (name, content) = profile.current_content().await?;
        let mut rules =
            infiltrator_domain::rules::load_rules_from_yaml(&content).map_err(rule_failure)?;
        if !mutate(&mut rules) {
            return Ok(());
        }
        let updated = infiltrator_domain::rules::apply_rules_to_yaml(&content, &rules)
            .map_err(rule_failure)?;
        profile.save_profile(&name, &updated).await
    }

    /// DUAL-11-14: apply one rules-workspace JSON document. The section decides
    /// which shared configuration use-case owns the write; the document is
    /// parsed and validated there, so a malformed editor buffer is refused
    /// before the profile is touched.
    async fn apply_rules_json_document(
        &self,
        section: infiltrator_contract::rules_workspace::RulesJsonSection,
        json: &str,
    ) -> Result<(), Failure> {
        use infiltrator_contract::rules_workspace::RulesJsonSection;
        if json.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "the JSON document is empty",
                false,
            ));
        }
        let configuration = self.configuration()?;
        let invalid = |error: &str| Failure::new(ErrorCode::InvalidInput, error.to_owned(), false);
        match section {
            RulesJsonSection::RuleProviders => {
                let providers: infiltrator_domain::rules::RuleProviders =
                    serde_json::from_str(json).map_err(|error| {
                        invalid(&format!("invalid rule providers JSON: {error}"))
                    })?;
                configuration
                    .save_rule_providers(providers)
                    .await
                    .map(|_| ())
            }
            RulesJsonSection::ProxyProviders => {
                let providers: infiltrator_domain::proxy_providers::ProxyProviders =
                    serde_json::from_str(json).map_err(|error| {
                        invalid(&format!("invalid proxy providers JSON: {error}"))
                    })?;
                configuration
                    .save_proxy_providers(providers)
                    .await
                    .map(|_| ())
            }
            RulesJsonSection::Sniffer => {
                let config: serde_json::Value = serde_json::from_str(json)
                    .map_err(|error| invalid(&format!("invalid sniffer JSON: {error}")))?;
                configuration.save_sniffer_config(config).await.map(|_| ())
            }
        }
    }

    async fn update_setting(&self, key: &str, value: &str) -> Result<(), Failure> {
        let key = key.trim();
        let value = value.trim();
        if let Some(action_id) = key.strip_prefix("shortcut.") {
            let action = infiltrator_contract::shortcuts::ShortcutAction::from_id(action_id)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::InvalidInput,
                        format!("unknown shortcut action {action_id}"),
                        false,
                    )
                })?;
            let chord =
                infiltrator_contract::shortcuts::ShortcutChord::parse(value).ok_or_else(|| {
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
            "language" | "theme" | "notifications_enabled" | "close_to_tray"
        ) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown setting {key}"),
                false,
            ));
        }
        let parsed_bool = match key {
            "notifications_enabled" | "close_to_tray" => {
                Some(value.parse::<bool>().map_err(|_| {
                    Failure::new(
                        ErrorCode::InvalidInput,
                        format!("invalid boolean {value}"),
                        false,
                    )
                })?)
            }
            _ => None,
        };
        // A theme value is a shared appearance preference: reject a typo
        // instead of storing a string no surface can resolve, and persist the
        // canonical spelling so both ends read the same value back.
        if key == "theme"
            && infiltrator_contract::theme::ThemePreference::parse_strict(value).is_none()
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown theme {value}"),
                false,
            ));
        }
        self.settings()?
            .update(|settings| match key {
                "language" => settings.language = value.to_string(),
                "theme" => {
                    settings.theme =
                        infiltrator_contract::theme::ThemePreference::from_setting(value)
                            .as_setting()
                            .to_string()
                }
                "notifications_enabled" => {
                    settings.notifications_enabled = parsed_bool.unwrap_or(false)
                }
                "close_to_tray" => settings.close_to_tray = parsed_bool.unwrap_or(false),
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

    fn configuration(
        &self,
    ) -> Result<crate::configuration_application::ConfigurationApplication, Failure> {
        self.configuration
            .clone()
            .ok_or_else(|| missing("configuration application"))
    }

    fn runtime(&self) -> Result<Arc<dyn RuntimeGateway>, Failure> {
        self.runtime
            .clone()
            .ok_or_else(|| missing("runtime gateway"))
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
        // DUAL-07-09: the shipped command path reloads the updated active
        // profile through the same managed-runtime seam it already holds.
        let refresh = match &self.managed_runtime {
            Some(managed) => refresh.with_core_reload(Arc::clone(managed)),
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

    fn speedtest(&self) -> Result<crate::speedtest_application::SpeedtestApplication, Failure> {
        self.speedtest
            .clone()
            .ok_or_else(|| missing("speedtest application"))
    }

    fn rule_tracer(
        &self,
    ) -> Result<crate::rule_tracer_application::RuleTracerApplication, Failure> {
        self.rule_tracer
            .clone()
            .ok_or_else(|| missing("rule tracer application"))
    }
}

impl CommandHandler for CommandApplication {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let application = self.clone();
        Box::pin(async move { application.execute(intent).await })
    }
}

const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_DELAY_TIMEOUT_MS: u32 = 5000;
const DEFAULT_DELAY_CONCURRENCY: usize = 30;
/// Bounded concurrency for the shared "update all subscriptions" batch.
const BATCH_UPDATE_CONCURRENCY: usize = 5;

fn delay_candidates(
    proxies: &std::collections::HashMap<String, Proxy>,
    group: Option<&str>,
) -> Result<Vec<String>, Failure> {
    let candidates = match group {
        Some(group) => match proxies.get(group) {
            Some(proxy) if proxy.is_group() => proxy.all().unwrap_or_default().to_vec(),
            Some(_) => {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("{group} is not a proxy group"),
                    false,
                ));
            }
            None => {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    format!("proxy group {group} was not found"),
                    false,
                ));
            }
        },
        None => proxies
            .values()
            .filter(|proxy| !proxy.is_group() && !matches!(proxy, Proxy::Unknown))
            .map(|proxy| proxy.name().to_string())
            .filter(|name| !name.is_empty())
            .collect(),
    };
    let mut unique = HashSet::new();
    Ok(candidates
        .into_iter()
        .filter(|candidate| unique.insert(candidate.clone()))
        .collect())
}

fn parse_routing_mode(value: &str) -> Result<AppRoutingMode, Failure> {
    match value.trim().to_ascii_lowercase().as_str() {
        "proxy_all" | "global" => Ok(AppRoutingMode::ProxyAll),
        "proxy_selected" | "whitelist" => Ok(AppRoutingMode::ProxySelected),
        "bypass_selected" | "blacklist" => Ok(AppRoutingMode::BypassSelected),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            format!("unknown app routing mode {value}"),
            false,
        )),
    }
}

fn parse_routing_rule(value: &str) -> Result<AppRoutingRule, Failure> {
    match value.trim().to_ascii_lowercase().as_str() {
        "proxy" => Ok(AppRoutingRule::Proxy),
        "direct" => Ok(AppRoutingRule::Direct),
        "block" => Ok(AppRoutingRule::Block),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            format!("unknown app routing rule {value}"),
            false,
        )),
    }
}

fn parse_release_channel(value: &str) -> Result<CoreReleaseChannel, Failure> {
    match value.trim().to_ascii_lowercase().as_str() {
        "stable" => Ok(CoreReleaseChannel::Stable),
        "alpha" | "pre-release" | "prerelease" => Ok(CoreReleaseChannel::Alpha),
        "meta" | "meta-core" | "metacore" | "nightly" => Ok(CoreReleaseChannel::MetaCore),
        _ => Err(Failure::new(
            ErrorCode::InvalidInput,
            format!("unknown core release channel {value}"),
            false,
        )),
    }
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

/// Configuration failure for the shared rule-edit path.
fn rule_failure(error: impl std::fmt::Display) -> Failure {
    Failure::new(ErrorCode::Configuration, error.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_android_and_desktop_routing_vocabulary() {
        assert_eq!(parse_routing_mode("global"), Ok(AppRoutingMode::ProxyAll));
        assert_eq!(
            parse_routing_mode("proxy_selected"),
            Ok(AppRoutingMode::ProxySelected)
        );
        assert_eq!(parse_routing_rule("block"), Ok(AppRoutingRule::Block));
        assert!(parse_routing_rule("drop").is_err());
    }
}

#[cfg(test)]
#[path = "command_application_tests.rs"]
mod rule_edit_tests;

#[cfg(test)]
#[path = "command_application_settings_tests.rs"]
mod settings_write_tests;
