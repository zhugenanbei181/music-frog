//! Host port trait implementations for [`MihomoRuntime`]: the runtime
//! gateway, profile-owner and endpoint ports, and the host adapter composition.

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
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_core::apply::failure_from_apply_error;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use infiltrator_ports::connection::ConnectionPort;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::dns_latency::DnsLatencyProbePort;
use infiltrator_ports::dns_leak::DnsLeakProbePort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::mini_hud_window::MiniHudWindowPort;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::outbound::OutboundPort;
use infiltrator_ports::pac::PacServicePort;
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCommitEffect, ProfileConfigurationTransactionOutcome,
    ProfileConfigurationTransactionRequest,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::resources::{CoreResourcePort, CpuResourceObserver};
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_configuration::RuntimeConfigurationPorts;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::runtime_http_proxy_endpoint::{
    RuntimeHttpProxyEndpoint, RuntimeHttpProxyEndpointPort,
};
use infiltrator_ports::runtime_stream::RuntimeStreamPort;
use infiltrator_ports::script_export::ScriptExportPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::stun_probe::StunEgressProbePort;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use mihomo_api::resource_port::MihomoRuntimeResourcePort;
use mihomo_api::runtime_configuration::{MihomoRuntimeConfigurationAdapter, RuntimeScopeResolver};
use mihomo_api::runtime_stream::MihomoRuntimeStreamPort;
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[async_trait::async_trait]
impl RuntimeGateway for MihomoRuntime {
    fn current_core_scope(&self) -> Result<CoreScope, PortError> {
        active_core_scope(self)
    }

    fn configuration_ports(&self) -> RuntimeConfigurationPorts {
        let application = self.application.clone();
        let scope_resolver: RuntimeScopeResolver =
            Arc::new(move || active_core_scope_for_application(&application));
        MihomoRuntimeConfigurationAdapter::ports(self.client.clone(), Some(scope_resolver))
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

    fn connection_port(&self) -> Arc<dyn ConnectionPort> {
        self.runtime_gateway.connection_port()
    }

    fn runtime_stream_port(&self) -> Arc<dyn RuntimeStreamPort> {
        let application = self.application.clone();
        let resolver: RuntimeScopeResolver =
            Arc::new(move || active_core_scope_for_application(&application));
        Arc::new(MihomoRuntimeStreamPort::new(
            self.client.clone(),
            Some(resolver),
        ))
    }

    fn resource_port(&self) -> Arc<dyn CoreResourcePort> {
        let application = self.application.clone();
        let resolver: RuntimeScopeResolver =
            Arc::new(move || active_core_scope_for_application(&application));
        let cpu_observer: Arc<dyn CpuResourceObserver> = Arc::new(DesktopCpuResourceObserver {
            service_manager: self.service_manager.clone(),
        });
        Arc::new(
            MihomoRuntimeResourcePort::new(self.client.clone(), Some(resolver))
                .with_cpu_observer(cpu_observer),
        )
    }
}

struct DesktopCpuResourceObserver {
    service_manager: crate::service::ServiceManager,
}

#[async_trait::async_trait]
impl CpuResourceObserver for DesktopCpuResourceObserver {
    async fn observe_cpu_percent(&self) -> ResourceAvailability<f32> {
        let pid = match self.service_manager.status().await {
            Ok(ServiceStatus::Running(pid)) => pid,
            Ok(ServiceStatus::Stopped) => return ResourceAvailability::Unsupported,
            Err(error) => {
                return ResourceAvailability::Failed(Failure::from(PortError::Io(
                    error.to_string(),
                )));
            }
        };
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        match system.process(Pid::from_u32(pid)) {
            Some(process) => ResourceAvailability::Available(process.cpu_usage()),
            None => ResourceAvailability::Unsupported,
        }
    }
}

#[async_trait::async_trait]
impl ActiveProfileReferenceApplier for MihomoRuntime {
    fn engine_id(&self) -> Option<EngineId> {
        CoreLifecyclePort::core_identity(self.application.as_ref()).map(|identity| identity.engine)
    }

    fn core_lifecycle(&self) -> CoreLifecycle {
        CoreLifecyclePort::lifecycle(self.application.as_ref())
    }

    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        self.application
            .active_profile_reference_write_authorization()
            .map_err(PortError::Rejected)
    }

    async fn apply_active_profile_workspace(
        &self,
        expected_reference: &ActiveProfileReference,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let runtime_engine = self.engine_id().ok_or_else(|| {
            PortError::Failed("active Core engine identity is unavailable".into())
        })?;
        let store_engine = ProfileStore::engine_id(self.config_manager.engine_profile_store());
        if expected.document.engine_id != runtime_engine
            || expected.document.engine_id != store_engine
            || expected_reference.engine_id != expected.document.engine_id
            || expected_reference.profile_id != expected.document.profile_id
        {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "profile engine does not match the active Core owner or profile store",
            ));
        }
        let outcome = self
            .configuration_transaction
            .transact(ProfileConfigurationTransactionRequest {
                expected_reference: expected_reference.clone(),
                expected: expected.clone(),
                update: update.clone(),
                authorization,
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
    async fn apply_active_profile_reference(
        &self,
        expected: &ActiveProfileReference,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError> {
        MihomoRuntime::apply_active_profile_reference(self, expected, strategy)
            .await
            .map(|outcome| outcome.generation)
            .map_err(|error| PortError::Rejected(failure_from_apply_error(error)))
    }
}

#[async_trait::async_trait]
impl RuntimeHttpProxyEndpointPort for MihomoRuntime {
    async fn observe_http_proxy_endpoint(&self) -> Result<RuntimeHttpProxyEndpoint, PortError> {
        MihomoRuntime::observe_http_proxy_endpoint(self)
            .await
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

    fn core_scope_source(&self) -> Arc<dyn CoreScopeSource> {
        Arc::new(self.application.as_ref().clone())
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

fn active_core_scope(runtime: &MihomoRuntime) -> Result<CoreScope, PortError> {
    active_core_scope_for_application(&runtime.application)
}

fn active_core_scope_for_application(
    application: &CoreApplication,
) -> Result<CoreScope, PortError> {
    let lifecycle = application.snapshot().lifecycle_snapshot();
    match &lifecycle.lifecycle {
        CoreLifecycle::Running | CoreLifecycle::Ready => lifecycle.scope().ok_or_else(|| {
            PortError::Failed("active Core identity is unavailable for controller operation".into())
        }),
        _ => Err(PortError::Failed(
            "controller operation requires a running Core".to_string(),
        )),
    }
}
