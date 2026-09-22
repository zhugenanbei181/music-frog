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

    /// Install the validated profile configuration application so surfaces can
    /// submit their shared DNS workbench edits through the same write path.
    pub fn with_configuration(
        mut self,
        configuration: crate::configuration_application::ConfigurationApplication,
    ) -> Self {
        self.configuration = Some(configuration);
        self
    }

    pub async fn execute(&self, intent: CommandIntent) -> Result<(), Failure> {
        match intent {
            CommandIntent::SwitchProfile { profile_id } => {
                let profile = self.profile()?;
                if let Some(runtime) = self.managed_runtime.clone() {
                    profile
                        .activate_profile(Some(runtime), &profile_id)
                        .await
                        .map(|_| ())
                } else {
                    profile.select_profile(&profile_id).await.map(|_| ())
                }
            }
            CommandIntent::SelectProxyNode { group, node } => self
                .runtime()?
                .switch_proxy(&group, &node)
                .await
                .map_err(Failure::from),
            CommandIntent::ToggleProxyGroupExpand { group } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.toggle_group_expand(&group);
                }
                Ok(())
            }
            CommandIntent::SetProxyGroupExpanded { group, expanded } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_group_expanded(&group, expanded);
                }
                Ok(())
            }
            CommandIntent::SetProxySortOrder { order } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_sort_order(order);
                }
                Ok(())
            }
            CommandIntent::ToggleFilterAlive { enabled } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_filter_alive(enabled);
                }
                Ok(())
            }
            CommandIntent::ToggleFavoriteProxy { proxy } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.toggle_favorite(&proxy);
                }
                Ok(())
            }
            CommandIntent::SetProxyCompactView { compact } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.set_compact_view(compact);
                }
                Ok(())
            }
            CommandIntent::ReorderProxyGroups { group_names } => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.reorder_groups(group_names);
                }
                Ok(())
            }
            CommandIntent::ResetProxyGroupOrder => {
                if let Some(prefs) = &self.proxy_preferences {
                    prefs.reset_group_order();
                }
                Ok(())
            }
            CommandIntent::TestDelay {
                group,
                url,
                timeout_ms,
            } => {
                if let Ok(speedtest) = self.speedtest() {
                    let scope = match group {
                        Some(g) => infiltrator_contract::speedtest::SpeedtestScope::SingleGroup(g),
                        None => infiltrator_contract::speedtest::SpeedtestScope::AllGroups,
                    };
                    speedtest.test_delays(scope, url, timeout_ms).await?;
                    return Ok(());
                }
                let runtime = self.runtime()?;
                let proxies = runtime.get_proxies().await.map_err(Failure::from)?;
                let candidates = delay_candidates(&proxies, group.as_deref())?;
                let test_url = url.unwrap_or_else(|| DEFAULT_DELAY_TEST_URL.to_string());
                let timeout = timeout_ms.unwrap_or(DEFAULT_DELAY_TIMEOUT_MS);
                let _ = crate::proxy_application::test_proxy_delays(
                    runtime,
                    candidates,
                    test_url,
                    timeout,
                    DEFAULT_DELAY_CONCURRENCY,
                )
                .await;
                Ok(())
            }
            CommandIntent::RunSpeedtest { node, url } => {
                let speedtest = self.speedtest()?;
                speedtest.probe_node_jitter(&node, 5, url, None).await?;
                Ok(())
            }
            CommandIntent::RecordSpeedtestBandwidth {
                node,
                total_bytes,
                duration_ms,
            } => {
                let speedtest = self.speedtest()?;
                speedtest.record_bandwidth(&node, total_bytes, duration_ms)?;
                Ok(())
            }
            CommandIntent::RecordSpeedtestOutboundIp { node, ip, country } => {
                let speedtest = self.speedtest()?;
                speedtest.record_outbound_ip(&node, &ip, country.as_deref())?;
                Ok(())
            }
            CommandIntent::SetSpeedtestConcurrency { limit } => {
                let speedtest = self.speedtest()?;
                speedtest.set_concurrency(limit);
                Ok(())
            }
            CommandIntent::CancelSpeedtest => {
                let speedtest = self.speedtest()?;
                speedtest.cancel();
                Ok(())
            }
            CommandIntent::UpdateProfile { profile_id } => {
                let profile = self.profile()?;
                let source = self.subscription_source()?;
                if let Some(runtime) = self.application_runtime.clone() {
                    self.refresh_application(profile, runtime)
                        .refresh_profile(source.as_ref(), &profile_id)
                        .await
                        .map(|_| ())
                } else {
                    profile
                        .update_subscription(source.as_ref(), &profile_id)
                        .await
                        .map(|_| ())
                }
            }
            CommandIntent::UpdateAllSubscriptions => {
                let profile = self.profile()?;
                let source = self.subscription_source()?;
                if let Some(runtime) = self.application_runtime.clone() {
                    self.refresh_application(profile, runtime)
                        .refresh_all(source.as_ref(), BATCH_UPDATE_CONCURRENCY)
                        .await
                        .map(|_| ())
                } else {
                    profile
                        .update_all_subscriptions(source.as_ref(), BATCH_UPDATE_CONCURRENCY)
                        .await
                        .map(|_| ())
                }
            }
            CommandIntent::SaveSubscriptionFilter { profile_id, filter } => {
                let profile = self.profile()?;
                let spec = infiltrator_domain::profile_options::filter_spec_from_draft(&filter)
                    .map_err(|error| {
                        Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
                    })?;
                profile
                    .apply_subscription_filter(self.managed_runtime.clone(), &profile_id, spec)
                    .await
                    .map(|_| ())
            }
            CommandIntent::ImportSubscription {
                profile_id,
                channel,
                source,
            } => {
                let port = self.import_source.clone().ok_or_else(|| {
                    Failure::unsupported(
                        "local file / clipboard import is not configured for this host",
                    )
                })?;
                let application =
                    crate::subscription_import_application::SubscriptionImportApplication::new(
                        self.profile()?,
                        port,
                    );
                let subscription_source = self.subscription_source()?;
                application
                    .import(subscription_source.as_ref(), &profile_id, channel, &source)
                    .await
                    .map(|_| ())
            }
            CommandIntent::RestoreSubscriptionBackup { profile_id } => self
                .profile()?
                .restore_backup(&profile_id)
                .await
                .map(|_| ()),
            CommandIntent::UpdateSubscriptionFetchSettings {
                profile_id,
                user_agent,
                insecure_skip_verify,
            } => {
                let profile = self.profile()?;
                profile
                    .update_subscription_fetch_settings(
                        &profile_id,
                        user_agent,
                        insecure_skip_verify,
                    )
                    .await
            }
            CommandIntent::DeleteProfile { profile_id } => {
                self.profile()?.delete_profile(&profile_id).await
            }
            CommandIntent::PreviewProfileAggregation { draft } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.preview(&draft).await.map(|_| ())
            }
            CommandIntent::CreateAggregatedProfile { draft } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application
                    .create_profile_with_runtime(self.managed_runtime.clone(), &draft)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SaveAggregationTemplate { name, draft } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.save_template(&name, &draft).await.map(|_| ())
            }
            CommandIntent::DeleteAggregationTemplate { name } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application.delete_template(&name).await.map(|_| ())
            }
            CommandIntent::ReAggregateProfile { template_name } => {
                let application =
                    crate::profile_aggregation_application::ProfileAggregationApplication::new(
                        self.profile()?,
                    );
                application
                    .reaggregate(self.managed_runtime.clone(), &template_name)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SetSubscriptionAutoReload {
                profile_id,
                enabled,
            } => {
                // DUAL-07-09: a host without a managed-runtime reload seam must
                // reject the enable action with a typed failure instead of
                // persisting a preference that can only silently no-op.
                if enabled && self.managed_runtime.is_none() {
                    return Err(Failure::unsupported(
                        "this host exposes no managed core-reload seam, so auto reload cannot be enabled",
                    ));
                }
                self.profile()?
                    .update_subscription_auto_reload(&profile_id, enabled)
                    .await
            }
            CommandIntent::UpdateSubscriptionSchedule { profile_id, draft } => {
                self.profile()?
                    .update_subscription_schedule(&profile_id, &draft)
                    .await
            }
            CommandIntent::RefreshRuleProviders => {
                let runtime = self.runtime()?;
                let providers = runtime.get_rule_providers().await.map_err(Failure::from)?;
                for provider in providers {
                    runtime
                        .update_rule_provider(&provider.name)
                        .await
                        .map_err(Failure::from)?;
                }
                Ok(())
            }
            CommandIntent::CloseConnection { id } => self
                .runtime()?
                .close_connection(&id)
                .await
                .map_err(Failure::from),
            CommandIntent::CloseAllConnections => self
                .runtime()?
                .close_all_connections()
                .await
                .map_err(Failure::from),
            CommandIntent::ClearDnsCache => match self.dns_cache.as_ref() {
                Some(dns_cache) => dns_cache.flush_all().await.map(|_| ()),
                None => self
                    .runtime()?
                    .flush_fakeip_cache()
                    .await
                    .map_err(Failure::from),
            },
            CommandIntent::ApplyDnsSettings { patch } => self
                .configuration()?
                .apply_dns_settings_with_runtime(self.managed_runtime.clone(), patch)
                .await
                .map(|_| ()),
            CommandIntent::RunDoctorDiagnostics => self.doctor()?.run(None).await.map(|_| ()),
            CommandIntent::RepairDoctorIssue { check_id } => {
                self.doctor()?.fix(Some(check_id)).await.map(|_| ())
            }
            CommandIntent::RepairAllDoctorIssues => self.doctor()?.fix(None).await.map(|_| ()),
            CommandIntent::ToggleAppRouting { app_id, enabled } => {
                self.routing()?.set_package_enabled(&app_id, enabled)
            }
            CommandIntent::SetAppRoutingMode { mode } => {
                let mode = parse_routing_mode(&mode)?;
                self.routing()?.set_mode(mode)
            }
            CommandIntent::SetAppRule { app_id, rule } => {
                let rule = parse_routing_rule(&rule)?;
                self.routing()?.set_rule(&app_id, rule)
            }
            CommandIntent::SyncNow => {
                let settings = self.settings()?.load_hydrated().await?;
                self.sync()?
                    .sync(settings.webdav, settings.configs_dir)
                    .await
                    .map(|_| ())
            }
            CommandIntent::CreateBackupSnapshot => {
                self.snapshots()?.create_current().await.map(|_| ())
            }
            CommandIntent::RestoreSnapshot { id } => {
                let profile = self.profile()?.current_profile().await?;
                let path = std::path::PathBuf::from(id);
                self.snapshots()?
                    .restore(self.managed_runtime.clone(), &profile, &path)
                    .await
            }
            CommandIntent::RollbackCore => self.versions()?.rollback().await.map(|_| ()),
            CommandIntent::PrepareServiceMode => self.service_mode()?.prepare().await.map(|_| ()),
            CommandIntent::RepairPortConflicts => self.port_conflicts()?.repair().await.map(|_| ()),
            CommandIntent::CheckUpdates => {
                let settings = self.settings()?.load().await?;
                let channel = parse_release_channel(&settings.core_channel)?;
                self.versions()?.latest(channel).await.map(|_| ())
            }
            CommandIntent::UpdateSetting { key, value } => self.update_setting(&key, &value).await,
            CommandIntent::SetProxyMode { mode } => self
                .runtime()?
                .set_proxy_mode(mode)
                .await
                .map_err(Failure::from),
            CommandIntent::SetCoreLogLevel { level } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_core_log_level(level)
                    .await
            }
            CommandIntent::SetTunStack { stack } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_stack(stack)
                    .await
            }
            CommandIntent::ToggleTun { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_enabled(enabled)
                    .await
            }
            CommandIntent::SetTunAutoRoute { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_auto_route(enabled)
                    .await
            }
            CommandIntent::SetTunStrictRoute { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_tun_strict_route(enabled)
                    .await
            }
            CommandIntent::ProbeTunMtu => {
                let runtime = self.runtime()?.clone();
                let snapshot = self.mtu()?.probe_and_apply(runtime).await;
                match snapshot.state {
                    MtuProbeState::Ready => Ok(()),
                    MtuProbeState::Unsupported => Err(Failure::unsupported(
                        "current host does not expose physical-link MTU probing",
                    )),
                    MtuProbeState::Failed { failure } => Err(failure),
                    MtuProbeState::Unknown | MtuProbeState::Probing => Err(Failure::new(
                        ErrorCode::InvalidState,
                        "MTU probe did not reach a terminal state",
                        true,
                    )),
                }
            }
            CommandIntent::SetSystemProxy { enabled } => {
                let endpoint = if enabled {
                    let config = self.runtime()?.get_config().await.map_err(Failure::from)?;
                    let port = if config.mixed_port > 0 {
                        config.mixed_port
                    } else {
                        config.port
                    };
                    (port > 0)
                        .then(|| format!("127.0.0.1:{port}"))
                        .ok_or_else(|| {
                            Failure::new(
                                ErrorCode::NotReady,
                                "current configuration has no HTTP proxy endpoint",
                                true,
                            )
                        })
                        .map(Some)?
                } else {
                    None
                };
                self.system_proxy()?
                    .set_enabled(enabled, endpoint, None)
                    .await
                    .map(|_| ())
            }
            CommandIntent::SetLanSharing {
                enabled,
                mixed_port,
                bind_address,
            } => RuntimeQueryApplication::new(self.runtime()?)
                .set_lan_sharing(enabled, mixed_port, &bind_address)
                .await
                .map(|_| ()),
            CommandIntent::SetLanSecurity {
                allowed_ips,
                disallowed_ips,
                skip_auth_prefixes,
                authentication_enabled,
                credentials,
            } => RuntimeQueryApplication::new(self.runtime()?)
                .set_lan_security(
                    &allowed_ips,
                    &disallowed_ips,
                    &skip_auth_prefixes,
                    authentication_enabled,
                    credentials.as_ref(),
                )
                .await
                .map(|_| ()),
            CommandIntent::SetIpv6Routing { enabled } => {
                RuntimeQueryApplication::new(self.runtime()?)
                    .set_ipv6_routing(enabled)
                    .await
                    .map(|_| ())
            }
            CommandIntent::ScanUwpApps => {
                self.uwp_loopback()?.snapshot().await;
                Ok(())
            }
            CommandIntent::SetUwpAppExemption { sid, exempt } => self
                .uwp_loopback()?
                .set_exempt(&sid, exempt)
                .await
                .map(|_| ()),
            CommandIntent::SetAllUwpExemptions { exempt } => {
                self.uwp_loopback()?.set_all(exempt).await.map(|_| ())
            }
            CommandIntent::ApplyPac {
                enabled,
                bypass_domains,
                bypass_lan,
                minify,
            } => self
                .pac()?
                .apply(infiltrator_contract::pac::PacRequest {
                    enabled,
                    bypass_domains,
                    bypass_lan,
                    minify,
                })
                .await
                .map(|_| ()),
            CommandIntent::RefreshNetworkRoaming => {
                let snapshot = self.network_roaming()?.refresh().await;
                match snapshot.status {
                    infiltrator_contract::network_roaming::NetworkRoamingStatus::Failed {
                        failure,
                    } => Err(failure),
                    infiltrator_contract::network_roaming::NetworkRoamingStatus::Unsupported {
                        reason,
                    } => Err(Failure::unsupported(reason)),
                    _ => Ok(()),
                }
            }
            CommandIntent::RepairNetworkRoutes => {
                self.network_roaming()?.force_repair().await.map(|_| ())
            }
            CommandIntent::StartVpn => {
                let snapshot = self.vpn()?.request_start().await?;
                match snapshot.state {
                    infiltrator_contract::vpn::VpnSessionState::Unsupported { reason } => {
                        Err(Failure::unsupported(reason))
                    }
                    infiltrator_contract::vpn::VpnSessionState::Failed { failure } => Err(failure),
                    _ => Ok(()),
                }
            }
            CommandIntent::StopVpn => self.vpn()?.stop().await.map(|_| ()),
            CommandIntent::ResetRuleHitCounters => {
                self.rule_tracer()?.clear_hits();
                Ok(())
            }
            CommandIntent::SimulateRuleTrace { query } => {
                self.rule_tracer()?.set_query(&query);
                Ok(())
            }
            CommandIntent::SetRuleTracerContext { src_ip } => {
                self.rule_tracer()?.set_context(&TrafficContextSnapshot {
                    src_ip,
                    ..TrafficContextSnapshot::default()
                });
                Ok(())
            }
            CommandIntent::ApplyTracerRuleOverride { request } => {
                let result = self.rule_tracer()?.apply_override(&request).await;
                match result.into_failure() {
                    Some(failure) => Err(failure),
                    None => Ok(()),
                }
            }
            CommandIntent::ToggleRuleEnabled { index } => {
                self.edit_rules(|rules| edit::toggle_rule_enabled(rules, index))
                    .await
            }
            CommandIntent::MoveRule { index, direction } => {
                self.edit_rules(|rules| edit::move_rule(rules, index, direction))
                    .await
            }
            CommandIntent::AddCustomRule {
                rule_type,
                payload,
                target,
            } => {
                let draft = infiltrator_contract::rule_edit::RuleDraft {
                    rule_type,
                    payload,
                    target,
                };
                let entry = edit::build_custom_rule(&draft)
                    .map_err(|error| Failure::new(ErrorCode::InvalidInput, error, false))?;
                self.edit_rules(move |rules| edit::prepend_rules(rules, [entry]) > 0)
                    .await
            }
            CommandIntent::ApplyGameRoutingPresets { target } => {
                self.edit_rules(move |rules| edit::inject_game_presets(rules, &target) > 0)
                    .await
            }
            CommandIntent::RunPrivilegedNetworkRegression => self
                .privileged_network()?
                .run(infiltrator_contract::privileged_network::PrivilegedNetworkRequest::standard())
                .await
                .map(|_| ()),
            CommandIntent::RefreshPublicIpProbe
            | CommandIntent::ReorderOverviewCards { .. }
            | CommandIntent::ResetOverviewCardOrder
            | CommandIntent::StartCore
            | CommandIntent::StopCore
            | CommandIntent::RestartCore
            | CommandIntent::ClearLogs
            | CommandIntent::SetLogLevelFilter { .. }
            | CommandIntent::TestDnsLatency
            | CommandIntent::ToggleIncludeSystemApps { .. }
            | CommandIntent::ResolveConflictKeepLocal
            | CommandIntent::ResolveConflictTakeRemote
            | CommandIntent::UnpackRuleProvider { .. } => Err(unsupported()),
        }
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
