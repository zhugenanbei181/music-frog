//! Host port trait implementations for [`MihomoRuntime`]: the runtime
//! gateway, the managed-runtime lifecycle and the host adapter composition.

use super::MihomoRuntime;
use crate::certificate_authority::DesktopCertificateAuthority;
use crate::mini_hud_window::DesktopMiniHudWindow;
use crate::mtu::DesktopMtuProbe;
use crate::rule_provider_cache::DesktopRuleProviderCache;
use crate::script_export::DesktopScriptExportPort;
use crate::service::ServiceStatus;
use crate::speedtest::DesktopSpeedtestPort;
use crate::system_dns_cache::DesktopSystemDnsCache;
use crate::system_proxy::DesktopSystemProxy;
use crate::tun_service::{ServiceModeStatus, TunServiceManager};
use futures_util::stream::{BoxStream, unfold};
use infiltrator_contract::command::{CommandIntent, CommandResult, ProxyMode};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_core::apply::{ApplyError, ApplyParams, EndpointConfigReloader};
use infiltrator_core::apply_workspace::apply_confirmed_workspace;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::runtime::{
    ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::dns_latency::DnsLatencyProbePort;
use infiltrator_ports::dns_leak::DnsLeakProbePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::mini_hud_window::MiniHudWindowPort;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::pac::PacServicePort;
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_gateway::{
    ManagedRuntime, RuntimeGateway, RuntimeStream, RuntimeStreamEvent,
};
use infiltrator_ports::script_export::ScriptExportPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::speedtest::SpeedtestPort;
use infiltrator_ports::stun_probe::StunEgressProbePort;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use mihomo_api::client::StreamEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::sync::mpsc::UnboundedReceiver;

#[async_trait::async_trait]
impl RuntimeGateway for MihomoRuntime {
    async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
        self.client
            .get_config()
            .await
            .map(Into::into)
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn patch_config(&self, updates: serde_json::Value) -> Result<(), PortError> {
        self.client
            .patch_config(updates)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_rules(&self) -> Result<Vec<RuleEntry>, PortError> {
        self.client
            .get_rules()
            .await
            .map(|rules| {
                rules
                    .into_iter()
                    .map(|rule| RuleEntry {
                        rule: format!("{},{},{}", rule.rule_type, rule.payload, rule.proxy),
                        enabled: true,
                    })
                    .collect()
            })
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn set_proxy_mode(&self, mode: ProxyMode) -> Result<(), PortError> {
        match self
            .application
            .execute(CommandIntent::SetProxyMode { mode })
            .await
        {
            CommandResult::Completed { .. } => Ok(()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Rejected(failure)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "proxy mode command unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
        self.client
            .get_proxies()
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn switch_proxy(&self, group: &str, proxy: &str) -> Result<(), PortError> {
        self.client
            .switch_proxy(group, proxy)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError> {
        self.client
            .test_delay(proxy, url, timeout_ms)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        self.client
            .get_proxy_providers()
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        self.client
            .get_rule_providers()
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn update_proxy_provider(&self, name: &str) -> Result<(), PortError> {
        self.client
            .update_proxy_provider(name)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn update_rule_provider(&self, name: &str) -> Result<(), PortError> {
        self.client
            .update_rule_provider(name)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
        self.client
            .flush_fakeip_cache()
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn upgrade_geo(&self) -> Result<(), PortError> {
        self.client
            .upgrade_geo()
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        self.client
            .get_connections()
            .await
            .map(Into::into)
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        self.client
            .get_memory()
            .await
            .map(Into::into)
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_cpu_percent(&self) -> Result<Option<f32>, PortError> {
        let pid = match self
            .service_manager
            .status()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?
        {
            ServiceStatus::Running(pid) => pid,
            ServiceStatus::Stopped => return Ok(None),
        };
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        Ok(system
            .process(Pid::from_u32(pid))
            .map(|process| process.cpu_usage()))
    }

    async fn trigger_gc(&self) -> Result<(), PortError> {
        self.client
            .trigger_gc()
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn close_connection(&self, id: &str) -> Result<(), PortError> {
        self.client
            .close_connection(id)
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        self.client
            .close_all_connections()
            .await
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn stream_logs(&self, level: Option<String>) -> Result<RuntimeStream<String>, PortError> {
        let receiver = self
            .client
            .stream_logs_events(level.as_deref())
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(map_stream(receiver, |line| line))
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        let receiver = self
            .client
            .stream_traffic_events()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(map_stream(receiver, Into::into))
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        let receiver = self
            .client
            .stream_connections_events()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(map_stream(receiver, Into::into))
    }
}

#[async_trait::async_trait]
impl ManagedRuntime for MihomoRuntime {
    async fn apply_profile_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let _guard = self.apply_guard.lock().await;
        let active = ProfileStore::get_current(self.config_manager.as_ref()).await?;
        if active != expected.profile {
            return self
                .config_manager
                .compare_and_save_inactive_workspace(expected, update)
                .await;
        }
        let reloader = EndpointConfigReloader::new(self.endpoints.clone());
        let params = ApplyParams {
            strategy,
            ..Default::default()
        };
        apply_confirmed_workspace(
            self.application.as_ref(),
            &self.config_manager,
            &reloader,
            expected,
            update,
            params,
        )
        .await
        .map_err(workspace_apply_error)
    }
    fn generation(&self) -> u64 {
        self.application.generation()
    }

    async fn is_running(&self) -> bool {
        MihomoRuntime::is_running(self).await
    }

    async fn restart(&self) -> Result<u64, PortError> {
        CoreLifecyclePort::restart(self.application.as_ref())
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError> {
        MihomoRuntime::http_proxy_endpoint(self)
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn shutdown(&self) -> Result<(), PortError> {
        MihomoRuntime::shutdown(self)
            .await
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn apply_current_config(&self, strategy: ApplyStrategy) -> Result<u64, PortError> {
        MihomoRuntime::apply_current_config(self, strategy)
            .await
            .map(|outcome| outcome.generation)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn apply_profile_content(
        &self,
        content: &str,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError> {
        MihomoRuntime::apply_profile_content(self, content, strategy)
            .await
            .map(|outcome| outcome.generation)
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

fn workspace_apply_error(error: ApplyError) -> PortError {
    let (code, retryable) = match error {
        ApplyError::SourceChanged | ApplyError::Busy { .. } => (ErrorCode::NotReady, true),
        ApplyError::Validation(_) => (ErrorCode::Configuration, false),
        ApplyError::Permission(_) => (ErrorCode::Permission, false),
        ApplyError::Write(_) => (ErrorCode::Storage, true),
        ApplyError::RolledBack { .. } => (ErrorCode::InvalidState, true),
        ApplyError::RollbackFailed { .. } | ApplyError::Lifecycle(_) => {
            (ErrorCode::InvalidState, false)
        }
    };
    PortError::Rejected(Failure::new(code, error.to_string(), retryable))
}

impl HostRuntime for MihomoRuntime {
    fn controller_url(&self) -> String {
        self.controller_url.clone()
    }

    fn core_binary_path(&self) -> PathBuf {
        self.binary_path.clone()
    }

    fn tun_service_status(&self) -> TunServiceStatus {
        match TunServiceManager::check_status_for(&self.binary_path) {
            ServiceModeStatus::InstalledAndRunning => TunServiceStatus::InstalledAndRunning,
            ServiceModeStatus::InstalledStopped => TunServiceStatus::InstalledStopped,
            ServiceModeStatus::NotInstalled => TunServiceStatus::NotInstalled,
            ServiceModeStatus::MissingPrivilege => TunServiceStatus::MissingPrivilege,
            ServiceModeStatus::Unsupported => TunServiceStatus::Unsupported,
        }
    }

    fn service_mode_port(&self) -> Option<Arc<dyn ServiceModePort>> {
        Some(self.service_mode.clone())
    }

    fn mtu_probe_port(&self) -> Option<Arc<dyn MtuProbePort>> {
        Some(Arc::new(DesktopMtuProbe::new()))
    }

    fn system_proxy_port(&self) -> Option<Arc<dyn SystemProxyPort>> {
        Some(Arc::new(DesktopSystemProxy::new()))
    }

    fn pac_service_port(&self) -> Option<Arc<dyn PacServicePort>> {
        Some(self.pac_service.clone())
    }

    fn network_roaming_port(&self) -> Option<Arc<dyn NetworkRoamingPort>> {
        Some(self.network_roaming_port.clone())
    }

    fn privileged_network_port(&self) -> Option<Arc<dyn PrivilegedNetworkPort>> {
        // The regression adapter is deliberately injected by headless hosts;
        // desktop production must not run destructive privilege probes merely
        // because a surface was opened.
        None
    }

    fn certificate_authority_port(&self) -> Option<Arc<dyn CertificateAuthorityPort>> {
        // DUAL-05-13: the desktop host can read a CA bundle from disk; a host
        // without this adapter surfaces a typed unsupported state instead.
        Some(Arc::new(DesktopCertificateAuthority::new()))
    }

    fn speedtest_port(&self) -> Option<Arc<dyn SpeedtestPort>> {
        Some(Arc::new(DesktopSpeedtestPort::new(self.speedtest.clone())))
    }

    fn system_dns_cache_port(&self) -> Option<Arc<dyn SystemDnsCachePort>> {
        Some(Arc::new(DesktopSystemDnsCache::new()))
    }

    /// DUAL-14-10: the desktop host owns the real UDP / DoH probe. The shared
    /// application is the port itself, so a probe started from either surface
    /// lands in the one report the surface reader publishes.
    fn dns_latency_probe_port(&self) -> Option<Arc<dyn DnsLatencyProbePort>> {
        Some(Arc::new(self.dns_latency.clone()))
    }

    /// DUAL-14-08: the shared cross-source leak application is the port
    /// itself, so a probe started from either surface lands in the one report
    /// the surface reader publishes.
    fn dns_leak_probe_port(&self) -> Option<Arc<dyn DnsLeakProbePort>> {
        Some(Arc::new(self.dns_leak.clone()))
    }

    /// DUAL-14-09 (re-scoped): the shared STUN application is the port itself,
    /// so a probe started from either surface lands in the one report the
    /// surface reader publishes.
    fn stun_egress_probe_port(&self) -> Option<Arc<dyn StunEgressProbePort>> {
        Some(Arc::new(self.stun_probe.clone()))
    }

    fn mini_hud_window_port(&self) -> Option<Arc<dyn MiniHudWindowPort>> {
        // The desktop host owns the adapter; the active surface registers the
        // live window handle when its window id resolves. Until then every
        // apply answers a typed unsupported (never a fake "Applied").
        Some(Arc::new(DesktopMiniHudWindow::shared()))
    }

    fn rule_provider_cache_port(&self) -> Option<Arc<dyn RuleProviderCachePort>> {
        // The core is spawned with `-d <config dir>`, so mihomo's home is the
        // directory holding `config_path`; its `rules/` subdirectory is the
        // only location the DUAL-11-06/07 unpack and purge touch.
        let home = self.config_path.parent()?.to_path_buf();
        Some(Arc::new(DesktopRuleProviderCache::new(home)))
    }

    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort> {
        self.application.clone()
    }

    fn script_export_port(&self) -> Option<Arc<dyn ScriptExportPort>> {
        // DUAL-10-12: no native file dialog ships with this desktop product, so
        // the adapter writes into a host-owned `exports/` directory next to the
        // configs and reports the real path. Without a resolvable config dir
        // the port is omitted and every export is a typed unsupported.
        let config_dir = self.config_path.parent()?.to_path_buf();
        Some(Arc::new(DesktopScriptExportPort::new(config_dir)))
    }
}

fn map_stream<T, U>(
    receiver: UnboundedReceiver<StreamEvent<T>>,
    map_item: fn(T) -> U,
) -> BoxStream<'static, RuntimeStreamEvent<U>>
where
    T: Send + 'static,
    U: Send + 'static,
{
    Box::pin(unfold(receiver, move |mut receiver| async move {
        let event = receiver.recv().await?;
        let event = match event {
            StreamEvent::Connecting => RuntimeStreamEvent::Connecting,
            StreamEvent::Connected => RuntimeStreamEvent::Connected,
            StreamEvent::Item(item) => RuntimeStreamEvent::Item(map_item(item)),
            StreamEvent::Reconnecting(error) => RuntimeStreamEvent::Reconnecting(error),
            StreamEvent::Failed(error) => RuntimeStreamEvent::Failed(error),
        };
        Some((event, receiver))
    }))
}
