use crate::runtime::AndroidBridgeAdapter;
use mihomo_api::error::Result;
use mihomo_platform::android_bridge::AndroidBridge;
use mihomo_platform::traits::{CoreController, CredentialStore, DataDirProvider};

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
    use mihomo_platform::android_bridge::AndroidBridge;
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

    #[tokio::test]
    async fn test_android_api_vpn_toggles() {
        let api = AndroidApi::new(TestBridge::new());
        assert!(api.vpn_start().await.unwrap());
        assert!(api.vpn_stop().await.unwrap());
        assert!(!api.vpn_is_running().await.unwrap());
    }

    #[tokio::test]
    async fn test_android_api_tun_toggles() {
        let api = AndroidApi::new(TestBridge::new());
        assert!(api.tun_set_enabled(true).await.unwrap());
        assert!(!api.tun_is_enabled().await.unwrap());
    }

    struct FailingBridge;
    #[async_trait::async_trait]
    impl AndroidBridge for FailingBridge {
        async fn core_start(&self) -> Result<()> {
            Err(mihomo_api::error::MihomoError::Config("fail".into()))
        }
        async fn core_stop(&self) -> Result<()> {
            Ok(())
        }
        async fn core_is_running(&self) -> Result<bool> {
            Ok(false)
        }
        fn core_controller_url(&self) -> Option<String> {
            None
        }
        async fn credential_get(&self, _s: &str, _k: &str) -> Result<Option<String>> {
            Ok(None)
        }
        async fn credential_set(&self, _s: &str, _k: &str, _v: &str) -> Result<()> {
            Ok(())
        }
        async fn credential_delete(&self, _s: &str, _k: &str) -> Result<()> {
            Ok(())
        }
        fn data_dir(&self) -> Option<PathBuf> {
            None
        }
        fn cache_dir(&self) -> Option<PathBuf> {
            None
        }
        async fn vpn_start(&self) -> Result<bool> {
            Err(mihomo_api::error::MihomoError::Service("vpn fail".into()))
        }
        async fn vpn_stop(&self) -> Result<bool> {
            Ok(false)
        }
        async fn vpn_is_running(&self) -> Result<bool> {
            Ok(false)
        }
        async fn tun_set_enabled(&self, _e: bool) -> Result<bool> {
            Ok(false)
        }
        async fn tun_is_enabled(&self) -> Result<bool> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn test_android_api_error_propagation() {
        let api = AndroidApi::new(FailingBridge);
        let result = api.core_start().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fail"));

        let vpn_result = api.vpn_start().await;
        assert!(vpn_result.is_err());
        assert!(vpn_result.unwrap_err().to_string().contains("vpn fail"));
    }

    #[tokio::test]
    async fn test_credential_lifecycle() {
        struct CredBridge {
            store: Mutex<std::collections::HashMap<String, String>>,
        }
        #[async_trait::async_trait]
        impl AndroidBridge for CredBridge {
            async fn core_start(&self) -> Result<()> {
                Ok(())
            }
            async fn core_stop(&self) -> Result<()> {
                Ok(())
            }
            async fn core_is_running(&self) -> Result<bool> {
                Ok(false)
            }
            fn core_controller_url(&self) -> Option<String> {
                None
            }
            async fn credential_get(&self, _s: &str, k: &str) -> Result<Option<String>> {
                Ok(self.store.lock().unwrap().get(k).cloned())
            }
            async fn credential_set(&self, _s: &str, k: &str, v: &str) -> Result<()> {
                self.store
                    .lock()
                    .unwrap()
                    .insert(k.to_string(), v.to_string());
                Ok(())
            }
            async fn credential_delete(&self, _s: &str, k: &str) -> Result<()> {
                self.store.lock().unwrap().remove(k);
                Ok(())
            }
            fn data_dir(&self) -> Option<PathBuf> {
                None
            }
            fn cache_dir(&self) -> Option<PathBuf> {
                None
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
            async fn tun_set_enabled(&self, _e: bool) -> Result<bool> {
                Ok(false)
            }
            async fn tun_is_enabled(&self) -> Result<bool> {
                Ok(false)
            }
        }

        let api = AndroidApi::new(CredBridge {
            store: Mutex::new(Default::default()),
        });

        api.credential_set("svc", "key1", "val1").await.unwrap();
        assert_eq!(
            api.credential_get("svc", "key1").await.unwrap(),
            Some("val1".to_string())
        );
        api.credential_delete("svc", "key1").await.unwrap();
        assert_eq!(api.credential_get("svc", "key1").await.unwrap(), None);
    }

    #[test]
    fn test_exhaustive_ffi_status_codes() {
        use crate::ffi::{FfiErrorCode, FfiStatus};
        let codes = [
            FfiErrorCode::Ok,
            FfiErrorCode::InvalidState,
            FfiErrorCode::InvalidInput,
            FfiErrorCode::NotReady,
            FfiErrorCode::NotSupported,
            FfiErrorCode::Io,
            FfiErrorCode::Network,
            FfiErrorCode::Unknown,
        ];
        for code in codes {
            let status = FfiStatus::err(code, "msg");
            assert_eq!(status.code, code);
        }
    }

    #[tokio::test]
    async fn test_vpn_state_consistency() {
        let api = AndroidApi::new(TestBridge::new());
        assert!(!api.vpn_is_running().await.unwrap());
    }

    #[tokio::test]
    async fn test_core_restart_flow() {
        let api = AndroidApi::new(TestBridge::new());
        api.core_start().await.unwrap();
        assert!(api.core_is_running().await);
        api.core_stop().await.unwrap();
        assert!(!api.core_is_running().await);
        api.core_start().await.unwrap();
        assert!(api.core_is_running().await);
    }

    #[tokio::test]
    async fn test_credential_error_handling() {
        struct FailingCredBridge;
        #[async_trait::async_trait]
        impl AndroidBridge for FailingCredBridge {
            async fn core_start(&self) -> Result<()> {
                Ok(())
            }
            async fn core_stop(&self) -> Result<()> {
                Ok(())
            }
            async fn core_is_running(&self) -> Result<bool> {
                Ok(false)
            }
            fn core_controller_url(&self) -> Option<String> {
                None
            }
            async fn credential_get(&self, _s: &str, _k: &str) -> Result<Option<String>> {
                Err(mihomo_api::error::MihomoError::Config("mock error".into()))
            }
            async fn credential_set(&self, _s: &str, _k: &str, _v: &str) -> Result<()> {
                Ok(())
            }
            async fn credential_delete(&self, _s: &str, _k: &str) -> Result<()> {
                Ok(())
            }
            fn data_dir(&self) -> Option<PathBuf> {
                None
            }
            fn cache_dir(&self) -> Option<PathBuf> {
                None
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
            async fn tun_set_enabled(&self, _e: bool) -> Result<bool> {
                Ok(false)
            }
            async fn tun_is_enabled(&self) -> Result<bool> {
                Ok(false)
            }
        }
        let api = AndroidApi::new(FailingCredBridge);
        assert!(api.credential_get("s", "k").await.is_err());
    }

    #[test]
    fn test_ffi_status_factories() {
        use crate::ffi::FfiStatus;
        let s = FfiStatus::ok();
        assert!(s.message.is_none());

        let e = FfiStatus::err(crate::ffi::FfiErrorCode::Io, "io");
        assert_eq!(e.message, Some("io".into()));
    }

    #[tokio::test]
    async fn test_android_api_empty_dirs() {
        struct NoDirBridge;
        #[async_trait::async_trait]
        impl AndroidBridge for NoDirBridge {
            async fn core_start(&self) -> Result<()> {
                Ok(())
            }
            async fn core_stop(&self) -> Result<()> {
                Ok(())
            }
            async fn core_is_running(&self) -> Result<bool> {
                Ok(false)
            }
            fn core_controller_url(&self) -> Option<String> {
                None
            }
            async fn credential_get(&self, _s: &str, _k: &str) -> Result<Option<String>> {
                Ok(None)
            }
            async fn credential_set(&self, _s: &str, _k: &str, _v: &str) -> Result<()> {
                Ok(())
            }
            async fn credential_delete(&self, _s: &str, _k: &str) -> Result<()> {
                Ok(())
            }
            fn data_dir(&self) -> Option<PathBuf> {
                None
            }
            fn cache_dir(&self) -> Option<PathBuf> {
                None
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
            async fn tun_set_enabled(&self, _e: bool) -> Result<bool> {
                Ok(false)
            }
            async fn tun_is_enabled(&self) -> Result<bool> {
                Ok(false)
            }
        }
        let api = AndroidApi::new(NoDirBridge);
        assert!(api.data_dir().is_none());
        assert!(api.cache_dir().is_none());
    }
}
