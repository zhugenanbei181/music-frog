//! Host port trait implementations for [`MihomoRuntime`]: the runtime
//! gateway, the managed-runtime lifecycle and the host adapter composition.

use futures_util::stream::BoxStream;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway, RuntimeStreamEvent};
use std::sync::Arc;
use sysinfo::{Pid, ProcessesToUpdate, System};

use super::MihomoRuntime;
use crate::service::ServiceStatus;
use crate::tun_service::TunServiceManager;

#[async_trait::async_trait]
impl RuntimeGateway for MihomoRuntime {
    async fn get_config(&self) -> Result<infiltrator_domain::runtime::ConfigSnapshot, PortError> {
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

    async fn set_proxy_mode(
        &self,
        mode: infiltrator_contract::command::ProxyMode,
    ) -> Result<(), PortError> {
        match self
            .application
            .execute(infiltrator_contract::command::CommandIntent::SetProxyMode { mode })
            .await
        {
            infiltrator_contract::command::CommandResult::Completed { .. } => Ok(()),
            infiltrator_contract::command::CommandResult::Rejected { failure, .. } => {
                Err(PortError::Failed(failure.message))
            }
            infiltrator_contract::command::CommandResult::Accepted { .. } => Err(
                PortError::Failed("proxy mode command unexpectedly returned Accepted".to_string()),
            ),
        }
    }

    async fn get_proxies(
        &self,
    ) -> Result<std::collections::HashMap<String, infiltrator_domain::proxy::Proxy>, PortError>
    {
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

    async fn get_proxy_providers(
        &self,
    ) -> Result<Vec<infiltrator_domain::runtime::ProxyProvider>, PortError> {
        self.client
            .get_proxy_providers()
            .await
            .map(|providers| providers.into_values().map(Into::into).collect())
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_rule_providers(
        &self,
    ) -> Result<Vec<infiltrator_domain::runtime::RuleProvider>, PortError> {
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

    async fn get_connections(
        &self,
    ) -> Result<infiltrator_domain::runtime::ConnectionSnapshot, PortError> {
        self.client
            .get_connections()
            .await
            .map(Into::into)
            .map_err(|error| PortError::Network(error.to_string()))
    }

    async fn get_memory(&self) -> Result<infiltrator_domain::runtime::MemoryData, PortError> {
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

    async fn stream_logs(
        &self,
        level: Option<String>,
    ) -> Result<infiltrator_ports::runtime_gateway::RuntimeStream<String>, PortError> {
        let receiver = self
            .client
            .stream_logs_events(level.as_deref())
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(map_stream(receiver, |line| line))
    }

    async fn stream_traffic(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<infiltrator_domain::runtime::TrafficData>,
        PortError,
    > {
        let receiver = self
            .client
            .stream_traffic_events()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(map_stream(receiver, Into::into))
    }

    async fn stream_connections(
        &self,
    ) -> Result<
        infiltrator_ports::runtime_gateway::RuntimeStream<
            infiltrator_domain::runtime::ConnectionSnapshot,
        >,
        PortError,
    > {
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

impl HostRuntime for MihomoRuntime {
    fn controller_url(&self) -> String {
        self.controller_url.clone()
    }

    fn core_binary_path(&self) -> std::path::PathBuf {
        self.binary_path.clone()
    }

    fn tun_service_status(&self) -> TunServiceStatus {
        match TunServiceManager::check_status_for(&self.binary_path) {
            crate::tun_service::ServiceModeStatus::InstalledAndRunning => {
                TunServiceStatus::InstalledAndRunning
            }
            crate::tun_service::ServiceModeStatus::InstalledStopped => {
                TunServiceStatus::InstalledStopped
            }
            crate::tun_service::ServiceModeStatus::NotInstalled => TunServiceStatus::NotInstalled,
            crate::tun_service::ServiceModeStatus::MissingPrivilege => {
                TunServiceStatus::MissingPrivilege
            }
            crate::tun_service::ServiceModeStatus::Unsupported => TunServiceStatus::Unsupported,
        }
    }

    fn service_mode_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::service_mode::ServiceModePort>> {
        Some(self.service_mode.clone())
    }

    fn mtu_probe_port(&self) -> Option<Arc<dyn infiltrator_ports::mtu_probe::MtuProbePort>> {
        Some(Arc::new(crate::mtu::DesktopMtuProbe::new()))
    }

    fn system_proxy_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::system_proxy::SystemProxyPort>> {
        Some(Arc::new(crate::system_proxy::DesktopSystemProxy::new()))
    }

    fn pac_service_port(&self) -> Option<Arc<dyn infiltrator_ports::pac::PacServicePort>> {
        Some(self.pac_service.clone())
    }

    fn network_roaming_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::network_roaming::NetworkRoamingPort>> {
        Some(self.network_roaming_port.clone())
    }

    fn privileged_network_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::privileged_network::PrivilegedNetworkPort>> {
        // The regression adapter is deliberately injected by headless hosts;
        // desktop production must not run destructive privilege probes merely
        // because a surface was opened.
        None
    }

    fn certificate_authority_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::certificate_authority::CertificateAuthorityPort>> {
        // DUAL-05-13: the desktop host can read a CA bundle from disk; a host
        // without this adapter surfaces a typed unsupported state instead.
        Some(Arc::new(
            crate::certificate_authority::DesktopCertificateAuthority::new(),
        ))
    }

    fn speedtest_port(&self) -> Option<Arc<dyn infiltrator_ports::speedtest::SpeedtestPort>> {
        Some(Arc::new(crate::speedtest::DesktopSpeedtestPort::new(
            self.speedtest.clone(),
        )))
    }

    fn rule_tracer_port(&self) -> Option<Arc<dyn infiltrator_ports::rule_tracer::RuleTracerPort>> {
        Some(Arc::new(self.rule_tracer.clone()))
    }

    fn system_dns_cache_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::system_dns_cache::SystemDnsCachePort>> {
        Some(Arc::new(
            crate::system_dns_cache::DesktopSystemDnsCache::new(),
        ))
    }

    /// DUAL-14-10: the desktop host owns the real UDP / DoH probe. The shared
    /// application is the port itself, so a probe started from either surface
    /// lands in the one report the surface reader publishes.
    fn dns_latency_probe_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::dns_latency::DnsLatencyProbePort>> {
        Some(Arc::new(self.dns_latency.clone()))
    }

    /// DUAL-14-08: the shared cross-source leak application is the port
    /// itself, so a probe started from either surface lands in the one report
    /// the surface reader publishes.
    fn dns_leak_probe_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::dns_leak::DnsLeakProbePort>> {
        Some(Arc::new(self.dns_leak.clone()))
    }

    /// DUAL-14-09 (re-scoped): the shared STUN application is the port itself,
    /// so a probe started from either surface lands in the one report the
    /// surface reader publishes.
    fn stun_egress_probe_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::stun_probe::StunEgressProbePort>> {
        Some(Arc::new(self.stun_probe.clone()))
    }

    fn mini_hud_window_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::mini_hud_window::MiniHudWindowPort>> {
        // The desktop host owns the adapter; the active surface registers the
        // live window handle when its window id resolves. Until then every
        // apply answers a typed unsupported (never a fake "Applied").
        Some(Arc::new(
            crate::mini_hud_window::DesktopMiniHudWindow::shared(),
        ))
    }

    fn rule_provider_cache_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::rule_provider_cache::RuleProviderCachePort>> {
        // The core is spawned with `-d <config dir>`, so mihomo's home is the
        // directory holding `config_path`; its `rules/` subdirectory is the
        // only location the DUAL-11-06/07 unpack and purge touch.
        let home = self.config_path.parent()?.to_path_buf();
        Some(Arc::new(
            crate::rule_provider_cache::DesktopRuleProviderCache::new(home),
        ))
    }

    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort> {
        self.application.clone()
    }

    fn script_export_port(
        &self,
    ) -> Option<Arc<dyn infiltrator_ports::script_export::ScriptExportPort>> {
        // DUAL-10-12: no native file dialog ships with this desktop product, so
        // the adapter writes into a host-owned `exports/` directory next to the
        // configs and reports the real path. Without a resolvable config dir
        // the port is omitted and every export is a typed unsupported.
        let config_dir = self.config_path.parent()?.to_path_buf();
        Some(Arc::new(
            crate::script_export::DesktopScriptExportPort::new(config_dir),
        ))
    }
}

fn map_stream<T, U>(
    receiver: tokio::sync::mpsc::UnboundedReceiver<mihomo_api::client::StreamEvent<T>>,
    map_item: fn(T) -> U,
) -> BoxStream<'static, RuntimeStreamEvent<U>>
where
    T: Send + 'static,
    U: Send + 'static,
{
    Box::pin(futures_util::stream::unfold(
        receiver,
        move |mut receiver| async move {
            let event = receiver.recv().await?;
            let event = match event {
                mihomo_api::client::StreamEvent::Connecting => RuntimeStreamEvent::Connecting,
                mihomo_api::client::StreamEvent::Connected => RuntimeStreamEvent::Connected,
                mihomo_api::client::StreamEvent::Item(item) => {
                    RuntimeStreamEvent::Item(map_item(item))
                }
                mihomo_api::client::StreamEvent::Reconnecting(error) => {
                    RuntimeStreamEvent::Reconnecting(error)
                }
                mihomo_api::client::StreamEvent::Failed(error) => RuntimeStreamEvent::Failed(error),
            };
            Some((event, receiver))
        },
    ))
}
