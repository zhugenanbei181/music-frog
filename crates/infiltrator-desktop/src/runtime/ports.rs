//! Host port trait implementations for [`MihomoRuntime`]: the runtime
//! gateway, the managed-runtime lifecycle and the host adapter composition.

use super::MihomoRuntime;
use crate::certificate_authority::DesktopCertificateAuthority;
use crate::mini_hud_window::DesktopMiniHudWindow;
use crate::mtu::DesktopMtuProbe;
use crate::rule_provider_cache::DesktopRuleProviderCache;
use crate::script_export::DesktopScriptExportPort;
use crate::service::ServiceStatus;
use crate::system_dns_cache::DesktopSystemDnsCache;
use crate::system_proxy::DesktopSystemProxy;
use crate::tun_service::{ServiceModeStatus, TunServiceManager};
use futures_util::StreamExt;
use futures_util::stream::{BoxStream, unfold};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
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
use infiltrator_ports::outbound::{OutboundPort, OutboundScopeSource};
use infiltrator_ports::pac::PacServicePort;
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionPort, ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::runtime_gateway::{
    ManagedRuntime, RuntimeGateway, RuntimeStream, RuntimeStreamEvent, ScopedRuntimeStream,
    ScopedRuntimeStreamEvent,
};
use infiltrator_ports::script_export::ScriptExportPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::stun_probe::StunEgressProbePort;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use mihomo_api::client::StreamEvent;
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::sync::mpsc::UnboundedReceiver;

#[async_trait::async_trait]
impl RuntimeGateway for MihomoRuntime {
    fn current_core_scope(&self) -> Result<CoreScope, PortError> {
        active_core_scope(self)
    }

    fn configuration_ports(&self) -> RuntimeConfigurationPorts {
        let application = self.application.clone();
        let scope_resolver: mihomo_api::runtime_configuration::RuntimeScopeResolver =
            Arc::new(move || active_core_scope_for_application(&application));
        mihomo_api::runtime_configuration::MihomoRuntimeConfigurationAdapter::ports(
            self.client.clone(),
            Some(scope_resolver),
        )
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

    async fn stream_logs_scoped(
        &self,
        requested_scope: CoreScope,
        level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        let opening_scope = active_core_scope(self)?;
        validate_log_open_scope(&requested_scope, &opening_scope)?;

        let stream = RuntimeGateway::stream_logs(self, level).await?;
        let stream_scope = active_core_scope(self)?;
        validate_log_post_open_scope(&requested_scope, &opening_scope, &stream_scope)?;

        let scoped = stream.map(move |event| ScopedRuntimeStreamEvent {
            scope: stream_scope.clone(),
            event,
        });
        Ok(Box::pin(scoped))
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
    fn engine_id(&self) -> Option<EngineId> {
        CoreLifecyclePort::core_identity(self.application.as_ref()).map(|identity| identity.engine)
    }

    async fn apply_profile_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let runtime_engine = self.engine_id().ok_or_else(|| {
            PortError::Failed("managed core engine identity is unavailable".into())
        })?;
        let store_engine = ProfileStore::engine_id(self.config_manager.as_ref());
        if expected.document.engine_id != runtime_engine
            || expected.document.engine_id != store_engine
        {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "profile engine does not match the managed runtime or profile store",
            ));
        }
        let active = ProfileStore::get_current(self.config_manager.as_ref()).await?;
        if active != expected.document.profile_id.as_str() {
            let guard = self.apply_guard.lock().await;
            let active = ProfileStore::get_current(self.config_manager.as_ref()).await?;
            if active != expected.document.profile_id.as_str() {
                return self
                    .config_manager
                    .compare_and_save_inactive_workspace(expected, update)
                    .await;
            }
            drop(guard);
        }
        let outcome = self
            .configuration_transaction
            .transact(ProfileConfigurationTransactionRequest {
                expected: expected.clone(),
                update: update.clone(),
                strategy,
            })
            .await?;
        match outcome {
            ProfileConfigurationTransactionOutcome::Committed {
                workspace,
                effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
            }
            | ProfileConfigurationTransactionOutcome::Committed {
                workspace,
                effect: ProfileConfigurationCommitEffect::StoredWhileStopped,
            }
            | ProfileConfigurationTransactionOutcome::Committed {
                workspace,
                effect: ProfileConfigurationCommitEffect::StoredWhileFailed,
            } => Ok(workspace),
            ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
                Err(PortError::Rejected(cause))
            }
            ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
                Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    format!("{}; rollback failed: {}", cause.message, rollback.message),
                    true,
                )))
            }
        }
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

    fn outbound_port(&self) -> Option<Arc<dyn OutboundPort>> {
        self.application.outbound_port()
    }

    fn outbound_scope_source(&self) -> Option<Arc<dyn OutboundScopeSource>> {
        Some(Arc::new(self.application.as_ref().clone()))
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

fn active_core_scope(runtime: &MihomoRuntime) -> Result<CoreScope, PortError> {
    active_core_scope_for_application(&runtime.application)
}

fn active_core_scope_for_application(
    application: &CoreApplication,
) -> Result<CoreScope, PortError> {
    let lifecycle = application.snapshot().lifecycle_snapshot();
    match &lifecycle.lifecycle {
        CoreLifecycle::Running | CoreLifecycle::Ready => lifecycle.scope().ok_or_else(|| {
            PortError::Failed("active core identity is unavailable for controller logs".into())
        }),
        _ => Err(PortError::Failed(
            "controller log stream requires a running core".to_string(),
        )),
    }
}

fn validate_log_open_scope(requested: &CoreScope, opening: &CoreScope) -> Result<(), PortError> {
    if opening == requested {
        Ok(())
    } else {
        Err(PortError::Failed(
            "controller log request does not match the active core scope".to_string(),
        ))
    }
}

fn validate_log_post_open_scope(
    requested: &CoreScope,
    opening: &CoreScope,
    observed: &CoreScope,
) -> Result<(), PortError> {
    if opening == requested && observed == requested && observed == opening {
        Ok(())
    } else {
        Err(PortError::Failed(
            "active core scope changed while opening the controller log stream".to_string(),
        ))
    }
}

#[cfg(test)]
mod log_scope_tests {
    use super::{validate_log_open_scope, validate_log_post_open_scope};
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use infiltrator_contract::session::SessionToken;
    use infiltrator_ports::error::PortError;

    fn scope(generation: u64, session: u128) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("desktop-main").expect("instance"),
                Some(CoreVersion::new("v1.19.32").expect("version")),
            ),
            generation: CoreGeneration::new(generation),
            session_token: Some(SessionToken::new(session)),
        }
    }

    #[test]
    fn scoped_log_stream_rejects_a_request_for_another_active_generation() {
        let requested = scope(7, 70);
        let active = scope(8, 80);

        let error = validate_log_open_scope(&requested, &active)
            .expect_err("mismatched scope must fail before opening the stream");

        assert!(matches!(error, PortError::Failed(_)));
        assert!(validate_log_post_open_scope(&requested, &active, &active).is_err());
    }

    #[test]
    fn scoped_log_stream_accepts_only_a_stable_scope_across_opening() {
        let requested = scope(7, 70);
        let opening = scope(7, 70);
        assert!(validate_log_open_scope(&requested, &opening).is_ok());
        assert!(validate_log_post_open_scope(&requested, &opening, &opening).is_ok());

        let restarted = scope(8, 80);
        assert!(matches!(
            validate_log_post_open_scope(&requested, &opening, &restarted),
            Err(PortError::Failed(_))
        ));
    }
}
