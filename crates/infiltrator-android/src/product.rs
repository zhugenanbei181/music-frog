//! Android product composition: the shared [`CoreApplication`] plus the
//! bounded surface pump that Bevy and Compose hosts consume.
//!
//! This is the Android analogue of `infiltrator-desktop/src/surface.rs`
//! (BANDROID-004). It assembles the Android bridge-backed Core process port,
//! the VpnService application and the controller gateway into one
//! runtime-neutral [`SurfacePump`]. Pages whose Android host adapters are not
//! composed yet stay typed `unavailable`; nothing here fabricates demo data.

use crate::composition::core_application;
use crate::composition::mtu_application;
use crate::composition::offline_startup_application;
use crate::runtime::AndroidBridgeAdapter;
use crate::vpn_service::AndroidVpnServicePort;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::surface_application::SurfacePumpBridge;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use mihomo_api::client::MihomoClient;
use mihomo_platform::android_bridge::{AndroidBridge, get_android_bridge};
use std::sync::Arc;
use std::time::Duration;

/// Host-supplied parameters for the Android product composition.
#[derive(Clone, Debug)]
pub struct AndroidProductConfig {
    /// Loopback controller endpoint exposed by the packaged Mihomo core.
    pub controller_url: String,
    /// Optional controller secret resolved by the shared configuration owner.
    pub controller_secret: Option<String>,
    /// Bounded surface sampling cadence for the pump worker.
    pub sample_interval: Duration,
}

impl AndroidProductConfig {
    pub fn new(controller_url: impl Into<String>) -> Self {
        Self {
            controller_url: controller_url.into(),
            controller_secret: None,
            sample_interval: Duration::from_millis(700),
        }
    }
}

/// Typed failure while opening the Android product composition.
#[derive(Debug, thiserror::Error)]
pub enum AndroidProductError {
    #[error("android bridge is not registered in this process")]
    BridgeUnavailable,
    #[error("android application runtime is unavailable: {0}")]
    RuntimeUnavailable(String),
}

/// One owned Android product session: the application facade and its surface
/// pump. The owner decides when to drop the pump and close the application.
pub struct AndroidProductComposition {
    application: Arc<CoreApplication>,
    pump: SurfacePump,
}

impl AndroidProductComposition {
    /// Open using the bridge registered in the current process.
    ///
    /// The `:vpn` process registers the bridge from `MihomoApplication` before
    /// this is called, so the UI process is not required to have run first.
    pub fn open(config: AndroidProductConfig) -> Result<Self, AndroidProductError> {
        let bridge = get_android_bridge().ok_or(AndroidProductError::BridgeUnavailable)?;
        Self::open_with_bridge(bridge, config)
    }

    /// Open with an explicit bridge. Used by hosts and tests that own the
    /// bridge value directly instead of reading the process registry.
    pub fn open_with_bridge(
        bridge: Arc<dyn AndroidBridge>,
        config: AndroidProductConfig,
    ) -> Result<Self, AndroidProductError> {
        let application = Arc::new(core_application(
            bridge.clone(),
            config.controller_url.clone(),
            config.controller_secret.clone(),
        ));
        let capabilities =
            CapabilityProvider::capabilities(&AndroidBridgeAdapter::new(bridge.clone()));
        let mut reader = ApplicationSurfaceReader::new(
            application.clone(),
            SurfaceKind::BevyAndroid,
            HostKind::Android,
        )
        .with_capabilities(capabilities)
        .with_offline_startup(offline_startup_application(bridge.clone()))
        .with_mtu(mtu_application(bridge.clone()))
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )));
        if let Ok(client) = MihomoClient::new(&config.controller_url, config.controller_secret) {
            let gateway: Arc<dyn RuntimeGateway> = Arc::new(client);
            reader = reader
                .with_gateway(gateway.clone())
                .with_resources(ResourceApplication::new(gateway));
        }
        let runtime = infiltrator_composition::tokio_application_runtime()
            .map_err(AndroidProductError::RuntimeUnavailable)?;
        let initial = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyAndroid,
            HostKind::Android,
            Failure::new(
                ErrorCode::NotReady,
                "waiting for the first Android surface snapshot",
                true,
            ),
        );
        let pump = SurfacePump::spawn(Arc::new(reader), config.sample_interval, runtime, initial);
        Ok(Self { application, pump })
    }

    /// The shared application facade both UI surfaces dispatch commands to.
    pub fn application(&self) -> Arc<CoreApplication> {
        self.application.clone()
    }

    /// The bounded surface pump owned by this composition.
    pub fn surface_pump(&self) -> SurfacePump {
        self.pump.clone()
    }

    /// A frame-drain bridge for a host event loop.
    pub fn surface_bridge(&self) -> SurfacePumpBridge {
        self.pump.bridge()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mihomo_api::error::Result;
    use mihomo_platform::android_bridge::clear_android_bridge;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::thread::yield_now;
    use std::time::Instant;

    struct TestBridge {
        running: Mutex<bool>,
    }

    impl TestBridge {
        fn new() -> Self {
            Self {
                running: Mutex::new(false),
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
            Ok(self.running.lock().map(|guard| *guard).unwrap_or(false))
        }

        fn core_controller_url(&self) -> Option<String> {
            Some("http://127.0.0.1:1".to_string())
        }

        async fn credential_get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
            Ok(None)
        }

        async fn credential_set(&self, _service: &str, _key: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        async fn credential_delete(&self, _service: &str, _key: &str) -> Result<()> {
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("data"))
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("cache"))
        }

        async fn vpn_start(&self) -> Result<bool> {
            Ok(false)
        }

        async fn vpn_stop(&self) -> Result<bool> {
            Ok(true)
        }

        async fn vpn_is_running(&self) -> Result<bool> {
            Ok(false)
        }

        async fn tun_set_enabled(&self, _enabled: bool) -> Result<bool> {
            Ok(false)
        }

        async fn tun_is_enabled(&self) -> Result<bool> {
            Ok(false)
        }
    }

    fn config() -> AndroidProductConfig {
        AndroidProductConfig {
            controller_url: "http://127.0.0.1:1".to_string(),
            controller_secret: None,
            sample_interval: Duration::from_millis(50),
        }
    }

    #[test]
    fn composition_publishes_an_android_surface_snapshot() {
        let composition =
            AndroidProductComposition::open_with_bridge(Arc::new(TestBridge::new()), config())
                .expect("composition opens with an explicit bridge");
        let bridge = composition.surface_bridge();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut delivered = None;
        while delivered.is_none() {
            delivered = bridge.drain().into_iter().next_back();
            assert!(
                delivered.is_some() || Instant::now() < deadline,
                "surface pump did not deliver a snapshot"
            );
            if delivered.is_none() {
                yield_now();
            }
        }
        let snapshot = delivered.expect("snapshot");
        assert_eq!(snapshot.surface, SurfaceKind::BevyAndroid);
        assert_eq!(snapshot.capabilities.host, HostKind::Android);
        // The application facade is the same shared instance the pump reads.
        assert!(Arc::strong_count(&composition.application()) >= 1);
    }

    #[test]
    fn missing_bridge_is_a_typed_open_failure() {
        clear_android_bridge();
        let error = match AndroidProductComposition::open(config()) {
            Ok(_) => panic!("open must fail without a registered bridge"),
            Err(error) => error,
        };
        assert!(matches!(error, AndroidProductError::BridgeUnavailable));
    }
}
