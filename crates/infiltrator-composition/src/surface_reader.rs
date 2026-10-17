//! Default product-composition assembler for the complete UI surface model.
//!
//! This type contains no concrete Mihomo, filesystem, or platform code. A
//! host composition supplies application facades and ports; this assembler
//! turns their results into the one contract read model consumed by Iced and
//! Bevy.

use infiltrator_application::active_exit_application::ActiveExitApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::connection_rate_application::{
    ConnectionRateApplication, connections_page_snapshot,
};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityGateFailure, CoreCapabilityLookup,
    CoreCapabilityRequirement, CoreCommandEffect,
};
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_latency_application::DnsLatencyApplication;
use infiltrator_application::dns_leak_application::DnsLeakApplication;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::dns_workbench_application::leak_report;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::mrs_acceleration_application::MrsAccelerationApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::outbound_application::OutboundApplication;
use infiltrator_application::outbound_application::preferences::OutboundUiPreferencesApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::privileged_network_application::PrivilegedNetworkApplication;
use infiltrator_application::profile_aggregation_application::last_aggregation_report;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::protocol_codec_application::studio_snapshot;
use infiltrator_application::reconnect_mask_application::ReconnectMaskApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::routing_intent_application::RoutingIntentApplication;
use infiltrator_application::rule_provider_application::RuleProviderApplication;
use infiltrator_application::rule_provider_runtime_application::RuleProviderRuntimeApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationApplication;
use infiltrator_application::runtime_control_projection::RuntimeControlApplication;
use infiltrator_application::runtime_status_application::project_runtime_status;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_application::stun_probe_application::StunProbeApplication;
use infiltrator_application::subscription_quota_application::SubscriptionQuotaApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::traffic_scale_application::TrafficScaleApplication;
use infiltrator_application::traffic_topology_application::TrafficTopologyApplication;
use infiltrator_application::traffic_waveform_application::TrafficWaveformApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilitySnapshot, CoreCapabilityStatus, CoreOperation,
    OutboundOperation,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreScope};
use infiltrator_contract::dns_cache::DnsCacheSnapshot;
use infiltrator_contract::dns_query::DnsQuerySnapshot;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::named_override::NamedOverrideCatalog;
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    SelectionAvailability,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::routing_intent::{RoutingIntentSnapshot, RoutingIntentStatus};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_ports::connection::ConnectionPort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_presentation::doctor_projection::project_observation;
use infiltrator_presentation::language_choice::project_language;
use infiltrator_presentation::log_projection::project_log_facts;
use infiltrator_presentation::routing_intent_projection;
use infiltrator_presentation::runtime_probe_settings::project_settings;
use infiltrator_presentation::shell_readout_fold::ShellReadoutProjection;
use lurker_core_contract::session::SessionToken;
use outbound_observation::{
    SurfaceReadScope, authorize_outbound_inventory, gate_outbound_observation,
    surface_read_scope_changed,
};
use page_builders::{app_routing_page, build_dns_page, build_settings_page, build_sync_page};
use projections::{merge_applied_mtu, missing, page_from_result};
use rules_page::{RulesTracerReplay, build_rules_page};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[path = "surface_reader_services.rs"]
mod surface_reader_services;

#[path = "surface_reader/composition.rs"]
mod composition;
#[path = "surface_reader/outbound_observation.rs"]
mod outbound_observation;
mod page_builders;
mod projections;
mod rules_page;
mod snapshot_views;
#[path = "surface_reader_versions.rs"]
mod versions;

/// Application facades required to expose the page read model.
#[derive(Clone)]
pub struct ProductSurfaceReader {
    scripts: Option<ScriptApplication>,
    script_exports: Option<ScriptExportApplication>,
    shell_readout: ShellReadoutProjection,
    runtime_control: RuntimeControlApplication,
    core: Arc<CoreApplication>,
    runtime_configuration_ports: Option<RuntimeConfigurationPorts>,
    connection_port: Option<Arc<dyn ConnectionPort>>,
    routing_intent_port: Option<Arc<dyn RoutingIntentPort>>,
    profiles: Option<ProfileApplication>,
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
    outbound: Option<OutboundApplication>,
    outbound_ui_preferences: Option<OutboundUiPreferencesApplication>,
    rule_provider: RuleProviderApplication,
    rule_provider_runtime: Option<RuleProviderRuntimeApplication>,
    /// DUAL-13-10/12: the shared per-connection instantaneous-rate window.
    /// Kept across reads (and cloned readers) so successive snapshots diff.
    connection_rates: ConnectionRateApplication,
    version_cache: Arc<Mutex<Option<(Instant, CoreVersionSnapshot)>>>,
    capabilities: CapabilitySnapshot,
    surface: SurfaceKind,
}

#[async_trait::async_trait]
impl SurfaceReader for ProductSurfaceReader {
    async fn read(&self) -> Result<surface_snapshot::SurfaceSnapshot, PortError> {
        let read_scope = SurfaceReadScope::from(&self.core.snapshot());
        let telemetry = self.core.read_telemetry().await;
        let core = telemetry
            .as_ref()
            .cloned()
            .unwrap_or_else(|_| self.core.snapshot());
        if SurfaceReadScope::from(&core) != read_scope
            || SurfaceReadScope::from(&self.core.snapshot()) != read_scope
        {
            return Err(surface_read_scope_changed());
        }
        let core_capability_snapshot = match core.identity.as_ref() {
            Some(identity) => self
                .core
                .core_capability_application()
                .matrix_for(&identity.engine, identity.version.as_ref())
                .map(|matrix| CoreCapabilitySnapshot::from_matrix(&matrix))
                .map(surface_snapshot::PageData::ready)
                .unwrap_or_else(surface_snapshot::PageData::unavailable),
            None => surface_snapshot::PageData::unavailable(Failure::new(
                ErrorCode::NotReady,
                "active core identity is unavailable",
                true,
            )),
        };
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
        let vpn_authorization = self.read_vpn_authorization().await;
        let privileged_network = self.read_privileged_network().await;
        let speedtest = self.core.speedtest_snapshot();
        let outbound_observation = match (&self.outbound, core.scope()) {
            (Some(outbound), Some(scope)) => {
                let capabilities = self.core.core_capability_application();
                match authorize_outbound_inventory(&capabilities, &scope) {
                    Err(failure) => Some(Err(failure)),
                    Ok(()) => Some(
                        outbound
                            .observation(scope.clone())
                            .await
                            .map_err(Failure::from)
                            .and_then(|observation| {
                                gate_outbound_observation(&capabilities, observation)
                            }),
                    ),
                }
            }
            (Some(_), None) => {
                let failure = core.failure.clone().unwrap_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "active core scope is unavailable for outbound observation",
                        true,
                    )
                });
                Some(Err(failure))
            }
            (None, _) => None,
        };
        let (outbound_inventory, outbound_details) = match outbound_observation.as_ref() {
            Some(Ok(observation)) => (
                surface_snapshot::PageData::ready(observation.inventory().clone()),
                surface_snapshot::PageData::ready(observation.details().clone()),
            ),
            Some(Err(failure)) => (
                surface_snapshot::PageData::failed(failure.clone()),
                surface_snapshot::PageData::failed(failure.clone()),
            ),
            None => {
                let failure = missing("outbound port");
                (
                    surface_snapshot::PageData::unavailable(failure.clone()),
                    surface_snapshot::PageData::unavailable(failure),
                )
            }
        };
        let outbound_ui_preferences = match (&self.outbound_ui_preferences, core.scope()) {
            (Some(preferences), Some(scope)) => match preferences.snapshot(&scope) {
                Ok(snapshot) => surface_snapshot::PageData::ready(snapshot),
                Err(failure) => surface_snapshot::PageData::failed(failure),
            },
            (Some(_), None) => {
                surface_snapshot::PageData::unavailable(core.failure.clone().unwrap_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "active core scope is unavailable for outbound preferences",
                        true,
                    )
                }))
            }
            (None, _) => surface_snapshot::PageData::unavailable(Failure::unsupported(
                "engine-scoped outbound UI preferences have not been migrated from legacy storage",
            )),
        };
        let protocol_studio = studio_snapshot()
            .map(surface_snapshot::PageData::ready)
            .unwrap_or_else(|| {
                surface_snapshot::PageData::unavailable(Failure::new(
                    ErrorCode::NotReady,
                    "protocol studio has not published an observation",
                    true,
                ))
            });
        let mut pages = surface_snapshot::SurfacePages::unavailable(missing("surface reader"));

        pages.overview =
            surface_snapshot::PageData::ready(surface_snapshot::OverviewPageSnapshot {
                routing_intent: core.routing_intent.clone(),
                upload_bps: core.upload_bps,
                download_bps: core.download_bps,
                active_connections: core.active_connections,
                core_version: core.core_version.clone(),
            });

        let runtime_config = match (&self.runtime_configuration_ports, core.scope()) {
            (Some(ports), Some(scope)) => Some(
                RuntimeConfigurationApplication::new(scope, ports.clone())
                    .observe_readout()
                    .await,
            ),
            (Some(_), None) => Some(Err(core.failure.clone().unwrap_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "active Core scope is unavailable for configuration reads",
                    true,
                )
            }))),
            (None, _) => None,
        };
        let runtime_control =
            self.runtime_control
                .observe(core.generation, revision, runtime_config.as_ref());
        let mtu = merge_applied_mtu(self.read_mtu().await, runtime_config.as_ref());
        let runtime_connections = match (&self.connection_port, core.scope()) {
            (Some(port), Some(scope)) => Some(port.observe(scope.clone()).await),
            (Some(_), None) => Some(Err(PortError::Failed(
                "active Core scope is unavailable for connection reads".to_owned(),
            ))),
            (None, _) => None,
        };
        let traffic_topology = self.traffic_topology.project(
            &core,
            runtime_config.as_ref(),
            outbound_observation.as_ref(),
            runtime_connections.as_ref(),
        );
        let runtime_status = project_runtime_status(
            self.capabilities.host,
            surface_snapshot::SurfaceOrigin::Live,
            &core,
            telemetry.as_ref(),
            &vpn_authorization,
            &vpn,
            &traffic_topology,
        );
        let active_exit_snapshot = self
            .active_exit
            .project(&core, outbound_observation.as_ref());
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
        let runtime_rule_providers = match (&self.rule_provider_runtime, core.scope()) {
            (Some(application), Some(scope)) => Some(application.inventory(&scope).await),
            (Some(_), None) => Some(Err(core.failure.clone().unwrap_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "active Core scope is unavailable for rule-provider reads",
                    true,
                )
            }))),
            (None, _) => None,
        };

        let hydrated_settings = match &self.settings {
            Some(settings) => Some(settings.load_hydrated().await),
            None => None,
        };
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
            let named_overrides = match &self.profiles {
                Some(profiles) => profiles
                    .load_named_override_catalog()
                    .await
                    .unwrap_or_default(),
                None => NamedOverrideCatalog::default(),
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
                        named_overrides,
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
                            Some(profiles) => match ProfileId::new(item.name.clone()) {
                                Ok(profile_id) => {
                                    match profiles.load_workspace(&profile_id).await {
                                        Ok(workspace) => match profiles.inspect_profile_options(
                                            &workspace.document.identity,
                                            workspace.sidecar_bytes.as_deref(),
                                        ) {
                                            Ok(options) => {
                                                (options.filter, Ok(workspace.identity()))
                                            }
                                            Err(error) => (Default::default(), Err(error)),
                                        },
                                        Err(failure) => (Default::default(), Err(failure)),
                                    }
                                }
                                Err(error) => (
                                    Default::default(),
                                    Err(Failure::new(
                                        ErrorCode::InvalidInput,
                                        error.to_string(),
                                        false,
                                    )),
                                ),
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
                        named_overrides,
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
                    .observe_at(Instant::now(), &connections.snapshot().connections);
                connections_page_snapshot(connections.snapshot(), &rates)
            },
            "Mihomo connection port",
        );

        let mrs_acceleration_snapshot = if let Some(Ok(inventory)) = &runtime_rule_providers {
            MrsAccelerationApplication::new().project(&core, Some(inventory.providers()))
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

        let log_facts = self.core.log_application().facts();
        pages.logs = project_log_facts(&core, &log_facts);

        if let Some(doctor) = &self.doctor {
            pages.doctor = project_observation(&doctor.observation(), &doctor.list_checks());
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
            runtime_status,
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
            core_capability_snapshot,
            failure: None,
            pages,
            protocol_studio,
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
            closed_connections: Default::default(),
            traffic_history: Default::default(),
            active_exit: active_exit_snapshot,
            outbound_inventory,
            outbound_details,
            outbound_ui_preferences,
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
        let routing_intent_scope = snapshot.core.scope();
        let observed_routing_intent = match &self.routing_intent_port {
            Some(port) => {
                RoutingIntentApplication::new(port.clone(), self.core.core_capability_application())
                    .observe(routing_intent_scope.as_ref())
                    .await
            }
            None => RoutingIntentSnapshot {
                current: None,
                status: RoutingIntentStatus::Unsupported,
                failure: Some(Failure::unsupported(
                    "routing intent adapter is not composed for this surface",
                )),
            },
        };
        snapshot.core.routing_intent = observed_routing_intent;
        let routing_intent = routing_intent_projection::from_surface(&snapshot);
        snapshot.core.routing_intent = routing_intent.clone();
        if let Some(overview) = &mut snapshot.pages.overview.data {
            overview.routing_intent = routing_intent;
        }
        if SurfaceReadScope::from(&self.core.snapshot()) != read_scope {
            return Err(surface_read_scope_changed());
        }
        snapshot.shell_readout.rate_failure = telemetry.err();
        snapshot.shell_readout = self.shell_readout.project(&snapshot);
        Ok(snapshot)
    }
}

#[cfg(test)]
#[path = "surface_reader_test_support.rs"]
mod test_support;
#[cfg(test)]
use self::test_support::{install_lifecycle_capabilities, test_execute_result_at_current_scope};

#[cfg(test)]
#[path = "surface_reader_test.rs"]
mod tests;

#[cfg(test)]
#[path = "surface_reader_script_tests.rs"]
mod script_tests;

#[cfg(test)]
#[path = "surface_reader_profile_tests.rs"]
mod profile_tests;
