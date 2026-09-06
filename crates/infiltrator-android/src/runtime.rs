use async_trait::async_trait;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::Result;
use mihomo_platform::android_bridge::AndroidBridge;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AndroidBridgeAdapter<B> {
    bridge: B,
}

impl<B> AndroidBridgeAdapter<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }
}

impl<B> AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    pub async fn vpn_start(&self) -> Result<bool> {
        self.bridge.vpn_start().await
    }

    pub async fn vpn_stop(&self) -> Result<bool> {
        self.bridge.vpn_stop().await
    }

    pub async fn vpn_is_running(&self) -> Result<bool> {
        self.bridge.vpn_is_running().await
    }

    pub async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        self.bridge.tun_set_enabled(enabled).await
    }

    pub async fn tun_is_enabled(&self) -> Result<bool> {
        self.bridge.tun_is_enabled().await
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
    async fn validate_offline_startup(
        &self,
    ) -> std::result::Result<infiltrator_contract::offline_startup::OfflineStartupSnapshot, PortError>
    {
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
    async fn probe_physical_mtu(
        &self,
    ) -> std::result::Result<infiltrator_contract::mtu::PhysicalMtuSnapshot, PortError> {
        self.bridge
            .physical_mtu()
            .map(|mtu| infiltrator_contract::mtu::PhysicalMtuSnapshot {
                interface: "android-active-link".to_owned(),
                mtu,
            })
            .ok_or_else(|| {
                PortError::unsupported(
                    infiltrator_contract::capability::Capability::Tun,
                    "Android native bridge does not expose physical MTU",
                )
            })
    }
}

fn map_port_error(error: mihomo_api::error::MihomoError) -> PortError {
    match error {
        mihomo_api::error::MihomoError::Io(error) => PortError::Io(error.to_string()),
        mihomo_api::error::MihomoError::Http(error) => PortError::Network(error.to_string()),
        mihomo_api::error::MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        mihomo_api::error::MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

#[async_trait]
impl<B> CoreProcess for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn start(&self) -> std::result::Result<(), PortError> {
        self.bridge.core_start().await.map_err(map_port_error)
    }

    async fn stop(&self) -> std::result::Result<(), PortError> {
        self.bridge.core_stop().await.map_err(map_port_error)
    }

    async fn status(&self) -> std::result::Result<CoreLifecycle, PortError> {
        let running = self
            .bridge
            .core_is_running()
            .await
            .map_err(map_port_error)?;
        Ok(if running {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        self.bridge.core_controller_url()
    }
}

#[async_trait]
impl<B> SecureStore for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn get(
        &self,
        namespace: &str,
        key: &str,
    ) -> std::result::Result<Option<String>, PortError> {
        self.bridge
            .credential_get(namespace, key)
            .await
            .map_err(map_port_error)
    }

    async fn set(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> std::result::Result<(), PortError> {
        self.bridge
            .credential_set(namespace, key, value)
            .await
            .map_err(map_port_error)
    }

    async fn delete(&self, namespace: &str, key: &str) -> std::result::Result<(), PortError> {
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
                        reason: "Windows AppContainer loopback is not available on Android".to_owned(),
                    },
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

    pub fn controller(&self) -> &dyn CoreProcess {
        &self.adapter
    }

    pub fn credential_store(&self) -> &dyn SecureStore {
        &self.adapter
    }

    pub fn data_dirs(&self) -> &dyn DataDirProvider {
        &self.adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_application::core_application::{CoreApplication, ReadinessPolicy};
    use infiltrator_contract::command::{CommandIntent, CommandResult};
    use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
    use infiltrator_ports::core_process::CoreReadiness;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;

    struct TestBridge {
        running: Mutex<bool>,
        store: Mutex<HashMap<(String, String), String>>,
        controller: Option<String>,
        data_dir: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
    }

    impl TestBridge {
        fn new() -> Self {
            Self {
                running: Mutex::new(false),
                store: Mutex::new(HashMap::new()),
                controller: Some("http://127.0.0.1:9090".to_string()),
                data_dir: Some(PathBuf::from("data")),
                cache_dir: Some(PathBuf::from("cache")),
            }
        }
    }

    #[async_trait]
    impl AndroidBridge for TestBridge {
        async fn core_start(&self) -> Result<()> {
            if let Ok(mut guard) = self.running.lock() {
                *guard = true;
            }
            Ok(())
        }

        async fn core_stop(&self) -> Result<()> {
            if let Ok(mut guard) = self.running.lock() {
                *guard = false;
            }
            Ok(())
        }

        async fn core_is_running(&self) -> Result<bool> {
            Ok(self
                .running
                .lock()
                .ok()
                .map(|guard| *guard)
                .unwrap_or(false))
        }

        fn core_controller_url(&self) -> Option<String> {
            self.controller.clone()
        }

        async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
            Ok(self
                .store
                .lock()
                .ok()
                .and_then(|g| g.get(&(service.to_string(), key.to_string())).cloned()))
        }

        async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
            if let Ok(mut guard) = self.store.lock() {
                guard.insert((service.to_string(), key.to_string()), value.to_string());
            }
            Ok(())
        }

        async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
            if let Ok(mut guard) = self.store.lock() {
                guard.remove(&(service.to_string(), key.to_string()));
            }
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            self.data_dir.clone()
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            self.cache_dir.clone()
        }

        fn offline_startup_snapshot(
            &self,
        ) -> infiltrator_contract::offline_startup::OfflineStartupSnapshot {
            infiltrator_contract::offline_startup::OfflineStartupSnapshot::ready(
                infiltrator_contract::offline_startup::LocalAssetStatus::Available,
            )
        }

        async fn vpn_start(&self) -> Result<bool> {
            Ok(true)
        }

        async fn vpn_stop(&self) -> Result<bool> {
            Ok(true)
        }

        async fn vpn_is_running(&self) -> Result<bool> {
            Ok(false)
        }

        async fn tun_set_enabled(&self, _enabled: bool) -> Result<bool> {
            Ok(true)
        }

        async fn tun_is_enabled(&self) -> Result<bool> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn test_adapter_core_cycle() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Stopped
        ));
        adapter.start().await.expect("start ok");
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Running
        ));
        adapter.stop().await.expect("stop ok");
        assert!(matches!(
            CoreProcess::status(&adapter).await.unwrap(),
            CoreLifecycle::Stopped
        ));
    }

    #[tokio::test]
    async fn test_adapter_credentials() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert!(value.is_none());
        adapter.set("svc", "key", "secret").await.expect("set ok");
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert_eq!(value, Some("secret".to_string()));
        adapter.delete("svc", "key").await.expect("delete ok");
        let value = adapter.get("svc", "key").await.expect("get ok");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_runtime_accessors() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        assert!(adapter
            .capabilities()
            .supports(Capability::LanAccessControl));
        assert!(adapter
            .capabilities()
            .supports(Capability::Ipv6Routing));
        assert!(!adapter
            .capabilities()
            .supports(Capability::UwpLoopback));
        let runtime = AndroidRuntime::new(adapter);
        assert_eq!(
            runtime.controller().controller_endpoint(),
            Some("http://127.0.0.1:9090".to_string())
        );
        assert_eq!(runtime.data_dirs().data_dir(), Some(PathBuf::from("data")));
        assert_eq!(
            runtime.data_dirs().cache_dir(),
            Some(PathBuf::from("cache"))
        );
    }

    #[tokio::test]
    async fn android_host_exposes_offline_first_startup_evidence() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let snapshot = infiltrator_application::offline_startup_application::OfflineStartupApplication::new(
            Arc::new(adapter),
        )
        .snapshot()
        .await;
        assert!(snapshot.is_offline_startable());
        assert_eq!(
            snapshot.policy,
            infiltrator_contract::offline_startup::StartupNetworkPolicy::OfflineFirst
        );
        assert_eq!(
            snapshot.remote_dependency,
            infiltrator_contract::offline_startup::StartupRemoteDependency::Optional
        );
    }

    #[tokio::test]
    async fn android_host_keeps_mtu_probe_unsupported_without_native_link_metrics() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let snapshot = infiltrator_application::mtu_application::MtuApplication::new(Arc::new(
            adapter,
        ))
        .probe()
        .await;
        assert_eq!(
            snapshot.state,
            infiltrator_contract::mtu::MtuProbeState::Unsupported
        );
    }

    struct ReadyProbe;

    #[async_trait]
    impl CoreReadiness for ReadyProbe {
        async fn probe(&self) -> std::result::Result<String, PortError> {
            Ok("http://127.0.0.1:9090".to_string())
        }
    }

    #[tokio::test]
    async fn android_host_composition_preserves_hot_reload_session_identity() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let runtime = infiltrator_composition::tokio_application_runtime().expect("runtime");
        let app = CoreApplication::new_with_policy(
            Arc::new(adapter),
            Arc::new(ReadyProbe),
            ReadinessPolicy {
                timeout: std::time::Duration::from_secs(1),
                poll_interval: std::time::Duration::from_millis(1),
            },
            runtime,
        );

        assert!(matches!(
            app.execute(CommandIntent::StartCore).await,
            CommandResult::Completed { .. }
        ));
        let before = app.snapshot();
        let token = CoreLifecyclePort::begin_reload(&app).expect("begin Android reload");
        CoreLifecyclePort::complete_reload(&app, token).expect("complete Android reload");
        let after = app.snapshot();
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.session_token, before.session_token);
        assert_eq!(after.lifecycle, CoreLifecycle::Running);
    }
}
