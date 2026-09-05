//! Runtime observation use-cases over the controller gateway.

use infiltrator_contract::error::Failure;
use infiltrator_contract::command::{CoreLogLevel, ProxyMode};
use infiltrator_contract::tun::TunStack;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::sync::Arc;

#[derive(Clone)]
pub struct RuntimeQueryApplication {
    gateway: Arc<dyn RuntimeGateway>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::command::CoreLogLevel;
    use infiltrator_contract::resources::{CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus};
    use crate::resource_application::ResourceApplication;
    use infiltrator_domain::proxy::Proxy;
    use infiltrator_domain::runtime::{
        ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::runtime_gateway::{RuntimeStreamEvent, RuntimeGateway};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestGateway {
        level: Arc<Mutex<String>>,
        tun_stack: Arc<Mutex<String>>,
        tun_mtu: Arc<Mutex<Option<u32>>>,
        apply_patch: bool,
        memory_bytes: Arc<Mutex<u64>>,
        gc_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl RuntimeGateway for TestGateway {
        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            Ok(ConfigSnapshot {
                mode: "rule".to_owned(),
                log_level: self.level.lock().expect("level lock").clone(),
                tun: Some(infiltrator_domain::runtime::TunSnapshot {
                    enable: true,
                    stack: self.tun_stack.lock().expect("stack lock").clone(),
                    auto_route: true,
                    strict_route: false,
                    mtu: *self.tun_mtu.lock().expect("tun mtu lock"),
                }),
                ..ConfigSnapshot::default()
            })
        }

        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Ok(MemoryData {
                in_use: *self.memory_bytes.lock().expect("memory lock"),
                os_limit: 0,
            })
        }

        async fn patch_config(
            &self,
            updates: serde_json::Value,
        ) -> Result<(), PortError> {
            if self.apply_patch
                && let Some(level) = updates.get("log-level").and_then(|value| value.as_str())
            {
                *self.level.lock().expect("level lock") = level.to_owned();
            }
            if self.apply_patch
                && let Some(stack) = updates
                    .get("tun")
                    .and_then(|value| value.get("stack"))
                    .and_then(|value| value.as_str())
            {
                *self.tun_stack.lock().expect("stack lock") = stack.to_owned();
            }
            if self.apply_patch
                && let Some(mtu) = updates
                    .get("tun")
                    .and_then(|value| value.get("mtu"))
                    .and_then(|value| value.as_u64())
            {
                *self.tun_mtu.lock().expect("tun mtu lock") = Some(mtu as u32);
            }
            Ok(())
        }

        async fn set_proxy_mode(
            &self,
            _mode: infiltrator_contract::command::ProxyMode,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn get_proxies(&self) -> Result<HashMap<String, Proxy>, PortError> {
            Ok(HashMap::new())
        }

        async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn test_delay(
            &self,
            _proxy: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<u32, PortError> {
            Ok(0)
        }

        async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
            Ok(Vec::new())
        }

        async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
            Ok(Vec::new())
        }

        async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn flush_fakeip_cache(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn trigger_gc(&self) -> Result<(), PortError> {
            self.gc_calls.fetch_add(1, Ordering::SeqCst);
            *self.memory_bytes.lock().expect("memory lock") = 400 * 1024 * 1024;
            Ok(())
        }

        async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
            Ok(ConnectionSnapshot::default())
        }

        async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn close_all_connections(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn stream_logs(
            &self,
            _level: Option<String>,
        ) -> Result<RuntimeStream<String>, PortError> {
            Ok(Box::pin(futures_util::stream::empty::<RuntimeStreamEvent<String>>()))
        }

        async fn stream_traffic(
            &self,
        ) -> Result<RuntimeStream<TrafficData>, PortError> {
            Ok(Box::pin(
                futures_util::stream::empty::<RuntimeStreamEvent<TrafficData>>(),
            ))
        }

        async fn stream_connections(
            &self,
        ) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
            Ok(Box::pin(
                futures_util::stream::empty::<RuntimeStreamEvent<ConnectionSnapshot>>(),
            ))
        }
    }

    #[tokio::test]
    async fn core_log_level_patch_is_read_back_before_success() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        RuntimeQueryApplication::new(gateway.clone())
            .set_core_log_level(CoreLogLevel::Debug)
            .await
            .unwrap();
        assert_eq!(gateway.level.lock().unwrap().as_str(), "debug");
    }

    #[tokio::test]
    async fn core_log_level_readback_mismatch_is_not_reported_as_success() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let failure = RuntimeQueryApplication::new(gateway)
            .set_core_log_level(CoreLogLevel::Debug)
            .await
            .unwrap_err();
        assert_eq!(
            failure.code,
            infiltrator_contract::error::ErrorCode::InvalidState
        );
    }

    #[tokio::test]
    async fn resource_poll_triggers_gc_once_when_memory_exceeds_soft_limit() {
        let memory = Arc::new(Mutex::new(CORE_MEMORY_SOFT_LIMIT_BYTES + 1));
        let gc_calls = Arc::new(AtomicUsize::new(0));
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: true,
            memory_bytes: memory,
            gc_calls: gc_calls.clone(),
        });
        let application = ResourceApplication::new(gateway);
        let first = application.poll().await.unwrap();
        assert!(matches!(first.gc, CoreGcStatus::Triggered { .. }));
        assert_eq!(gc_calls.load(Ordering::SeqCst), 1);

        let second = application.poll().await.unwrap();
        assert!(matches!(second.gc, CoreGcStatus::NotNeeded));
        assert_eq!(gc_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tun_stack_patch_is_read_back_and_reference_only_lwip_is_rejected() {
        let stack = Arc::new(Mutex::new("gvisor".to_owned()));
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: stack.clone(),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        RuntimeQueryApplication::new(gateway)
            .set_tun_stack(TunStack::Mixed)
            .await
            .unwrap();
        assert_eq!(stack.lock().unwrap().as_str(), "mixed");

        let failure = RuntimeQueryApplication::new(Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }))
        .set_tun_stack(TunStack::Lwip)
        .await
        .unwrap_err();
        assert_eq!(
            failure.code,
            infiltrator_contract::error::ErrorCode::Unsupported
        );
    }

    #[tokio::test]
    async fn tun_mtu_patch_is_read_back_before_success() {
        let tun_mtu = Arc::new(Mutex::new(None));
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: tun_mtu.clone(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        RuntimeQueryApplication::new(gateway)
            .set_tun_mtu(1420)
            .await
            .expect("MTU readback should match");
        assert_eq!(*tun_mtu.lock().expect("tun mtu lock"), Some(1420));
    }

    #[tokio::test]
    async fn tun_mtu_rejects_out_of_range_and_readback_mismatch() {
        let failure = RuntimeQueryApplication::new(Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }))
        .set_tun_mtu(1279)
        .await
        .expect_err("out-of-range MTU must fail before I/O");
        assert_eq!(failure.code, infiltrator_contract::error::ErrorCode::InvalidInput);

        let failure = RuntimeQueryApplication::new(Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }))
        .set_tun_mtu(1420)
        .await
        .expect_err("ignored patch must fail readback");
        assert_eq!(failure.code, infiltrator_contract::error::ErrorCode::InvalidState);
    }
}

impl RuntimeQueryApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self { gateway }
    }

    pub async fn memory(&self) -> Result<MemoryData, Failure> {
        self.gateway.get_memory().await.map_err(Failure::from)
    }

    pub async fn set_proxy_mode(&self, mode: ProxyMode) -> Result<(), Failure> {
        self.gateway
            .set_proxy_mode(mode)
            .await
            .map_err(Failure::from)
    }

    /// Apply a core log-level change live and read it back before reporting
    /// success. The check prevents a controller that accepted but ignored a
    /// malformed/unsupported value from producing a false UI success.
    pub async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<(), Failure> {
        self.gateway
            .patch_config(serde_json::json!({ "log-level": level.as_str() }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        if CoreLogLevel::parse(&observed.log_level) != Some(level) {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "core log level readback mismatch: requested {}, observed {}",
                    level.as_str(),
                    observed.log_level
                ),
                true,
            ));
        }
        Ok(())
    }

    /// Apply one of Mihomo's live TUN stack values and verify the controller
    /// readback. Reference-only catalog entries are rejected before I/O.
    pub async fn set_tun_stack(&self, stack: TunStack) -> Result<(), Failure> {
        if !stack.is_live_supported() {
            return Err(Failure::unsupported(format!(
                "TUN stack {} is reference-only and not accepted by Mihomo",
                stack.as_str()
            )));
        }
        self.gateway
            .patch_config(serde_json::json!({ "tun": { "stack": stack.as_str() } }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        let observed_stack = observed
            .tun
            .as_ref()
            .and_then(|tun| TunStack::parse(&tun.stack));
        if observed_stack != Some(stack) {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "TUN stack readback mismatch: requested {}, observed {}",
                    stack.as_str(),
                    observed
                        .tun
                        .map(|tun| tun.stack)
                        .unwrap_or_else(|| "missing".to_owned())
                ),
                true,
            ));
        }
        Ok(())
    }

    /// Apply the calculated TUN MTU and verify that the running controller
    /// reports the same value. This keeps adaptive negotiation from becoming
    /// a UI-only calculation when the host has a live gateway.
    pub async fn set_tun_mtu(&self, mtu: u32) -> Result<(), Failure> {
        if !(infiltrator_contract::mtu::MIN_TUN_MTU_BYTES
            ..=infiltrator_contract::mtu::MAX_TUN_MTU_BYTES)
            .contains(&mtu)
        {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidInput,
                format!(
                    "TUN MTU {mtu} is outside the supported range {}..={}",
                    infiltrator_contract::mtu::MIN_TUN_MTU_BYTES,
                    infiltrator_contract::mtu::MAX_TUN_MTU_BYTES
                ),
                false,
            ));
        }
        self.gateway
            .patch_config(serde_json::json!({ "tun": { "mtu": mtu } }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        let observed_mtu = observed.tun.as_ref().and_then(|tun| tun.mtu);
        if observed_mtu != Some(mtu) {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "TUN MTU readback mismatch: requested {mtu}, observed {}",
                    observed_mtu
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "missing".to_owned())
                ),
                true,
            ));
        }
        Ok(())
    }

    pub async fn logs(
        &self,
        level: Option<String>,
    ) -> Result<RuntimeStream<String>, Failure> {
        self.gateway
            .stream_logs(level)
            .await
            .map_err(Failure::from)
    }

    pub async fn traffic(&self) -> Result<RuntimeStream<TrafficData>, Failure> {
        self.gateway
            .stream_traffic()
            .await
            .map_err(Failure::from)
    }
}
