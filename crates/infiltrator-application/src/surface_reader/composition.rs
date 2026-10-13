use super::*;

impl ApplicationSurfaceReader {
    pub fn new(core: Arc<CoreApplication>, surface: SurfaceKind, host: HostKind) -> Self {
        let outbound_ui_preferences = Some(core.outbound_ui_preferences_application());
        Self {
            scripts: None,
            script_exports: None,
            shell_readout: ShellReadoutApplication::default(),
            runtime_control: RuntimeControlApplication::default(),
            core,
            gateway: None,
            connection_port: None,
            routing_intent_port: None,
            profiles: None,
            configuration: None,
            doctor: None,
            read_revision: Arc::new(AtomicU64::new(0)),
            routing: None,
            settings: None,
            snapshots: None,
            versions: None,
            endpoint_source: None,
            service_mode: None,
            port_conflicts: None,
            resources: None,
            offline_startup: None,
            mtu: None,
            system_proxy: None,
            uwp_loopback: None,
            pac: None,
            network_roaming: None,
            vpn: None,
            privileged_network: None,
            traffic_waveform: TrafficWaveformApplication::new(),
            traffic_scale: TrafficScaleApplication,
            traffic_topology: TrafficTopologyApplication,
            active_exit: ActiveExitApplication,
            rule_tracer: RuleTracerApplication::new(),
            subscription_quota: SubscriptionQuotaApplication::default(),
            dns_cache: None,
            dns_query: None,
            dns_latency: None,
            dns_leak: None,
            stun_probe: None,
            outbound: None,
            outbound_ui_preferences,
            rule_provider: RuleProviderApplication::default(),
            connection_rates: ConnectionRateApplication::new(),
            version_cache: Arc::new(Mutex::new(None)),
            capabilities: CapabilitySnapshot::new(host, 0, Vec::new()),
            surface,
        }
    }

    pub fn with_scripts(
        mut self,
        scripts: ScriptApplication,
        exports: ScriptExportApplication,
    ) -> Self {
        self.scripts = Some(scripts);
        self.script_exports = Some(exports);
        self
    }

    pub fn with_capabilities(mut self, capabilities: CapabilitySnapshot) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_gateway(mut self, gateway: Arc<dyn RuntimeGateway>) -> Self {
        self.connection_port = Some(gateway.connection_port());
        self.gateway = Some(gateway);
        self
    }

    pub fn with_routing_intent_port(mut self, port: Arc<dyn RoutingIntentPort>) -> Self {
        self.routing_intent_port = Some(port);
        self
    }

    pub fn with_outbound(mut self, outbound: OutboundApplication) -> Self {
        self.outbound = Some(outbound);
        self
    }

    pub fn with_outbound_ui_preferences(
        mut self,
        application: OutboundUiPreferencesApplication,
    ) -> Self {
        self.outbound_ui_preferences = Some(application);
        self
    }

    pub fn with_profiles(mut self, profiles: ProfileApplication) -> Self {
        self.profiles = Some(profiles);
        self
    }

    pub fn with_configuration(mut self, configuration: ConfigurationApplication) -> Self {
        self.configuration = Some(configuration);
        self
    }

    pub fn with_doctor(mut self, doctor: DoctorApplication) -> Self {
        self.doctor = Some(doctor);
        self
    }

    pub fn with_routing(mut self, routing: RoutingApplication) -> Self {
        self.routing = Some(routing);
        self
    }

    pub fn with_settings(mut self, settings: SettingsApplication) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn with_snapshots(mut self, snapshots: SnapshotApplication) -> Self {
        self.snapshots = Some(snapshots);
        self
    }

    pub fn with_versions(mut self, versions: VersionApplication) -> Self {
        self.versions = Some(versions);
        self
    }

    pub fn with_endpoint_source(mut self, source: Arc<dyn EndpointSource>) -> Self {
        self.endpoint_source = Some(source);
        self
    }

    pub fn with_service_mode(mut self, service_mode: ServiceModeApplication) -> Self {
        self.service_mode = Some(service_mode);
        self
    }

    pub fn with_port_conflicts(mut self, port_conflicts: PortConflictApplication) -> Self {
        self.port_conflicts = Some(port_conflicts);
        self
    }

    pub fn with_resources(mut self, resources: ResourceApplication) -> Self {
        self.resources = Some(resources);
        self
    }

    pub fn with_offline_startup(mut self, startup: OfflineStartupApplication) -> Self {
        self.offline_startup = Some(startup);
        self
    }

    pub fn with_mtu(mut self, mtu: MtuApplication) -> Self {
        self.mtu = Some(mtu);
        self
    }

    pub fn with_system_proxy(mut self, system_proxy: SystemProxyApplication) -> Self {
        self.system_proxy = Some(system_proxy);
        self
    }

    pub fn with_uwp_loopback(mut self, uwp_loopback: UwpLoopbackApplication) -> Self {
        self.uwp_loopback = Some(uwp_loopback);
        self
    }

    pub fn with_pac(mut self, pac: PacApplication) -> Self {
        self.pac = Some(pac);
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

    /// Share the live rule tracer engine with the host composition so UI
    /// intents and surface projections observe one query state.
    pub fn with_rule_tracer(mut self, application: RuleTracerApplication) -> Self {
        self.rule_tracer = application;
        self
    }

    /// Share the DNS cache application so the published read model carries the
    /// honest last Fake-IP / OS-cache flush report.
    pub fn with_dns_query(mut self, application: DnsQueryApplication) -> Self {
        self.dns_query = Some(application);
        self
    }

    pub fn with_dns_cache(mut self, application: DnsCacheApplication) -> Self {
        self.dns_cache = Some(application);
        self
    }

    /// DUAL-14-10/13: share the latency probe application so the published read
    /// model carries the last real per-nameserver probe and the DNS self-heal
    /// observation derived from it.
    pub fn with_dns_latency(mut self, application: DnsLatencyApplication) -> Self {
        self.dns_latency = Some(application);
        self
    }

    /// DUAL-14-08: share the leak probe application so the published read model
    /// carries the last real cross-source observation.
    pub fn with_dns_leak(mut self, application: DnsLeakApplication) -> Self {
        self.dns_leak = Some(application);
        self
    }

    /// DUAL-14-09 (re-scoped): share the STUN UDP-egress probe application so
    /// the published read model carries the last real host observation and its
    /// comparison against the expected proxied egress.
    pub fn with_stun_probe(mut self, application: StunProbeApplication) -> Self {
        self.stun_probe = Some(application);
        self
    }

    /// The shared tracer engine handed to inbound UI ports.
    pub fn rule_tracer(&self) -> RuleTracerApplication {
        self.rule_tracer.clone()
    }

    /// DUAL-11-06/07: the shared unpack + cache-maintenance service. The
    /// adapter result is published as the observed cache fact.
    pub fn with_rule_provider_cache(mut self, cache: Arc<dyn RuleProviderCachePort>) -> Self {
        self.rule_provider = RuleProviderApplication::new(Some(cache));
        self
    }

    /// The shared unpack service handed to inbound UI ports.
    pub fn rule_provider(&self) -> RuleProviderApplication {
        self.rule_provider.clone()
    }

    /// Observed cache location read for the rules page (never a guess).
    pub(super) async fn provider_cache(&self) -> RuleProviderCacheSnapshot {
        self.rule_provider.snapshot().await
    }

    pub fn core(&self) -> &Arc<CoreApplication> {
        &self.core
    }
}
