use crate::composition::{core_application, desktop_product_backend};
use crate::network_roaming::DesktopNetworkRoamingPort;
use crate::pac_service::DesktopPacServicePort;
use crate::rule_override::DesktopRuleOverridePort;
use crate::script_export::DesktopScriptExportPort;
use crate::service::{ServiceManager, ServiceStatus};
use crate::service_mode::DesktopServiceMode;
use crate::storage::doctor;
use crate::storage::{settings_store, speedtest_history_store, subscription_source};
use crate::surface::{SurfaceEngines, SurfacePumpParams, surface_pump};
use crate::system_dns_cache::DesktopSystemDnsCache;
use crate::system_proxy::DesktopSystemProxy;
use crate::tun_service::{ServiceModeStatus, TunServiceManager};
use anyhow::anyhow;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_latency_application::DnsLatencyApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_application::stun_probe_application::StunProbeApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::stun_probe::DEFAULT_STUN_SERVER;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::system_proxy::SystemProxyRecoveryStatus;
use infiltrator_core::apply::{
    ApplyOutcome, ApplyParams, EndpointConfigReloader, apply_current_profile,
};
use infiltrator_core::dns_latency_io::HttpDnsLatencyProber;
use infiltrator_core::dns_leak_io::HttpDnsLeakEchoProbe;
use infiltrator_core::snapshot_io::FileSnapshotStore;
use infiltrator_core::stun_io::UdpStunProbe;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::proxy::ProxyGroup;
use infiltrator_ports::core_binary_selection::CoreBinarySelection;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::engine_backend_factory::EngineBackendPorts;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationTransactionPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use mihomo_api::client::MihomoClient;
use mihomo_api::controller_fake_ip_cache::MihomoControllerFakeIpCacheAdapter;
use mihomo_api::proxy::manager::ProxyManager;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use mihomo_version::engine::engine_id;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time;
use tokio::fs::{copy, metadata, read_to_string};
use tokio::sync::Mutex;
use yaml_rust2::{Yaml, YamlLoader};

mod ports;
mod product_commands;

pub(crate) struct MihomoRuntime {
    profiles: ProfileApplication,
    configuration: ConfigurationApplication,
    snapshots: SnapshotApplication,
    scripts: ScriptApplication,
    script_exports: ScriptExportApplication,
    config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
    pub config_path: PathBuf,
    pub binary_path: PathBuf,
    pub controller_url: String,
    client: MihomoClient,
    core_capability_provider:
        Arc<dyn infiltrator_ports::core_capability_provider::CoreCapabilityProvider>,
    runtime_gateway: Arc<dyn RuntimeGateway>,
    backend_ports: EngineBackendPorts,
    service_manager: ServiceManager,
    endpoints: Arc<ProfileEndpointSource<DefaultCredentialStore>>,
    application: Arc<CoreApplication>,
    _watchdog: infiltrator_composition::CoreWatchdogHandle,
    apply_guard: Arc<Mutex<()>>,
    configuration_transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    service_mode: Arc<DesktopServiceMode>,
    pac_service: Arc<DesktopPacServicePort>,
    network_roaming_port: Arc<DesktopNetworkRoamingPort>,
    rule_tracer: RuleTracerApplication,
    rule_list: RuleListApplication,
    dns_cache: DnsCacheApplication,
    dns_query: DnsQueryApplication,
    doctor: DoctorApplication,
    /// DUAL-14-10: one prober instance shared by the command handler, the host
    /// port and the surface reader.
    dns_latency: DnsLatencyApplication,
    /// DUAL-14-08: one cross-source leak application shared by the command
    /// handler, the host port and the surface reader. The desktop injects the
    /// real echo adapter; the configured sources stay empty until this host
    /// has a controlled echo authority, so both surfaces publish the typed
    /// unsupported state instead of a verdict.
    dns_leak: DnsLeakApplication,
    /// DUAL-14-09 (re-scoped): one STUN UDP-egress probe application shared by
    /// the command handler, the host port and the surface reader. The desktop
    /// injects the real UDP adapter; the observed mapping is this host's own
    /// egress as seen by a STUN server, never a browser WebRTC result.
    stun_probe: StunProbeApplication,
}

#[derive(Debug, Clone, Serialize)]
pub struct MihomoSummary {
    pub profile: String,
    pub running: bool,
    pub controller: String,
    pub groups: Vec<ProxyGroup>,
}

impl MihomoRuntime {
    /// Initialize the real desktop ports without starting a stopped kernel.
    pub(crate) async fn prepare_offline(
        selection: CoreBinarySelection,
        config_manager: Arc<ConfigManager<DefaultCredentialStore>>,
        mut preflight: infiltrator_composition::active_profile_backend::ActiveProfileSelectionPreflight,
    ) -> anyhow::Result<Self> {
        if selection.engine_id != preflight.reference.engine_id
            || selection.engine_id != engine_id()
        {
            anyhow::bail!(
                "Mihomo runtime cannot own selected engine {}",
                selection.engine_id.as_str()
            );
        }
        let binary = selection.binary_path.clone();
        let recovery = SystemProxyApplication::new(Arc::new(DesktopSystemProxy::new()));
        let recovery_snapshot = recovery.recover_orphaned().await;
        if !matches!(
            &recovery_snapshot.status,
            SystemProxyRecoveryStatus::NotNeeded
        ) {
            log::warn!(
                "system proxy startup recovery: {:?}",
                recovery_snapshot.status
            );
        }
        let cm = config_manager;
        let active_profile = preflight.reference.profile_id.clone();
        let workspace = ProfileStore::load_workspace(cm.as_ref(), &active_profile).await?;
        if workspace.document.identity != preflight.document.identity
            || workspace.document.identity.engine_id != preflight.reference.engine_id
            || workspace.document.identity.profile_id != preflight.reference.profile_id
        {
            return Err(anyhow::Error::new(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "profile document changed after desktop product preflight",
                true,
            ))));
        }
        let content = cm.load(active_profile.as_str()).await?;
        mihomo_config::yaml::validate(&content)?;
        let config_path = cm
            .existing_profile_yaml_path(active_profile.as_str())
            .await?;
        cm.ensure_proxy_ports_for_profile(&active_profile).await?;
        let controller_url = cm
            .ensure_external_controller_for_profile(&active_profile)
            .await?;
        let workspace = ProfileStore::load_workspace(cm.as_ref(), &active_profile).await?;
        preflight.refresh_document(workspace.document)?;
        let geoip_candidates =
            collect_geoip_candidates(&binary, &selection.related_asset_candidates);
        ensure_geoip_database_offline(&config_path, &geoip_candidates).await?;
        let service_manager = ServiceManager::new(binary.clone(), config_path.clone());
        let service_mode = Arc::new(DesktopServiceMode::new(binary.clone()));

        let endpoints = Arc::new(ProfileEndpointSource::new(cm.clone()));
        let endpoint = endpoints.resolve_profile(&active_profile).await?;
        let backend = desktop_product_backend(
            &service_manager,
            &endpoint.url,
            endpoint.secret.clone(),
            cm.clone(),
            preflight,
        )
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
        let backend_ports = backend.ports.clone();
        let configuration_transaction = backend_ports.configuration_transaction();
        let client = backend.client.clone();
        let core_capability_provider = Arc::clone(&backend.capability_provider);
        let runtime_gateway: Arc<dyn RuntimeGateway> = Arc::new(backend.client.clone());
        // One speedtest owner is installed into Core composition. The bounded
        // history is versioned and persisted.
        let history_store: Arc<dyn SpeedtestHistoryStore> = Arc::new(speedtest_history_store()?);
        let speedtest =
            SpeedtestApplication::new(SettingsApplication::new(settings_store().await?))
                .with_history_store(history_store);
        // One live rule tracer engine shared by the host runtime port and the
        // surface reader, so both surfaces replay the same query state.
        let rule_tracer = RuleTracerApplication::new();
        let rule_list = RuleListApplication::default();
        // One DNS cache application shared by the command handler and the
        // surface reader, so the honest last flush report reaches the UI.
        let dns_query = DnsQueryApplication::new(Some(Arc::new(client.clone())));
        let dns_cache = DnsCacheApplication::new(
            Some(Arc::new(MihomoControllerFakeIpCacheAdapter::new(
                client.clone(),
            ))),
            Some(Arc::new(DesktopSystemDnsCache::new())),
        );
        // DUAL-14-10: the real per-nameserver prober. The desktop host owns
        // the UDP/DoH network I/O; the shared application only publishes the
        // measured report.
        let dns_latency = DnsLatencyApplication::new(Some(Arc::new(HttpDnsLatencyProber::new())));
        // DUAL-14-08: the real echo adapter plus the real default TXT echo
        // authorities (two independent public services through the platform
        // resolver). The application still compares both observations; an
        // unreachable authority is a typed failure, never a guessed verdict.
        let dns_leak = DnsLeakApplication::new(
            Some(Arc::new(HttpDnsLeakEchoProbe::new())),
            default_echo_sources(),
        );
        // DUAL-14-09 (re-scoped): the real UDP STUN adapter and the public
        // default server. The observation is this host/process's UDP egress
        // mapping as seen by a STUN server; the application never presents it
        // as a browser WebRTC result, and a host with no expected egress keeps
        // the comparison honestly unknown.
        let stun_probe =
            StunProbeApplication::new(Some(Arc::new(UdpStunProbe::new())), DEFAULT_STUN_SERVER);
        let doctor = DoctorApplication::new(Arc::new(doctor()?));
        let profiles =
            ProfileApplication::new(cm.clone()).with_document_codec(backend.ports.configuration());
        let configuration = ConfigurationApplication::from_profile_application(profiles.clone());
        let apply_guard = Arc::new(Mutex::new(()));
        let snapshots = SnapshotApplication::new(
            cm.clone(),
            Arc::new(FileSnapshotStore::new(cm.config_dir().to_path_buf())),
        );
        let scripts = ScriptApplication::new();
        let script_exports = ScriptExportApplication::new(Some(Arc::new(
            DesktopScriptExportPort::new(cm.config_dir()),
        )));
        let application = Arc::new(core_application(
            &service_manager,
            backend,
            endpoint.secret.clone(),
            SurfaceEngines {
                profiles: profiles.clone(),
                configuration: configuration.clone(),
                snapshots: snapshots.clone(),
                scripts: scripts.clone(),
                script_exports: script_exports.clone(),
                doctor: doctor.clone(),
                rule_tracer: rule_tracer.clone(),
                rule_list: rule_list.clone(),
                dns_cache: dns_cache.clone(),
                dns_query: dns_query.clone(),
                dns_latency: dns_latency.clone(),
                dns_leak: dns_leak.clone(),
                stun_probe: stun_probe.clone(),
            },
            speedtest.clone(),
            cm.clone(),
            Arc::new(subscription_source()),
            cm.clone(),
            endpoints.clone() as Arc<dyn EndpointSource>,
            apply_guard.clone(),
        )?);
        let scope_application = application.clone();
        let scope_resolver: mihomo_api::runtime_configuration::RuntimeScopeResolver =
            Arc::new(move || {
                let lifecycle = scope_application.snapshot().lifecycle_snapshot();
                match &lifecycle.lifecycle {
                    CoreLifecycle::Running | CoreLifecycle::Ready => {
                        lifecycle.scope().ok_or_else(|| {
                            PortError::Failed(
                                "active Core identity is unavailable for configuration access"
                                    .into(),
                            )
                        })
                    }
                    _ => Err(PortError::Failed(
                        "configuration access requires a running Core".to_owned(),
                    )),
                }
            });
        client.set_configuration_scope_resolver(scope_resolver);
        // Attach to an already-running instance by proving it answers, or
        // start a fresh one and let the application own readiness retries.
        if service_manager.is_running().await {
            log::info!("Attaching to running mihomo service");
            application
                .adopt_if_running()
                .await
                .map_err(|error| anyhow!(error.message))?;
        }
        let watchdog = infiltrator_composition::spawn_core_watchdog(application.clone());
        let pac_service = Arc::new(DesktopPacServicePort::shared());
        let network_roaming_port = Arc::new(DesktopNetworkRoamingPort::shared());
        // DUAL-12-08: the tracer reverse-apply commits through the same apply
        // transaction the editor uses, sharing its serialization guard.
        let rule_port = Arc::new(DesktopRuleOverridePort::new(
            cm.clone(),
            profiles.clone(),
            application.clone(),
            configuration_transaction.clone(),
            apply_guard.clone(),
        ));
        rule_tracer.set_override_port(rule_port.clone());
        rule_list.set_port(rule_port);

        Ok(Self {
            profiles,
            configuration,
            snapshots,
            scripts,
            script_exports,
            config_manager: cm,
            config_path,
            binary_path: binary,
            controller_url,
            client,
            core_capability_provider,
            runtime_gateway,
            backend_ports,
            service_manager,
            endpoints,
            application,
            _watchdog: watchdog,
            apply_guard,
            configuration_transaction,
            service_mode,
            pac_service,
            rule_tracer,
            rule_list,
            dns_cache,
            dns_query,
            doctor,
            dns_latency,
            dns_leak,
            stun_probe,
            network_roaming_port,
        })
    }

    /// The 0.30 contract/application seam for host surfaces. Legacy runtime
    /// helpers remain available while their use-cases are migrated.
    pub fn application(&self) -> Arc<CoreApplication> {
        self.application.clone()
    }

    pub fn core_binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub fn tun_service_status(&self) -> ServiceModeStatus {
        TunServiceManager::check_status_for(&self.binary_path)
    }

    /// Apply the current profile's content to the core through the
    /// CORE-004 transaction: atomic write, reload-or-restart, readiness,
    /// rollback on failure.
    pub async fn apply_current_config(
        &self,
        strategy: ApplyStrategy,
    ) -> anyhow::Result<ApplyOutcome> {
        let _guard = self.apply_guard.lock().await;
        let profile = ProfileStore::get_current(self.config_manager.as_ref()).await?;
        let content = self.config_manager.load(&profile).await?;
        let reloader =
            EndpointConfigReloader::new(self.endpoints.clone() as Arc<dyn EndpointSource>);
        let params = ApplyParams {
            strategy,
            ..Default::default()
        };
        let outcome = apply_current_profile(
            self.application.as_ref(),
            &self.config_manager,
            &reloader,
            &content,
            params,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
        if let Err(error) = self.config_manager.clear_backup(&profile).await {
            // The core is already healthy and the transaction has committed;
            // a stale transient backup must not turn that success into a
            // false failure (it will be overwritten by the next save).
            log::warn!("failed to clear profile apply backup: {error}");
        }
        Ok(outcome)
    }

    /// Apply caller-provided content through the same atomic write,
    /// readiness and rollback transaction used by the current profile flow.
    /// This is used by desktop editors so a running core never observes a
    /// directly-written file that has not been validated and applied.
    pub async fn apply_profile_content(
        &self,
        new_content: &str,
        strategy: ApplyStrategy,
    ) -> anyhow::Result<ApplyOutcome> {
        let _guard = self.apply_guard.lock().await;
        let profile = ProfileStore::get_current(self.config_manager.as_ref()).await?;
        let reloader =
            EndpointConfigReloader::new(self.endpoints.clone() as Arc<dyn EndpointSource>);
        let params = ApplyParams {
            strategy,
            ..Default::default()
        };
        apply_current_profile(
            self.application.as_ref(),
            &self.config_manager,
            &reloader,
            new_content,
            params,
        )
        .await
        .map_err(|error| anyhow!("profile {profile}: {error}"))
    }

    pub fn client(&self) -> MihomoClient {
        self.client.clone()
    }

    /// Compose the complete 11-page surface reader for an inbound UI. The
    /// returned pump owns only runtime-neutral snapshots; all concrete
    /// storage/controller details stay in this desktop host.
    pub async fn surface_pump(
        &self,
        surface: SurfaceKind,
        sample_interval: time::Duration,
    ) -> anyhow::Result<SurfacePump> {
        surface_pump(SurfacePumpParams {
            core: self.application.clone(),
            gateway: self.runtime_gateway.clone(),
            routing_intent_port: Arc::new(MihomoRoutingIntentAdapter::new(self.client.clone())),
            surface,
            sample_interval,
            binary_path: self.binary_path.clone(),
            network_roaming_port: self.network_roaming_port.clone(),
            engines: self.surface_engines(),
        })
        .await
    }

    fn surface_engines(&self) -> SurfaceEngines {
        SurfaceEngines {
            profiles: self.profiles.clone(),
            configuration: self.configuration.clone(),
            snapshots: self.snapshots.clone(),
            scripts: self.scripts.clone(),
            script_exports: self.script_exports.clone(),
            doctor: self.doctor.clone(),
            rule_tracer: self.rule_tracer.clone(),
            rule_list: self.rule_list.clone(),
            dns_cache: self.dns_cache.clone(),
            dns_query: self.dns_query.clone(),
            dns_latency: self.dns_latency.clone(),
            dns_leak: self.dns_leak.clone(),
            stun_probe: self.stun_probe.clone(),
        }
    }

    pub async fn summary(&self) -> anyhow::Result<MihomoSummary> {
        let profile = ProfileStore::get_current(self.config_manager.as_ref()).await?;
        let running = matches!(
            self.service_manager.status().await?,
            ServiceStatus::Running(_)
        );

        let proxy_manager = ProxyManager::new(self.client());
        let groups = proxy_manager.list_groups().await.unwrap_or_default();

        Ok(MihomoSummary {
            profile,
            running,
            controller: self.controller_url.clone(),
            groups,
        })
    }

    pub async fn switch_proxy(&self, group: &str, proxy: &str) -> anyhow::Result<()> {
        self.client
            .switch_proxy(group, proxy)
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        CoreLifecyclePort::stop(self.application.as_ref())
            .await
            .map_err(|e| anyhow!(e.to_string()))
    }

    pub async fn is_running(&self) -> bool {
        self.service_manager.is_running().await
    }

    pub async fn http_proxy_endpoint(&self) -> anyhow::Result<Option<String>> {
        let content = read_to_string(&self.config_path).await?;
        proxy_endpoint_from_config(&content)
    }
}

fn proxy_endpoint_from_config(content: &str) -> anyhow::Result<Option<String>> {
    let doc = parse_yaml_doc(content)?;
    let port = get_yaml_u16(&doc, "mixed-port").or_else(|| get_yaml_u16(&doc, "port"));
    Ok(port
        .filter(|port| *port != 0)
        .map(|p| format!("127.0.0.1:{p}")))
}

const GEOIP_MIN_SIZE: u64 = 1024 * 1024;

fn collect_geoip_candidates(binary: &Path, bundled_candidates: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = binary.parent() {
        dirs.push(dir.to_path_buf());
    }
    for candidate in bundled_candidates {
        if let Some(dir) = candidate.parent()
            && !dirs.contains(&dir.to_path_buf())
        {
            dirs.push(dir.to_path_buf());
        }
    }

    dirs.into_iter()
        .map(|dir| dir.join("geoip.metadb"))
        .collect()
}

/// Local-only GeoIP preparation for the first boot. A missing database is a
/// visible degradation handled by the shared startup snapshot; it is never a
/// reason to block a valid local profile or to start an HTTP request.
async fn ensure_geoip_database_offline(
    config_path: &Path,
    geoip_candidates: &[PathBuf],
) -> anyhow::Result<()> {
    let content = read_to_string(config_path).await?;
    if !content.to_ascii_uppercase().contains("GEOIP") {
        return Ok(());
    }

    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("配置目录不存在"))?;
    let geoip_path = config_dir.join("geoip.metadb");
    if metadata(&geoip_path)
        .await
        .is_ok_and(|meta| meta.len() >= GEOIP_MIN_SIZE)
    {
        return Ok(());
    }

    if try_copy_geoip_candidates(geoip_candidates, &geoip_path).await? {
        return Ok(());
    }

    log::warn!(
        "offline bootstrap: local GeoIP database unavailable at {}; continuing without network",
        geoip_path.display()
    );
    Ok(())
}

async fn try_copy_geoip_candidates(
    candidates: &[PathBuf],
    geoip_path: &Path,
) -> anyhow::Result<bool> {
    for candidate in candidates {
        if let Ok(meta) = metadata(candidate).await
            && meta.len() >= GEOIP_MIN_SIZE
        {
            log::info!(
                "使用本地 GeoIP 数据库: {} -> {}",
                candidate.display(),
                geoip_path.display()
            );
            copy(candidate, geoip_path).await?;
            return Ok(true);
        }
    }

    Ok(false)
}

fn parse_yaml_doc(content: &str) -> anyhow::Result<Yaml> {
    let docs = YamlLoader::load_from_str(content).map_err(|err| anyhow!(err.to_string()))?;
    docs.into_iter()
        .next()
        .ok_or_else(|| anyhow!("配置内容不是有效的 YAML"))
}

fn get_yaml_u16(doc: &Yaml, key: &str) -> Option<u16> {
    let value = doc.as_hash()?.get(&Yaml::String(key.to_string()))?;
    match value {
        Yaml::Integer(num) => {
            if *num >= 0 && *num <= u16::MAX as i64 {
                Some(*num as u16)
            } else {
                None
            }
        }
        Yaml::Real(raw) => raw.parse::<u16>().ok(),
        Yaml::String(raw) => raw.parse::<u16>().ok(),
        _ => None,
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
