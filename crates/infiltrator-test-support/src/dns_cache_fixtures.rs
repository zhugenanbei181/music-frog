//! Explicit isolated Fake-IP controller and OS cache targets for behavior captures.
use async_trait::async_trait;
use futures_util::future::pending;
use infiltrator_contract::capability::Capability;
use infiltrator_ports::controller_fake_ip_cache::{
    ControllerFakeIpCacheFlushReceipt, ControllerFakeIpCacheFlushRequest,
    ControllerFakeIpCacheFlushState, ControllerFakeIpCachePort,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_dns_cache::{SystemDnsCacheFlush, SystemDnsCachePort};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePortMode {
    Allowed,
    Denied,
    Unsupported,
    Pending,
}

pub struct IsolatedCaches {
    pub fake_calls: AtomicUsize,
    pub system_calls: AtomicUsize,
    fake_mode: Mutex<CachePortMode>,
    system_mode: Mutex<CachePortMode>,
    fake_entries: Mutex<Vec<String>>,
    system_entries: Mutex<Vec<String>>,
}

impl Default for IsolatedCaches {
    fn default() -> Self {
        Self {
            fake_calls: AtomicUsize::new(0),
            system_calls: AtomicUsize::new(0),
            fake_mode: Mutex::new(CachePortMode::Allowed),
            system_mode: Mutex::new(CachePortMode::Allowed),
            fake_entries: Mutex::new(vec!["cached.example.test".into()]),
            system_entries: Mutex::new(vec!["host.example.test".into()]),
        }
    }
}

impl IsolatedCaches {
    pub fn set_fake_mode(&self, mode: CachePortMode) {
        *self.fake_mode.lock().expect("isolated Fake-IP mode") = mode;
    }

    pub fn set_system_mode(&self, mode: CachePortMode) {
        *self.system_mode.lock().expect("isolated OS mode") = mode;
    }

    pub fn contents(&self) -> (Vec<String>, Vec<String>) {
        (
            self.fake_entries
                .lock()
                .expect("isolated Fake-IP entries")
                .clone(),
            self.system_entries
                .lock()
                .expect("isolated OS entries")
                .clone(),
        )
    }
}

pub fn test_core_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine ID"),
            CoreInstanceId::new("dns-cache-test").expect("valid instance ID"),
            Some(CoreVersion::new("v1.19.32").expect("valid core version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

#[async_trait]
impl ControllerFakeIpCachePort for IsolatedCaches {
    async fn flush(
        &self,
        request: ControllerFakeIpCacheFlushRequest,
    ) -> Result<ControllerFakeIpCacheFlushReceipt, PortError> {
        self.fake_calls.fetch_add(1, Ordering::SeqCst);
        let mode = *self.fake_mode.lock().expect("isolated Fake-IP mode");
        match mode {
            CachePortMode::Allowed => {
                self.fake_entries
                    .lock()
                    .expect("isolated Fake-IP entries")
                    .clear();
                Ok(ControllerFakeIpCacheFlushReceipt {
                    scope: request.scope,
                    operation: request.operation,
                    state: ControllerFakeIpCacheFlushState::Flushed,
                })
            }
            CachePortMode::Denied => Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow Fake-IP cache access, then retry clearing.",
                false,
            ))),
            CachePortMode::Unsupported => Err(PortError::unsupported(
                Capability::Dns,
                "this controller has no Fake-IP flush",
            )),
            CachePortMode::Pending => pending().await,
        }
    }
}

#[async_trait]
impl SystemDnsCachePort for IsolatedCaches {
    async fn flush_system_cache(&self) -> Result<SystemDnsCacheFlush, PortError> {
        self.system_calls.fetch_add(1, Ordering::SeqCst);
        let mode = *self.system_mode.lock().expect("isolated OS mode");
        match mode {
            CachePortMode::Allowed => {
                self.system_entries
                    .lock()
                    .expect("isolated OS entries")
                    .clear();
                Ok(SystemDnsCacheFlush::Flushed)
            }
            CachePortMode::Denied => Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow system resolver cache access, then retry clearing.",
                false,
            ))),
            CachePortMode::Unsupported => Ok(SystemDnsCacheFlush::Unsupported),
            CachePortMode::Pending => pending().await,
        }
    }
}
