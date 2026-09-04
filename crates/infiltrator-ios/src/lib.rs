//! iOS host adapter boundary for the 0.30 architecture.
//!
//! iOS is deliberately a separate host from Android: NetworkExtension
//! providers, app-extension lifetimes, entitlements, and Swift callbacks do
//! not have the same process model as Android `VpnService`. This crate keeps
//! those platform APIs outside the application and accepts a bridge supplied
//! by the native host.

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
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Native iOS code implements this bridge around the app/extension lifecycle.
/// It returns runtime-neutral `PortError` values so no Swift, Objective-C or
/// NetworkExtension type crosses into the application layer.
#[async_trait]
pub trait IosBridge: Send + Sync {
    async fn core_start(&self) -> Result<(), PortError>;
    async fn core_stop(&self) -> Result<(), PortError>;
    async fn core_is_running(&self) -> Result<bool, PortError>;
    fn core_controller_url(&self) -> Option<String>;

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>, PortError>;
    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<(), PortError>;
    async fn credential_delete(&self, service: &str, key: &str) -> Result<(), PortError>;

    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf>;
}

/// Host adapter shared by the Rust application and an injected native iOS
/// bridge. Cloning the adapter only clones the bridge handle.
#[derive(Clone)]
pub struct IosHostAdapter {
    bridge: Arc<dyn IosBridge>,
}

impl IosHostAdapter {
    pub fn new<B>(bridge: B) -> Self
    where
        B: IosBridge + 'static,
    {
        Self {
            bridge: Arc::new(bridge),
        }
    }

    pub fn from_arc(bridge: Arc<dyn IosBridge>) -> Self {
        Self { bridge }
    }

    pub fn controller_url(&self) -> Option<String> {
        self.bridge.core_controller_url()
    }
}

#[async_trait]
impl CoreProcess for IosHostAdapter {
    async fn start(&self) -> Result<(), PortError> {
        self.bridge.core_start().await
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.bridge.core_stop().await
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(if self.bridge.core_is_running().await? {
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
impl SecureStore for IosHostAdapter {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>, PortError> {
        self.bridge.credential_get(service, key).await
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.bridge.credential_set(service, key, value).await
    }

    async fn delete(&self, service: &str, key: &str) -> Result<(), PortError> {
        self.bridge.credential_delete(service, key).await
    }
}

impl DataDirProvider for IosHostAdapter {
    fn data_dir(&self) -> Option<PathBuf> {
        self.bridge.data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.bridge.cache_dir()
    }
}

impl CapabilityProvider for IosHostAdapter {
    fn host_kind(&self) -> HostKind {
        HostKind::Ios
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        ios_capabilities()
    }
}

/// Conservative iOS capability declaration. NetworkExtension-backed TUN and
/// app routing are not reported as supported until the native bridge proves
/// the entitlement and extension lifecycle are installed.
pub fn ios_capabilities() -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        HostKind::Ios,
        1,
        vec![
            status(Capability::CoreLifecycle, Availability::Supported),
            status(
                Capability::SystemProxy,
                unsupported("iOS has no global proxy API"),
            ),
            status(
                Capability::Autostart,
                unsupported("iOS controls app launch"),
            ),
            status(
                Capability::CoreVersionInstall,
                unsupported("core delivery is controlled by the signed app bundle"),
            ),
            status(
                Capability::Tun,
                unsupported("NetworkExtension bridge not installed"),
            ),
            status(
                Capability::AppRouting,
                unsupported("NetworkExtension bridge not installed"),
            ),
        ],
    )
}

fn status(capability: Capability, availability: Availability) -> CapabilityStatus {
    CapabilityStatus {
        capability,
        availability,
    }
}

fn unsupported(reason: &'static str) -> Availability {
    Availability::Unsupported {
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_ports::core_process::CoreProcess;
    use std::sync::Mutex;

    struct FakeBridge {
        running: Mutex<bool>,
    }

    #[async_trait]
    impl IosBridge for FakeBridge {
        async fn core_start(&self) -> Result<(), PortError> {
            *self.running.lock().expect("running lock") = true;
            Ok(())
        }

        async fn core_stop(&self) -> Result<(), PortError> {
            *self.running.lock().expect("running lock") = false;
            Ok(())
        }

        async fn core_is_running(&self) -> Result<bool, PortError> {
            Ok(*self.running.lock().expect("running lock"))
        }

        fn core_controller_url(&self) -> Option<String> {
            Some("http://127.0.0.1:9090".to_owned())
        }

        async fn credential_get(
            &self,
            _service: &str,
            _key: &str,
        ) -> Result<Option<String>, PortError> {
            Ok(None)
        }

        async fn credential_set(
            &self,
            _service: &str,
            _key: &str,
            _value: &str,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn credential_delete(&self, _service: &str, _key: &str) -> Result<(), PortError> {
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            None
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            None
        }
    }

    #[tokio::test]
    async fn adapter_maps_native_lifecycle_to_the_port() {
        let adapter = IosHostAdapter::new(FakeBridge {
            running: Mutex::new(false),
        });
        assert_eq!(adapter.status().await.unwrap(), CoreLifecycle::Stopped);
        adapter.start().await.unwrap();
        assert_eq!(adapter.status().await.unwrap(), CoreLifecycle::Running);
        adapter.stop().await.unwrap();
        assert_eq!(adapter.status().await.unwrap(), CoreLifecycle::Stopped);
        assert_eq!(adapter.host_kind(), HostKind::Ios);
    }

    #[test]
    fn capabilities_are_conservative_until_network_extension_is_wired() {
        let capabilities = ios_capabilities();
        assert!(capabilities.supports(Capability::CoreLifecycle));
        assert!(!capabilities.supports(Capability::Tun));
        assert!(matches!(
            capabilities.availability(Capability::Tun),
            Availability::Unsupported { .. }
        ));
    }
}
