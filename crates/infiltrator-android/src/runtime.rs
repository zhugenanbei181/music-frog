use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt, CoreLaunchSpec,
};
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::mtu::PhysicalMtuSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::android_core_process_recovery::AndroidCoreProcessRecoveryPort;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_process::{
    CoreProcess, CoreProcessStartAdmission, CoreProcessStartContext,
};
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
#[cfg(test)]
use mihomo_api::error::Result;
use mihomo_platform::android_bridge::AndroidBridge;
#[cfg(test)]
use mihomo_platform::android_bridge::{CoreProcessStartReceipt, CoreProcessStopReceipt};
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

#[derive(Clone)]
pub struct AndroidBridgeAdapter<B> {
    bridge: B,
}

impl<B> AndroidBridgeAdapter<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }
}

#[async_trait]
pub(crate) trait CoreLaunchSpecSource: Send + Sync {
    async fn resolve(&self) -> result::Result<CoreLaunchSpec, PortError>;
}

#[derive(Clone)]
pub(crate) struct AndroidCoreProcessAdapter<B> {
    engine_id: EngineId,
    bridge: B,
    launch_spec_source: Arc<dyn CoreLaunchSpecSource>,
}

impl<B> AndroidCoreProcessAdapter<B> {
    pub(crate) fn new(
        engine_id: EngineId,
        bridge: B,
        launch_spec_source: Arc<dyn CoreLaunchSpecSource>,
    ) -> Self {
        Self {
            engine_id,
            bridge,
            launch_spec_source,
        }
    }
}

pub fn android_bridge_adapter<B>(bridge: B) -> AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    AndroidBridgeAdapter::new(bridge)
}

#[async_trait]
impl<B> OfflineStartupPort for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn validate_offline_startup(&self) -> result::Result<OfflineStartupSnapshot, PortError> {
        // The Android host owns the APK ABI binary and sandbox paths. Rust
        // consumes its typed local proof and never performs a remote probe.
        Ok(self.bridge.offline_startup_snapshot())
    }
}

#[async_trait]
impl<B> MtuProbePort for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn probe_physical_mtu(&self) -> result::Result<PhysicalMtuSnapshot, PortError> {
        self.bridge
            .physical_mtu()
            .map(|mtu| PhysicalMtuSnapshot {
                interface: "android-active-link".to_owned(),
                mtu,
            })
            .ok_or_else(|| {
                PortError::unsupported(
                    Capability::Tun,
                    "Android native bridge does not expose physical MTU",
                )
            })
    }
}

#[async_trait]
impl<B> AndroidCoreProcessRecoveryPort for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn observe(&self) -> result::Result<AndroidCoreProcessObservation, PortError> {
        self.bridge
            .core_process_observation()
            .await
            .map_err(map_port_error)
    }

    async fn consume_exited(
        &self,
        context: &CoreProcessStartContext,
    ) -> result::Result<AndroidCoreProcessRecoveryReceipt, PortError> {
        self.bridge
            .core_process_recovery_completed(context)
            .await
            .map_err(map_port_error)
    }
}

fn map_port_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

#[async_trait]
impl<B> CoreProcess for AndroidCoreProcessAdapter<B>
where
    B: AndroidBridge,
{
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            self.engine_id.clone(),
            CoreInstanceId::new("android-active").ok()?,
            None,
        ))
    }

    async fn prepare_start(
        &self,
        context: &CoreProcessStartContext,
    ) -> result::Result<CoreProcessStartAdmission, PortError> {
        let expected_identity = self
            .core_identity()
            .ok_or_else(|| PortError::Failed("Android Core identity is unavailable".to_owned()))?;
        validate_android_start_context(context, &expected_identity)?;
        match self
            .bridge
            .core_process_observation()
            .await
            .map_err(map_port_error)?
        {
            AndroidCoreProcessObservation::NeverStarted => {
                Ok(CoreProcessStartAdmission::LaunchNewProcess)
            }
            AndroidCoreProcessObservation::Running { owned } if owned.context == *context => {
                Ok(CoreProcessStartAdmission::ReuseExactOwnedProcess)
            }
            AndroidCoreProcessObservation::Running { .. } => Err(PortError::Failed(
                "Android Core child belongs to a different Core session".to_owned(),
            )),
            AndroidCoreProcessObservation::Exited { .. } => Err(PortError::Failed(
                "Android Core child exit awaits owner recovery".to_owned(),
            )),
            AndroidCoreProcessObservation::Unavailable { reason } => Err(PortError::Failed(
                format!("Android Core process observation is unavailable: {reason:?}"),
            )),
        }
    }

    async fn start(&self, context: CoreProcessStartContext) -> result::Result<(), PortError> {
        match self.prepare_start(&context).await? {
            CoreProcessStartAdmission::LaunchNewProcess => {}
            CoreProcessStartAdmission::ReuseExactOwnedProcess => return Ok(()),
        }
        let launch_spec = self.launch_spec_source.resolve().await?;
        if launch_spec.configuration_identity.engine_id != self.engine_id {
            return Err(PortError::Failed(
                "Android Core launch specification engine differs from the active owner identity"
                    .to_owned(),
            ));
        }
        self.bridge
            .core_start(&launch_spec, &context)
            .await
            .map_err(map_port_error)?
            .into_port_result()
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        self.bridge
            .core_stop()
            .await
            .map_err(map_port_error)?
            .into_port_result()
    }

    async fn status(&self) -> result::Result<CoreLifecycle, PortError> {
        match self
            .bridge
            .core_process_observation()
            .await
            .map_err(map_port_error)?
        {
            AndroidCoreProcessObservation::NeverStarted
            | AndroidCoreProcessObservation::Exited { .. } => Ok(CoreLifecycle::Stopped),
            AndroidCoreProcessObservation::Running { .. } => Ok(CoreLifecycle::Running),
            AndroidCoreProcessObservation::Unavailable { reason } => Err(PortError::Failed(
                format!("Android Core process observation is unavailable: {reason:?}"),
            )),
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

fn validate_android_start_context(
    context: &CoreProcessStartContext,
    expected_identity: &CoreIdentity,
) -> result::Result<(), PortError> {
    match context {
        CoreProcessStartContext::Managed {
            identity,
            generation,
            session_token,
        } if identity.same_instance(expected_identity)
            && generation.value() > 0
            && session_token.is_valid() =>
        {
            Ok(())
        }
        CoreProcessStartContext::Managed { .. } => Err(PortError::Failed(
            "Core start context targets a different Android process".to_owned(),
        )),
        CoreProcessStartContext::Standalone { .. } => Err(PortError::Failed(
            "Android Core start requires a managed owner session".to_owned(),
        )),
    }
}

#[async_trait]
impl<B> SecureStore for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn get(&self, namespace: &str, key: &str) -> result::Result<Option<String>, PortError> {
        self.bridge
            .credential_get(namespace, key)
            .await
            .map_err(map_port_error)
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        self.bridge
            .credential_set(namespace, key, value)
            .await
            .map_err(map_port_error)
    }

    async fn delete(&self, namespace: &str, key: &str) -> result::Result<(), PortError> {
        self.bridge
            .credential_delete(namespace, key)
            .await
            .map_err(map_port_error)
    }
}

impl<B> DataDirProvider for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    fn data_dir(&self) -> Option<PathBuf> {
        self.bridge.data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.bridge.cache_dir()
    }
}

impl<B> CapabilityProvider for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    fn host_kind(&self) -> HostKind {
        HostKind::Android
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Android,
            1,
            vec![
                CapabilityStatus {
                    capability: Capability::CoreLifecycle,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Tun,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::SystemProxy,
                    availability: Availability::Unsupported {
                        reason: "Android VpnService owns proxy routing".to_string(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::LanAccessControl,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Ipv6Routing,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::UwpLoopback,
                    availability: Availability::Unsupported {
                        reason: "Windows AppContainer loopback is not available on Android"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::PacService,
                    availability: Availability::Unsupported {
                        reason: "Android uses VpnService instead of desktop PAC injection"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::NetworkRoaming,
                    availability: Availability::Unsupported {
                        reason: "Android native VpnService route callbacks are not exposed"
                            .to_owned(),
                    },
                },
                CapabilityStatus {
                    capability: Capability::VpnService,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::Logs,
                    availability: Availability::Supported,
                },
                CapabilityStatus {
                    capability: Capability::CoreVersionInstall,
                    availability: Availability::Unsupported {
                        reason: "core binaries are delivered with the APK ABI".to_string(),
                    },
                },
            ],
        )
    }
}

pub struct AndroidRuntime<B>
where
    B: AndroidBridge,
{
    adapter: AndroidBridgeAdapter<B>,
}

impl<B> AndroidRuntime<B>
where
    B: AndroidBridge,
{
    pub fn new(adapter: AndroidBridgeAdapter<B>) -> Self {
        Self { adapter }
    }

    pub fn credential_store(&self) -> &dyn SecureStore {
        &self.adapter
    }

    pub fn data_dirs(&self) -> &dyn DataDirProvider {
        &self.adapter
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
