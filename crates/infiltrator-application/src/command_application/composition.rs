//! Explicit dependency composition for the shared command application.

use super::{
    ActiveProfileReferenceApplier, ApplicationRuntime, CapabilityProvider,
    CertificateAuthorityPort, CommandApplication, ConfigurationApplication, ConnectionApplication,
    CoreCapabilityApplication, DnsCacheApplication, DnsLatencyApplication, DnsLeakApplication,
    DnsQueryApplication, DoctorApplication, GeoDatabaseApplication, LogApplication,
    LogExportApplication, MtuApplication, NetworkRoamingApplication, OutboundApplication,
    OutboundUiPreferencesApplication, OverviewLayoutApplication, PacApplication,
    PortConflictApplication, PrivilegedNetworkApplication, ProfileApplication, RoutingApplication,
    RoutingIntentPort, RuleListApplication, RuleProviderApplication, RuleProviderCachePort,
    RuleProviderRuntimeApplication, RuleTracerApplication, RuntimeQueryApplication,
    ServiceModeApplication, SettingsApplication, SnapshotApplication, StunProbeApplication,
    SubscriptionImportPort, SubscriptionNotificationPort, SubscriptionSource, SyncApplication,
    SystemProxyApplication, UwpLoopbackApplication, VersionApplication, VpnServiceApplication,
};
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use std::sync::Arc;

impl CommandApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_profile(mut self, application: ProfileApplication) -> Self {
        self.profile = Some(application);
        self
    }

    pub fn with_core_capabilities(mut self, application: CoreCapabilityApplication) -> Self {
        self.core_capabilities = application;
        self
    }

    pub fn with_outbound(mut self, application: OutboundApplication) -> Self {
        self.outbound = Some(application);
        self
    }

    pub fn with_outbound_ui_preferences(
        mut self,
        application: OutboundUiPreferencesApplication,
    ) -> Self {
        self.outbound_ui_preferences = Some(application);
        self
    }

    pub fn with_host_capabilities(mut self, provider: Arc<dyn CapabilityProvider>) -> Self {
        self.host_capabilities = Some(provider);
        self
    }

    pub fn with_runtime_configuration_ports(mut self, ports: RuntimeConfigurationPorts) -> Self {
        self.runtime_configuration_ports = Some(ports);
        self
    }

    pub fn with_connections(mut self, application: ConnectionApplication) -> Self {
        self.connections = Some(application);
        self
    }

    pub fn with_runtime_queries(mut self, application: RuntimeQueryApplication) -> Self {
        self.runtime_queries = Some(application);
        self
    }

    pub fn with_profile_reference_applier(
        mut self,
        runtime: Arc<dyn ActiveProfileReferenceApplier>,
    ) -> Self {
        self.profile_reference_applier = Some(runtime);
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
    pub fn with_certificate_authority(mut self, port: Arc<dyn CertificateAuthorityPort>) -> Self {
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

    pub fn with_logs(mut self, logs: LogApplication) -> Self {
        self.log_export = Some(LogExportApplication::new(logs.clone(), None, Vec::new()));
        self.logs = Some(logs);
        self
    }

    pub fn with_log_export(mut self, application: LogExportApplication) -> Self {
        assert!(
            self.logs
                .as_ref()
                .is_some_and(|logs| application.belongs_to(logs)),
            "Log export must use the command facade's log owner"
        );
        self.log_export = Some(application);
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

    pub fn with_routing_intent_port(mut self, port: Arc<dyn RoutingIntentPort>) -> Self {
        self.routing_intent_port = Some(port);
        self
    }

    pub fn with_sync(mut self, application: SyncApplication) -> Self {
        self.sync = Some(application);
        self
    }

    /// Install the Overview page's card-order owner.
    ///
    /// The owner is the single source of the card order: both surfaces submit
    /// the same typed intents and read the order back from its snapshot.
    pub fn with_overview_layout(mut self, application: OverviewLayoutApplication) -> Self {
        self.overview_layout = Some(application);
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

    pub fn with_rule_tracer(mut self, rule_tracer: RuleTracerApplication) -> Self {
        self.rule_tracer = Some(rule_tracer);
        self
    }
    pub fn with_rule_list(mut self, application: RuleListApplication) -> Self {
        self.rule_list = Some(application);
        self
    }

    /// Share the DNS cache flush application with the surface reader so the
    /// honest Fake-IP / OS-cache report reaches both surfaces.
    pub fn with_dns_query(mut self, application: DnsQueryApplication) -> Self {
        self.dns_query = Some(application);
        self
    }

    pub fn with_dns_cache(mut self, dns_cache: DnsCacheApplication) -> Self {
        self.dns_cache = Some(dns_cache);
        self
    }

    /// DUAL-14-10: share the per-nameserver latency application so
    /// `TestDnsLatency` runs the real host probe and both surfaces publish the
    /// same report.
    pub fn with_dns_latency(mut self, dns_latency: DnsLatencyApplication) -> Self {
        self.dns_latency = Some(dns_latency);
        self
    }

    /// DUAL-14-08: share the cross-source leak application so `TestDnsLeak`
    /// runs the real host probe and both surfaces publish the same report.
    pub fn with_dns_leak(mut self, dns_leak: DnsLeakApplication) -> Self {
        self.dns_leak = Some(dns_leak);
        self
    }

    /// DUAL-14-09 (re-scoped): share the STUN UDP-egress application so
    /// `RunStunProbe` runs the real host probe and both surfaces publish the
    /// same report.
    pub fn with_stun_probe(mut self, stun_probe: StunProbeApplication) -> Self {
        self.stun_probe = Some(stun_probe);
        self
    }

    /// DUAL-11-06/07: install the host's rule-provider cache access. Without
    /// it the unpack can still serve inline payloads and the controller
    /// payload, while a purge answers a typed unsupported instead of claiming
    /// a cleanup.
    pub fn with_rule_provider_cache(mut self, cache: Arc<dyn RuleProviderCachePort>) -> Self {
        self.rule_provider = Some(RuleProviderApplication::new(Some(cache)));
        self
    }

    pub fn with_rule_provider_runtime(
        mut self,
        application: RuleProviderRuntimeApplication,
    ) -> Self {
        self.rule_provider_runtime = Some(application);
        self
    }

    pub fn with_geo_database(mut self, application: GeoDatabaseApplication) -> Self {
        self.geo_database = Some(application);
        self
    }

    /// Install the validated profile configuration application so surfaces can
    /// submit their shared DNS workbench edits through the same write path.
    pub fn with_configuration(mut self, configuration: ConfigurationApplication) -> Self {
        self.configuration = Some(configuration);
        self
    }
}
