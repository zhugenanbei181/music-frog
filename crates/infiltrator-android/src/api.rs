use crate::runtime::{AndroidBridge, AndroidBridgeAdapter};
use mihomo_api::Result;
use mihomo_platform::{CoreController, CredentialStore, DataDirProvider};

pub struct AndroidApi<B>
where
    B: AndroidBridge,
{
    adapter: AndroidBridgeAdapter<B>,
}

impl<B> AndroidApi<B>
where
    B: AndroidBridge,
{
    pub fn new(bridge: B) -> Self {
        Self {
            adapter: AndroidBridgeAdapter::new(bridge),
        }
    }

    pub fn controller_url(&self) -> Option<String> {
        self.adapter.controller_url()
    }

    pub async fn core_start(&self) -> Result<()> {
        self.adapter.start().await
    }

    pub async fn core_stop(&self) -> Result<()> {
        self.adapter.stop().await
    }

    pub async fn core_is_running(&self) -> bool {
        self.adapter.is_running().await
    }

    pub async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.adapter.get(service, key).await
    }

    pub async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.adapter.set(service, key, value).await
    }

    pub async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.adapter.delete(service, key).await
    }

    pub fn data_dir(&self) -> Option<std::path::PathBuf> {
        self.adapter.data_dir()
    }

    pub fn cache_dir(&self) -> Option<std::path::PathBuf> {
        self.adapter.cache_dir()
    }

    pub async fn vpn_start(&self) -> Result<bool> {
        self.adapter.vpn_start().await
    }

    pub async fn vpn_stop(&self) -> Result<bool> {
        self.adapter.vpn_stop().await
    }

    pub async fn vpn_is_running(&self) -> Result<bool> {
        self.adapter.vpn_is_running().await
    }

    pub async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        self.adapter.tun_set_enabled(enabled).await
    }

    pub async fn tun_is_enabled(&self) -> Result<bool> {
        self.adapter.tun_is_enabled().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::AndroidBridge;
    use std::path::PathBuf;
    use std::sync::Mutex;

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

    #[async_trait::async_trait]
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
            Some("http://127.0.0.1:9090".to_string())
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
    async fn test_android_api_core_flow() {
        let api = AndroidApi::new(TestBridge::new());
        assert!(!api.core_is_running().await);
        api.core_start().await.expect("start ok");
        assert!(api.core_is_running().await);
        api.core_stop().await.expect("stop ok");
        assert!(!api.core_is_running().await);
    }

    #[test]
    fn test_android_api_dirs() {
        let api = AndroidApi::new(TestBridge::new());
        assert_eq!(api.data_dir(), Some(PathBuf::from("data")));
        assert_eq!(api.cache_dir(), Some(PathBuf::from("cache")));
    }
}
