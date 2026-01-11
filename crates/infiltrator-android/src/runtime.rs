use async_trait::async_trait;
use mihomo_api::Result;
pub use mihomo_platform::AndroidBridge;
use mihomo_platform::{CoreController, CredentialStore, DataDirProvider};
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

#[async_trait]
impl<B> CoreController for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn start(&self) -> Result<()> {
        self.bridge.core_start().await
    }

    async fn stop(&self) -> Result<()> {
        self.bridge.core_stop().await
    }

    async fn is_running(&self) -> bool {
        self.bridge
            .core_is_running()
            .await
            .unwrap_or_else(|err| {
                log::warn!("android core is_running failed: {err}");
                false
            })
    }

    fn controller_url(&self) -> Option<String> {
        self.bridge.core_controller_url()
    }

    async fn pid(&self) -> Option<u32> {
        None
    }
}

#[async_trait]
impl<B> CredentialStore for AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.bridge.credential_get(service, key).await
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.bridge.credential_set(service, key, value).await
    }

    async fn delete(&self, service: &str, key: &str) -> Result<()> {
        self.bridge.credential_delete(service, key).await
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

pub fn android_bridge_adapter<B>(bridge: B) -> AndroidBridgeAdapter<B>
where
    B: AndroidBridge,
{
    AndroidBridgeAdapter::new(bridge)
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

    pub fn controller(&self) -> &dyn CoreController {
        &self.adapter
    }

    pub fn credential_store(&self) -> &dyn CredentialStore {
        &self.adapter
    }

    pub fn data_dirs(&self) -> &dyn DataDirProvider {
        &self.adapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
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
        assert!(!adapter.is_running().await);
        adapter.start().await.expect("start ok");
        assert!(adapter.is_running().await);
        adapter.stop().await.expect("stop ok");
        assert!(!adapter.is_running().await);
    }

    #[tokio::test]
    async fn test_adapter_credentials() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let value = adapter
            .get("svc", "key")
            .await
            .expect("get ok");
        assert!(value.is_none());
        adapter
            .set("svc", "key", "secret")
            .await
            .expect("set ok");
        let value = adapter
            .get("svc", "key")
            .await
            .expect("get ok");
        assert_eq!(value, Some("secret".to_string()));
        adapter
            .delete("svc", "key")
            .await
            .expect("delete ok");
        let value = adapter
            .get("svc", "key")
            .await
            .expect("get ok");
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_runtime_accessors() {
        let adapter = AndroidBridgeAdapter::new(TestBridge::new());
        let runtime = AndroidRuntime::new(adapter);
        assert_eq!(
            runtime.controller().controller_url(),
            Some("http://127.0.0.1:9090".to_string())
        );
        assert_eq!(
            runtime.data_dirs().data_dir(),
            Some(PathBuf::from("data"))
        );
        assert_eq!(
            runtime.data_dirs().cache_dir(),
            Some(PathBuf::from("cache"))
        );
    }
}
