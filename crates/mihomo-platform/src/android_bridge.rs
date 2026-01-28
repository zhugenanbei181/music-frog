use async_trait::async_trait;
use mihomo_api::Result;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

#[async_trait]
pub trait AndroidBridge: Send + Sync {
    async fn core_start(&self) -> Result<()>;
    async fn core_stop(&self) -> Result<()>;
    async fn core_is_running(&self) -> Result<bool>;
    fn core_controller_url(&self) -> Option<String>;

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>>;
    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    async fn credential_delete(&self, service: &str, key: &str) -> Result<()>;

    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf>;

    async fn vpn_start(&self) -> Result<bool>;
    async fn vpn_stop(&self) -> Result<bool>;
    async fn vpn_is_running(&self) -> Result<bool>;
    async fn tun_set_enabled(&self, enabled: bool) -> Result<bool>;
    async fn tun_is_enabled(&self) -> Result<bool>;
}

#[async_trait]
impl AndroidBridge for Box<dyn AndroidBridge> {
    async fn core_start(&self) -> Result<()> {
        self.as_ref().core_start().await
    }

    async fn core_stop(&self) -> Result<()> {
        self.as_ref().core_stop().await
    }

    async fn core_is_running(&self) -> Result<bool> {
        self.as_ref().core_is_running().await
    }

    fn core_controller_url(&self) -> Option<String> {
        self.as_ref().core_controller_url()
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.as_ref().credential_get(service, key).await
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.as_ref().credential_set(service, key, value).await
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.as_ref().credential_delete(service, key).await
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.as_ref().data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.as_ref().cache_dir()
    }

    async fn vpn_start(&self) -> Result<bool> {
        self.as_ref().vpn_start().await
    }

    async fn vpn_stop(&self) -> Result<bool> {
        self.as_ref().vpn_stop().await
    }

    async fn vpn_is_running(&self) -> Result<bool> {
        self.as_ref().vpn_is_running().await
    }

    async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        self.as_ref().tun_set_enabled(enabled).await
    }

    async fn tun_is_enabled(&self) -> Result<bool> {
        self.as_ref().tun_is_enabled().await
    }
}

#[async_trait]
impl AndroidBridge for Arc<dyn AndroidBridge> {
    async fn core_start(&self) -> Result<()> {
        self.as_ref().core_start().await
    }

    async fn core_stop(&self) -> Result<()> {
        self.as_ref().core_stop().await
    }

    async fn core_is_running(&self) -> Result<bool> {
        self.as_ref().core_is_running().await
    }

    fn core_controller_url(&self) -> Option<String> {
        self.as_ref().core_controller_url()
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.as_ref().credential_get(service, key).await
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.as_ref().credential_set(service, key, value).await
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.as_ref().credential_delete(service, key).await
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.as_ref().data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.as_ref().cache_dir()
    }

    async fn vpn_start(&self) -> Result<bool> {
        self.as_ref().vpn_start().await
    }

    async fn vpn_stop(&self) -> Result<bool> {
        self.as_ref().vpn_stop().await
    }

    async fn vpn_is_running(&self) -> Result<bool> {
        self.as_ref().vpn_is_running().await
    }

    async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        self.as_ref().tun_set_enabled(enabled).await
    }

    async fn tun_is_enabled(&self) -> Result<bool> {
        self.as_ref().tun_is_enabled().await
    }
}

pub fn set_android_bridge(bridge: Arc<dyn AndroidBridge>) {
    let mut guard = android_bridge_state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(bridge);
}

pub fn clear_android_bridge() {
    let mut guard = android_bridge_state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

pub fn get_android_bridge() -> Option<Arc<dyn AndroidBridge>> {
    android_bridge_state()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn android_bridge_state() -> &'static RwLock<Option<Arc<dyn AndroidBridge>>> {
    static ANDROID_BRIDGE: OnceLock<RwLock<Option<Arc<dyn AndroidBridge>>>> = OnceLock::new();
    ANDROID_BRIDGE.get_or_init(|| RwLock::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBridge;

    #[async_trait]
    impl AndroidBridge for TestBridge {
        async fn core_start(&self) -> Result<()> {
            Ok(())
        }

        async fn core_stop(&self) -> Result<()> {
            Ok(())
        }

        async fn core_is_running(&self) -> Result<bool> {
            Ok(true)
        }

        fn core_controller_url(&self) -> Option<String> {
            Some("http://127.0.0.1:9090".to_string())
        }

        async fn credential_get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
            Ok(Some("secret".to_string()))
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
            Ok(true)
        }

        async fn tun_set_enabled(&self, _enabled: bool) -> Result<bool> {
            Ok(true)
        }

        async fn tun_is_enabled(&self) -> Result<bool> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_android_bridge_registry() {
        clear_android_bridge();
        set_android_bridge(Arc::new(TestBridge));
        let bridge = get_android_bridge().expect("bridge");
        
        // Exhaustive check of proxied methods via Arc
        assert!(bridge.core_start().await.is_ok());
        assert!(bridge.core_stop().await.is_ok());
        assert!(bridge.core_is_running().await.unwrap());
        assert_eq!(bridge.core_controller_url().unwrap(), "http://127.0.0.1:9090");
        
        assert_eq!(bridge.credential_get("s", "k").await.unwrap(), Some("secret".to_string()));
        assert!(bridge.credential_set("s", "k", "v").await.is_ok());
        assert!(bridge.credential_delete("s", "k").await.is_ok());
        
        assert_eq!(bridge.data_dir().unwrap(), PathBuf::from("data"));
        assert_eq!(bridge.cache_dir().unwrap(), PathBuf::from("cache"));
        
        assert!(bridge.vpn_start().await.unwrap());
        assert!(bridge.vpn_stop().await.unwrap());
        assert!(bridge.vpn_is_running().await.unwrap());
        assert!(bridge.tun_set_enabled(true).await.unwrap());
        assert!(bridge.tun_is_enabled().await.unwrap());

        // Test Box proxy
        let boxed: Box<dyn AndroidBridge> = Box::new(TestBridge);
        assert!(boxed.core_start().await.is_ok());
        assert_eq!(boxed.data_dir().unwrap(), PathBuf::from("data"));

        clear_android_bridge();
        assert!(get_android_bridge().is_none());
    }
}
