//! Default application-level assembler for the complete UI surface model.
//!
//! This type contains no concrete Mihomo, filesystem, or platform code. A
//! host composition supplies application facades and ports; this assembler
//! turns their results into the one contract read model consumed by Iced and
//! Bevy.

use crate::active_exit_application::ActiveExitApplication;
use crate::configuration_application::ConfigurationApplication;
use crate::connection_rate_application::{ConnectionRateApplication, connections_page_snapshot};
use crate::core_application::CoreApplication;
use crate::dns_cache_application::DnsCacheApplication;
use crate::dns_latency_application::DnsLatencyApplication;
use crate::dns_leak_application::DnsLeakApplication;
use crate::dns_query_application::DnsQueryApplication;
use crate::dns_workbench_application::leak_report;
use crate::doctor_application::DoctorApplication;
use crate::language_choice::project_language;
use crate::mrs_acceleration_application::MrsAccelerationApplication;
use crate::mtu_application::MtuApplication;
use crate::network_roaming_application::NetworkRoamingApplication;
use crate::offline_startup_application::OfflineStartupApplication;
use crate::pac_application::PacApplication;
use crate::port_conflict_application::PortConflictApplication;
use crate::privileged_network_application::PrivilegedNetworkApplication;
use crate::profile_aggregation_application::last_aggregation_report;
use crate::profile_application::ProfileApplication;
use crate::protocol_codec_application::studio_snapshot;
use crate::proxy_inspection_projection::{inspection_catalogue, with_egress_record};
use crate::proxy_preferences_application::ProxyPreferencesApplication;
use crate::proxy_probe_options_projection::project_settings;
use crate::proxy_projection::project_groups_snapshot;
use crate::proxy_search_projection::project_name_runs;
use crate::reconnect_mask_application::ReconnectMaskApplication;
use crate::resource_application::ResourceApplication;
use crate::routing_application::RoutingApplication;
use crate::rule_provider_application::RuleProviderApplication;
use crate::rule_tracer_application::RuleTracerApplication;
use crate::runtime_control_projection::RuntimeControlApplication;
use crate::script_application::ScriptApplication;
use crate::script_export_application::ScriptExportApplication;
use crate::service_mode_application::ServiceModeApplication;
use crate::settings_application::SettingsApplication;
use crate::shell_readout_application::ShellReadoutApplication;
use crate::snapshot_application::SnapshotApplication;
use crate::speedtest_application::SpeedtestApplication;
use crate::stun_probe_application::StunProbeApplication;
use crate::subscription_quota_application::SubscriptionQuotaApplication;
use crate::system_proxy_application::SystemProxyApplication;
use crate::traffic_scale_application::TrafficScaleApplication;
use crate::traffic_topology_application::TrafficTopologyApplication;
use crate::traffic_waveform_application::TrafficWaveformApplication;
use crate::uwp_loopback_application::UwpLoopbackApplication;
use crate::version_application::VersionApplication;
use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::dns_cache::DnsCacheSnapshot;
use infiltrator_contract::dns_query::DnsQuerySnapshot;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot;
use infiltrator_contract::version::{CoreChannelStatus, CoreVersionSnapshot};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::filter_policy_form::filter_spec_to_draft;
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::surface::SurfaceReader;
use page_builders::{app_routing_page, build_dns_page, build_settings_page, build_sync_page};
use projections::{active_exit, merge_applied_mtu, missing, page_from_result};
use rules_page::{RulesTracerReplay, build_rules_page};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "surface_reader_services.rs"]
mod surface_reader_services;

mod page_builders;
mod projections;
mod rules_page;
mod snapshot_views;

/// Application facades required to expose the page read model.
#[derive(Clone)]
pub struct ApplicationSurfaceReader {
    scripts: Option<ScriptApplication>,
    script_exports: Option<ScriptExportApplication>,
    shell_readout: ShellReadoutApplication,
    runtime_control: RuntimeControlApplication,
    core: Arc<CoreApplication>,
    gateway: Option<Arc<dyn RuntimeGateway>>,
    profiles: Option<ProfileApplication>,
    proxy_preferences: Option<ProxyPreferencesApplication>,
    configuration: Option<ConfigurationApplication>,
    doctor: Option<DoctorApplication>,
    read_revision: Arc<AtomicU64>,
    routing: Option<RoutingApplication>,
    settings: Option<SettingsApplication>,
    snapshots: Option<SnapshotApplication>,
    versions: Option<VersionApplication>,
    endpoint_source: Option<Arc<dyn EndpointSource>>,
    service_mode: Option<ServiceModeApplication>,
    port_conflicts: Option<PortConflictApplication>,
    resources: Option<ResourceApplication>,
    offline_startup: Option<OfflineStartupApplication>,
    mtu: Option<MtuApplication>,
    system_proxy: Option<SystemProxyApplication>,
    uwp_loopback: Option<UwpLoopbackApplication>,
    pac: Option<PacApplication>,
    network_roaming: Option<NetworkRoamingApplication>,
    vpn: Option<VpnServiceApplication>,
    privileged_network: Option<PrivilegedNetworkApplication>,
    traffic_waveform: TrafficWaveformApplication,
    traffic_scale: TrafficScaleApplication,
    traffic_topology: TrafficTopologyApplication,
    active_exit: ActiveExitApplication,
    rule_tracer: RuleTracerApplication,
    subscription_quota: SubscriptionQuotaApplication,
    speedtest: Option<SpeedtestApplication>,
    dns_cache: Option<DnsCacheApplication>,
    dns_query: Option<DnsQueryApplication>,
    /// DUAL-14-10/13: the shared latency prober whose last report drives both
    /// the latency row and the self-heal snapshot.
    dns_latency: Option<DnsLatencyApplication>,
    /// DUAL-14-08: the shared cross-source leak prober whose last report the
    /// DNS page publishes to both surfaces.
    dns_leak: Option<DnsLeakApplication>,
    /// DUAL-14-09 (re-scoped): the shared STUN UDP-egress prober whose last
    /// report the DNS privacy area publishes to both surfaces.
    stun_probe: Option<StunProbeApplication>,
    rule_provider: RuleProviderApplication,
    /// DUAL-13-10/12: the shared per-connection instantaneous-rate window.
    /// Kept across reads (and cloned readers) so successive snapshots diff.
    connection_rates: ConnectionRateApplication,
    version_cache: Arc<Mutex<Option<(Instant, CoreVersionSnapshot)>>>,
    capabilities: CapabilitySnapshot,
    surface: SurfaceKind,
}

impl ApplicationSurfaceReader {
    pub fn new(core: Arc<CoreApplication>, surface: SurfaceKind, host: HostKind) -> Self {
        Self {
            scripts: None,
            script_exports: None,
            shell_readout: ShellReadoutApplication::default(),
            runtime_control: RuntimeControlApplication::default(),
            core,
            gateway: None,
            profiles: None,
            proxy_preferences: None,
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
            speedtest: None,
            dns_cache: None,
            dns_query: None,
            dns_latency: None,
            dns_leak: None,
            stun_probe: None,
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
        self.gateway = Some(gateway);
        self
    }

    pub fn with_profiles(mut self, profiles: ProfileApplication) -> Self {
        self.profiles = Some(profiles);
        self
    }

    pub fn with_proxy_preferences(mut self, preferences: ProxyPreferencesApplication) -> Self {
        self.proxy_preferences = Some(preferences);
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

    /// Attach the shared speedtest/jitter engine so its snapshot is published
    /// to both surfaces. Without this the field would fall back to `Default`.
    pub fn with_speedtest(mut self, application: SpeedtestApplication) -> Self {
        self.speedtest = Some(application);
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

    async fn read_versions(&self) -> CoreVersionSnapshot {
        let Some(versions) = &self.versions else {
            return CoreVersionSnapshot::default();
        };
        if let Some((probed_at, snapshot)) = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .cloned()
        {
            let has_ready_channel = snapshot
                .channels
                .iter()
                .any(|channel| matches!(channel.status, CoreChannelStatus::Ready { .. }));
            let ttl = if has_ready_channel {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(30)
            };
            if probed_at.elapsed() < ttl {
                return snapshot;
            }
        }

        let next_revision = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .map_or(1, |(_, snapshot)| snapshot.revision.saturating_add(1));
        let mut snapshot = versions.probe_channels().await;
        snapshot.revision = next_revision;
        *self.version_cache.lock().expect("version cache lock") =
            Some((Instant::now(), snapshot.clone()));
        snapshot
    }
}

#[async_trait::async_trait]
impl SurfaceReader for ApplicationSurfaceReader {
    async fn read(&self) -> Result<surface_snapshot::SurfaceSnapshot, PortError> {
        let telemetry = self.core.read_telemetry().await;
        let core = telemetry
            .as_ref()
            .cloned()
            .unwrap_or_else(|_| self.core.snapshot());
        // Controller-independent pages also change while the core is idle.
        let revision = self
            .read_revision
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |previous| {
                previous.checked_add(1).map(|next| next.max(core.revision))
            })
            .map_err(|_| PortError::Failed("surface revision exhausted".into()))?
            + 1;
        let revision = revision.max(core.revision);
        let traffic_waveform = self.traffic_waveform.record(&core);
        let traffic_scale = self.traffic_scale.compute(&traffic_waveform);
        let versions = self.read_versions().await;
        let controller_auth = self.read_controller_auth().await;
        let service_mode = self.read_service_mode().await;
        let port_conflicts = self.read_port_conflicts().await;
        let resources = self.read_resources().await;
        let offline_startup = self.read_offline_startup().await;
        let system_proxy = self.read_system_proxy().await;
        let system_proxy_recovery = self.read_system_proxy_recovery();
        let pac = self.read_pac().await;
        let network_roaming = self.read_network_roaming().await;
        let vpn = self.read_vpn().await;
        let privileged_network = self.read_privileged_network().await;
        let mut pages = surface_snapshot::SurfacePages::unavailable(missing("surface reader"));

        pages.overview =
            surface_snapshot::PageData::ready(surface_snapshot::OverviewPageSnapshot {
                proxy_mode: core.proxy_mode,
                upload_bps: core.upload_bps,
                download_bps: core.download_bps,
                active_connections: core.active_connections,
                memory_bytes: core.memory_bytes,
                core_version: core.core_version.clone(),
            });

        let runtime_config = match &self.gateway {
            Some(gateway) => Some(gateway.get_config().await),
            None => None,
        };
        let runtime_control =
            self.runtime_control
                .observe(core.generation, revision, runtime_config.as_ref());
        let mtu = merge_applied_mtu(self.read_mtu().await, runtime_config.as_ref());
        let runtime_proxies = match &self.gateway {
            Some(gateway) => Some(gateway.get_proxies().await),
            None => None,
        };
        let runtime_connections = match &self.gateway {
            Some(gateway) => Some(gateway.get_connections().await),
            None => None,
        };
        let traffic_topology = self.traffic_topology.project(
            &core,
            runtime_config.as_ref(),
            runtime_proxies.as_ref(),
            runtime_connections.as_ref(),
        );
        let active_exit_snapshot = self.active_exit.project(&core, runtime_proxies.as_ref());
        let quota_read = self.subscription_quota.begin();
        let current_before = match &self.profiles {
            Some(profiles) => Some(profiles.active_quota_identity().await),
            None => None,
        };
        let profile_result = match &self.profiles {
            Some(profiles) => Some(profiles.list_profiles().await),
            None => None,
        };
        let mut current_after = match &self.profiles {
            Some(profiles) => Some(profiles.active_quota_identity().await),
            None => None,
        };
        if current_before.as_ref().is_some_and(|result| result.is_ok())
            && current_after.as_ref().is_some_and(|result| result.is_ok())
            && current_before
                .as_ref()
                .and_then(|result| result.as_ref().ok())
                != current_after
                    .as_ref()
                    .and_then(|result| result.as_ref().ok())
        {
            current_after = Some(Err(Failure::new(
                ErrorCode::NotReady,
                "Active profile changed during the quota read",
                true,
            )
            .with_reason(FailureReason::QuotaSourceChanged)));
        }
        if let Some(Err(failure)) = current_before.as_ref() {
            current_after = Some(Err(failure.clone()));
        }
        let subscription_quota = self.subscription_quota.project(
            &quota_read,
            &core,
            current_after.as_ref(),
            profile_result.as_ref(),
        );
        let runtime_rule_providers = match &self.gateway {
            Some(gateway) => Some(gateway.get_rule_providers().await),
            None => None,
        };

        let speedtest = self
            .speedtest
            .as_ref()
            .map(|application| application.snapshot())
            .unwrap_or_default();
        let hydrated_settings = match &self.settings {
            Some(settings) => Some(settings.load_hydrated().await),
            None => None,
        };
        let proxy_preferences = self
            .proxy_preferences
            .as_ref()
            .map(ProxyPreferencesApplication::preferences)
            .transpose()
            .map(|preferences| preferences.unwrap_or_default());
        let proxy_page_input = runtime_proxies.map(|result| {
            result.and_then(|proxies| {
                proxy_preferences
                    .map(|preferences| (proxies, preferences))
                    .map_err(PortError::Rejected)
            })
        });
        pages.proxies = page_from_result(
            proxy_page_input,
            |(proxies, preferences)| {
                let (groups, filter_alive) = project_groups_snapshot(&proxies, &preferences);
                let mut node_details = inspection_catalogue(&proxies);
                for detail in &mut node_details {
                    with_egress_record(detail, &speedtest);
                }
                surface_snapshot::ProxiesPageSnapshot {
                    name_runs: project_name_runs(&groups, &preferences.search_query),
                    search_query: preferences.search_query.clone(),
                    node_details,
                    groups,
                    testing: false,
                    active_exit: active_exit(&proxies),
                    filter_alive,
                    sort_order: preferences.sort_order,
                    compact_view: preferences.compact_view,
                    // DUAL-05: the shared protocol studio, published by
                    // `ProtocolCodecApplication` and read here. A host without a
                    // profile store still gets the typed draft + report, with
                    // `can_persist = false`.
                    custom_node: studio_snapshot().unwrap_or_default(),
                }
            },
            "Mihomo proxy gateway",
        );

        let profile_editor = match &self.profiles {
            Some(profiles) => profiles.verified_editor_snapshot().await,
            None => Default::default(),
        };
        let apply_transaction = profile_result
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .and_then(|items| items.iter().find(|profile| profile.active))
            .and_then(|profile| {
                self.profiles
                    .as_ref()
                    .and_then(|profiles| profiles.apply_transaction(&profile.name))
            });
        let snapshot_views = self.read_snapshot_views().await;
        let (snapshot_history, yaml_ast_diff) =
            snapshot_views.as_ref().cloned().unwrap_or_default();
        if self.profiles.is_some() {
            // DUAL-08-13: the template library sidecar. A store without one
            // answers with a typed unsupported, published as `available =
            // false` rather than as a fake empty library.
            let (aggregation_templates, aggregation_templates_available) = match &self.profiles {
                Some(profiles) => match profiles.load_aggregation_templates().await {
                    Ok(templates) => (templates, true),
                    Err(_) => (Vec::new(), false),
                },
                None => (Vec::new(), false),
            };
            pages.profiles = match profile_result {
                Some(Ok(items)) if items.is_empty() => {
                    surface_snapshot::PageData::empty(surface_snapshot::ProfilesPageSnapshot {
                        profiles: Vec::new(),
                        auto_update_interval_hours: 0,
                        updating: false,
                        aggregation: last_aggregation_report(),
                        aggregation_templates,
                        aggregation_templates_available,
                        snapshot_history: snapshot_history.clone(),
                        apply_transaction: apply_transaction.clone(),
                    })
                }
                Some(Ok(items)) => {
                    let mut snapshots = Vec::with_capacity(items.len());
                    // DUAL-07-14: the page-level interval is the shortest fixed
                    // interval actually scheduled; cron-only profiles are not
                    // counted, so the header can stay honest.
                    let auto_update_interval_hours = items
                        .iter()
                        .filter(|item| item.auto_update_enabled)
                        .filter_map(|item| item.update_interval_hours)
                        .min()
                        .unwrap_or(0);
                    for item in items {
                        // DUAL-07-08: surface the stored node-keyword filter so
                        // both surfaces can prefill the editor.
                        let (filter, filter_source) = match &self.profiles {
                            Some(profiles) => match profiles.load_workspace(&item.name).await {
                                Ok(workspace) => match workspace
                                    .options
                                    .filter
                                    .as_ref()
                                    .map(filter_spec_to_draft)
                                    .transpose()
                                {
                                    Ok(filter) => {
                                        (filter.unwrap_or_default(), Ok(workspace.source))
                                    }
                                    Err(error) => (
                                        Default::default(),
                                        Err(Failure::new(
                                            ErrorCode::Configuration,
                                            error.to_string(),
                                            false,
                                        )),
                                    ),
                                },
                                Err(failure) => (Default::default(), Err(failure)),
                            },
                            None => (
                                Default::default(),
                                Err(Failure::unsupported(
                                    "profile workspace reader is unavailable",
                                )),
                            ),
                        };
                        // DUAL-09-12: the same classification the write guard
                        // enforces, derived from the real subscription source.
                        let write_protection = ProfileWriteProtection::from_subscription_url(
                            item.subscription_url.as_deref().unwrap_or_default(),
                        );
                        snapshots.push(surface_snapshot::ProfileSnapshot {
                            id: item.name.clone(),
                            name: item.name,
                            url: item.subscription_url.unwrap_or_default(),
                            updated_at: item
                                .last_updated
                                .map(|value| value.to_rfc3339())
                                .unwrap_or_default(),
                            upload_bytes: item.traffic_upload,
                            download_bytes: item.traffic_download,
                            total_bytes: item.traffic_total,
                            is_active: item.active,
                            user_agent: item.user_agent.unwrap_or_default(),
                            insecure_skip_verify: item.insecure_skip_verify,
                            etag: item.etag,
                            last_modified: item.last_modified,
                            has_backup: item.has_backup,
                            cron_expression: item.cron_expression,
                            auto_update_enabled: item.auto_update_enabled,
                            update_interval_hours: item.update_interval_hours,
                            next_update: item.next_update.map(|value| value.to_rfc3339()),
                            auto_reload_core: item.auto_reload_core,
                            filter,
                            filter_source,
                            write_protection,
                        });
                    }
                    surface_snapshot::PageData::ready(surface_snapshot::ProfilesPageSnapshot {
                        profiles: snapshots,
                        auto_update_interval_hours,
                        updating: false,
                        aggregation: last_aggregation_report(),
                        aggregation_templates,
                        aggregation_templates_available,
                        // DUAL-09-06/11/14: the shared history, the host apply
                        // transaction and loaded editor document are separate observations.
                        // History comes from this product instance and active profile.
                        snapshot_history: snapshot_history.clone(),
                        apply_transaction: apply_transaction.clone(),
                    })
                }
                Some(Err(failure)) => surface_snapshot::PageData::failed(failure),
                None => surface_snapshot::PageData::unavailable(missing("profile application")),
            };
        }

        if let Err(failure) = snapshot_views {
            pages.profiles.status = surface_snapshot::PageStatus::Failed { failure };
        }

        let dns_hosts = match self.configuration.as_ref() {
            Some(configuration) => match configuration.load_hosts_profile().await {
                Ok(profile) => surface_snapshot::PageData::ready(profile),
                Err(failure) => surface_snapshot::PageData::failed(failure),
            },
            None => surface_snapshot::unobserved_hosts(),
        };
        pages.dns = build_dns_page(
            self.configuration.as_ref(),
            runtime_config.as_ref(),
            self.dns_latency.as_ref(),
            self.stun_probe.as_ref(),
            runtime_connections.as_ref(),
            &port_conflicts,
        )
        .await;

        pages.connections = page_from_result(
            runtime_connections,
            |connections| {
                // DUAL-13-10/12: the runtime DTO carries cumulative totals
                // only; the shared rate window derives the instantaneous
                // bytes-per-second from this observation and the previous one
                // and the shared mapper publishes them into the read model.
                let rates = self
                    .connection_rates
                    .observe_at(Instant::now(), &connections.connections);
                connections_page_snapshot(&connections, &rates)
            },
            "Mihomo connections gateway",
        );

        let mrs_acceleration_snapshot = if let Some(Ok(provs)) = &runtime_rule_providers {
            MrsAccelerationApplication::new().project(&core, Some(provs.as_slice()))
        } else {
            MrsAccelerationSnapshot::empty(core.generation, revision)
        };

        // DUAL-11-07: the observed provider-cache fact is read once per
        // revision so both surfaces render the same count/size.
        let provider_cache_snapshot = self.provider_cache().await;

        // Resolve the tracer inputs once so the shared engine replays the same
        // rule list the rules page renders. Without runtime proxy facts the
        // outbound stage stays honestly unknown (no fabricated node data).
        pages.rules = build_rules_page(
            self.configuration.as_ref(),
            &self.rule_provider,
            runtime_rule_providers,
            RulesTracerReplay {
                application: &self.rule_tracer,
            },
            mrs_acceleration_snapshot,
            provider_cache_snapshot,
        )
        .await;

        pages.logs = self.core.log_application().page(&core);

        if let Some(doctor) = &self.doctor {
            pages.doctor = doctor.page();
        }

        let uwp_loopback = self.read_uwp_loopback().await;
        if let Some(routing) = &self.routing {
            pages.app_routing = match routing.load() {
                Ok(config) => surface_snapshot::PageData::ready(app_routing_page(
                    config,
                    uwp_loopback.clone(),
                )),
                Err(failure) => surface_snapshot::PageData::failed(failure),
            };
        }

        pages.sync = build_sync_page(
            hydrated_settings.as_ref(),
            self.profiles.as_ref(),
            self.snapshots.as_ref(),
        )
        .await;
        let reconnect_mask = ReconnectMaskApplication.project(&core);
        pages.settings = build_settings_page(
            hydrated_settings.as_ref(),
            &runtime_control,
            &system_proxy,
            pac,
        );

        let mut snapshot = surface_snapshot::SurfaceSnapshot {
            profile_editor,
            runtime_control,
            shell_readout: Default::default(),
            probe_settings: project_settings(hydrated_settings.as_ref()),
            language_settings: project_language(hydrated_settings.as_ref()),
            dns_leak: leak_report(self.dns_leak.as_ref()),
            rule_trace: self.rule_tracer.execution(),
            dns_query: self
                .dns_query
                .as_ref()
                .map(DnsQueryApplication::snapshot)
                .unwrap_or_else(DnsQuerySnapshot::unavailable),
            dns_cache: self
                .dns_cache
                .as_ref()
                .map(DnsCacheApplication::snapshot)
                .unwrap_or_else(DnsCacheSnapshot::unavailable),
            dns_hosts,
            surface: self.surface,
            origin: surface_snapshot::SurfaceOrigin::Live,
            generation: core.generation,
            revision,
            core,
            capabilities: self.capabilities.clone(),
            failure: None,
            pages,
            versions,
            controller_auth,
            service_mode,
            port_conflicts,
            resources,
            offline_startup,
            mtu,
            system_proxy,
            system_proxy_recovery,
            network_roaming,
            vpn,
            privileged_network,
            traffic_waveform,
            traffic_scale,
            traffic_topology,
            active_exit: active_exit_snapshot,
            public_ip: PublicIpProbeSnapshot::default(),
            overview_layout: Default::default(),
            reconnect_mask,
            viewport: Default::default(),
            subscription_quota,
            yaml_ast_diff,
            script_sandbox: self
                .scripts
                .as_ref()
                .and_then(|scripts| scripts.observation().result.map(|result| result.snapshot)),
            script_export: self
                .script_exports
                .as_ref()
                .and_then(ScriptExportApplication::snapshot),
            speedtest,
        };
        snapshot.shell_readout.rate_failure = telemetry.err();
        snapshot.shell_readout = self.shell_readout.project(&snapshot);
        Ok(snapshot)
    }
}

#[cfg(test)]
#[path = "surface_reader_test.rs"]
mod tests;
