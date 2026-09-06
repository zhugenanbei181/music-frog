//! Runtime observation use-cases over the controller gateway.

use infiltrator_contract::error::Failure;
use infiltrator_contract::command::{CoreLogLevel, ProxyMode};
use infiltrator_contract::tun::TunStack;
use infiltrator_domain::runtime::{MemoryData, TrafficData};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

#[derive(Clone)]
pub struct RuntimeQueryApplication {
    gateway: Arc<dyn RuntimeGateway>,
    next_revision: Arc<AtomicU64>,
}

#[path = "runtime_query_lan.rs"]
mod runtime_query_lan;
#[path = "runtime_query_ipv6.rs"] mod runtime_query_ipv6;

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
pub(crate) mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::command::CoreLogLevel;
    use infiltrator_contract::lan::LanCredentials;
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

    #[derive(Clone, Debug)]
    pub(crate) struct TestLanState {
        allow_lan: bool,
        mixed_port: u16,
        bind_address: String, ipv6_enabled: bool,
        allowed_ips: Vec<String>,
        disallowed_ips: Vec<String>,
        skip_auth_prefixes: Vec<String>,
        authentication_enabled: bool,
        authentication_user_count: usize,
        authentication_username: Option<String>,
    }

    pub(crate) struct TestGateway {
        pub(crate) level: Arc<Mutex<String>>,
        pub(crate) tun_stack: Arc<Mutex<String>>,
        pub(crate) tun_mtu: Arc<Mutex<Option<u32>>>,
        pub(crate) tun_routing: Arc<Mutex<(bool, bool)>>,
        pub(crate) tun_enabled: Arc<Mutex<bool>>,
        pub(crate) lan: Arc<Mutex<TestLanState>>,
        pub(crate) apply_patch: bool,
        pub(crate) memory_bytes: Arc<Mutex<u64>>,
        pub(crate) gc_calls: Arc<AtomicUsize>,
    }

    pub(crate) fn lan_state() -> Arc<Mutex<TestLanState>> {
        Arc::new(Mutex::new(TestLanState {
            allow_lan: false,
            mixed_port: 7890,
            bind_address: "*".to_owned(), ipv6_enabled: true,
            allowed_ips: vec!["192.168.0.0/16".to_owned()],
            disallowed_ips: Vec::new(),
            skip_auth_prefixes: vec!["127.0.0.0/8".to_owned()],
            authentication_enabled: false,
            authentication_user_count: 0,
            authentication_username: None,
        }))
    }

    #[async_trait]
    impl RuntimeGateway for TestGateway {
        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            let lan = self.lan.lock().expect("LAN state lock").clone();
            Ok(ConfigSnapshot { mode: "rule".to_owned(),
                log_level: self.level.lock().expect("level lock").clone(),
                tun: {
                    let (auto_route, strict_route) =
                        *self.tun_routing.lock().expect("tun routing lock");
                    Some(infiltrator_domain::runtime::TunSnapshot {
                        enable: *self.tun_enabled.lock().expect("tun enabled lock"),
                        stack: self.tun_stack.lock().expect("stack lock").clone(),
                        auto_route,
                        strict_route,
                        mtu: *self.tun_mtu.lock().expect("tun mtu lock"),
                    })
                },
                allow_lan: lan.allow_lan, ipv6: lan.ipv6_enabled,
                mixed_port: lan.mixed_port,
                bind_address: lan.bind_address,
                lan_allowed_ips: lan.allowed_ips,
                lan_disallowed_ips: lan.disallowed_ips,
                skip_auth_prefixes: lan.skip_auth_prefixes,
                authentication_enabled: lan.authentication_enabled,
                authentication_user_count: lan.authentication_user_count,
                authentication_username: lan.authentication_username,
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
            if self.apply_patch {
                let mut lan = self.lan.lock().expect("LAN state lock"); if let Some(enabled) = updates.get("ipv6").and_then(|value| value.as_bool()) { lan.ipv6_enabled = enabled; }
                if let Some(enabled) = updates.get("allow-lan").and_then(|value| value.as_bool())
                {
                    lan.allow_lan = enabled;
                }
                if let Some(port) = updates.get("mixed-port").and_then(|value| value.as_u64()) {
                    lan.mixed_port = port as u16;
                }
                if let Some(bind_address) = updates
                    .get("bind-address")
                    .and_then(|value| value.as_str())
                {
                    lan.bind_address = bind_address.to_owned();
                }
                if let Some(values) = updates
                    .get("lan-allowed-ips")
                    .and_then(|value| value.as_array())
                {
                    lan.allowed_ips = values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect();
                }
                if let Some(values) = updates
                    .get("lan-disallowed-ips")
                    .and_then(|value| value.as_array())
                {
                    lan.disallowed_ips = values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect();
                }
                if let Some(values) = updates
                    .get("skip-auth-prefixes")
                    .and_then(|value| value.as_array())
                {
                    lan.skip_auth_prefixes = values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect();
                }
                if let Some(values) = updates
                    .get("authentication")
                    .and_then(|value| value.as_array())
                {
                    lan.authentication_enabled = !values.is_empty();
                    lan.authentication_user_count = values.len();
                    lan.authentication_username = values
                        .first()
                        .and_then(|value| value.as_str())
                        .and_then(|value| value.split_once(':'))
                        .map(|(username, _)| username.to_owned());
                }
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
            if self.apply_patch
                && let Some(tun) = updates.get("tun")
            {
                if let Some(enabled) = tun.get("enable").and_then(|value| value.as_bool()) {
                    *self.tun_enabled.lock().expect("tun enabled lock") = enabled;
                }
                let mut routing = self.tun_routing.lock().expect("tun routing lock");
                if let Some(auto_route) = tun.get("auto-route").and_then(|value| value.as_bool())
                {
                    routing.0 = auto_route;
                }
                if let Some(strict_route) = tun
                    .get("strict-route")
                    .and_then(|value| value.as_bool())
                {
                    routing.1 = strict_route;
                }
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
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
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }))
        .set_tun_mtu(1420)
        .await
        .expect_err("ignored patch must fail readback");
        assert_eq!(failure.code, infiltrator_contract::error::ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn strict_route_enables_auto_route_and_auto_route_off_clears_strict_route() {
        let routing = Arc::new(Mutex::new((false, false)));
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: routing.clone(),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let application = RuntimeQueryApplication::new(gateway);

        application
            .set_tun_strict_route(true)
            .await
            .expect("strict route should enable auto route");
        assert_eq!(*routing.lock().expect("routing lock"), (true, true));

        application
            .set_tun_auto_route(false)
            .await
            .expect("auto route disable should clear strict route");
        assert_eq!(*routing.lock().expect("routing lock"), (false, false));
    }

    #[tokio::test]
    async fn tun_routing_readback_mismatch_is_not_reported_as_success() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((false, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let failure = RuntimeQueryApplication::new(gateway)
            .set_tun_strict_route(true)
            .await
            .expect_err("ignored strict-route patch must fail readback");
        assert_eq!(failure.code, infiltrator_contract::error::ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn tun_enable_patch_is_read_back_before_success() {
        let enabled = Arc::new(Mutex::new(true));
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: enabled.clone(),
            lan: lan_state(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        RuntimeQueryApplication::new(gateway)
            .set_tun_enabled(false)
            .await
            .expect("TUN enable readback should match");
        assert!(!*enabled.lock().expect("tun enabled lock"));
    }

    #[tokio::test]
    async fn lan_sharing_patch_is_atomic_and_reads_back_bind_address() {
        let lan = lan_state();
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan.clone(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let snapshot = RuntimeQueryApplication::new(gateway)
            .set_lan_sharing(true, 8080, "192.168.1.10")
            .await
            .expect("Allow-LAN readback should match");

        assert!(snapshot.enabled);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.mixed_port, 8080);
        assert_eq!(snapshot.bind_address, "192.168.1.10");
        let lan = lan.lock().expect("LAN state lock");
        assert!(lan.allow_lan);
        assert_eq!(lan.mixed_port, 8080);
        assert_eq!(lan.bind_address, "192.168.1.10");
    }

    #[tokio::test]
    async fn lan_sharing_rejects_zero_port_and_invalid_bind_address_before_io() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let application = RuntimeQueryApplication::new(gateway);
        let zero_port = application
            .set_lan_sharing(true, 0, "*")
            .await
            .expect_err("enabled LAN sharing needs a port");
        assert_eq!(
            zero_port.code,
            infiltrator_contract::error::ErrorCode::InvalidInput
        );

        let invalid_address = application
            .set_lan_sharing(false, 7890, "192.168.1.0/24")
            .await
            .expect_err("bind-address cannot be a CIDR");
        assert_eq!(
            invalid_address.code,
            infiltrator_contract::error::ErrorCode::InvalidInput
        );
    }

    #[tokio::test]
    async fn lan_security_patch_reads_back_acl_and_only_exposes_auth_summary() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let credentials = LanCredentials {
            username: "lan-user".to_owned(),
            password: "not-in-snapshot".to_owned(),
        };
        let snapshot = RuntimeQueryApplication::new(gateway)
            .set_lan_security(
                &["192.168.1.0/24".to_owned()],
                &["192.168.1.10/32".to_owned()],
                &["127.0.0.0/8".to_owned()],
                true,
                Some(&credentials),
            )
            .await
            .expect("LAN security readback should match");

        assert_eq!(snapshot.allowed_ips, vec!["192.168.1.0/24"]);
        assert_eq!(snapshot.disallowed_ips, vec!["192.168.1.10/32"]);
        assert_eq!(snapshot.skip_auth_prefixes, vec!["127.0.0.0/8"]);
        assert!(snapshot.authentication_enabled);
        assert_eq!(snapshot.authentication_user_count, 1);
        assert!(!format!("{credentials:?}").contains("not-in-snapshot"));
    }

    #[tokio::test]
    async fn lan_security_requires_credentials_when_authentication_is_enabled() {
        let application = RuntimeQueryApplication::new(Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }));
        let failure = application
            .set_lan_security(
                &["192.168.0.0/16".to_owned()],
                &[],
                &[],
                true,
                None,
            )
            .await
            .expect_err("auth cannot be enabled without credentials");
        assert_eq!(
            failure.code,
            infiltrator_contract::error::ErrorCode::InvalidInput
        );
    }

}

impl RuntimeQueryApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            next_revision: Arc::new(AtomicU64::new(1)),
        }
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

    /// Toggle the controller-owned TUN ingress and verify the live value.
    pub async fn set_tun_enabled(&self, enabled: bool) -> Result<(), Failure> {
        self.gateway
            .patch_config(serde_json::json!({ "tun": { "enable": enabled } }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        let observed_enabled = observed.tun.as_ref().map(|tun| tun.enable);
        if observed_enabled != Some(enabled) {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "TUN enable readback mismatch: requested {enabled}, observed {}",
                    observed_enabled
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "missing".to_owned())
                ),
                true,
            ));
        }
        Ok(())
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

    /// Enable/disable automatic route installation while preserving the
    /// strict-route invariant. Turning auto-route off also clears strict
    /// route because Mihomo only applies strict routing with auto-route.
    pub async fn set_tun_auto_route(&self, enabled: bool) -> Result<(), Failure> {
        let (_, strict_route) = self.current_tun_routing().await?;
        self.set_tun_routing(enabled, enabled && strict_route).await
    }

    /// Enable/disable strict route enforcement. Enabling strict route also
    /// enables auto-route in the same PATCH so traffic cannot pass through an
    /// invalid intermediate configuration.
    pub async fn set_tun_strict_route(&self, enabled: bool) -> Result<(), Failure> {
        let (auto_route, _) = self.current_tun_routing().await?;
        self.set_tun_routing(auto_route || enabled, enabled).await
    }

    async fn current_tun_routing(&self) -> Result<(bool, bool), Failure> {
        let config = self.gateway.get_config().await.map_err(Failure::from)?;
        config
            .tun
            .map(|tun| (tun.auto_route, tun.strict_route))
            .ok_or_else(|| {
                Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "TUN configuration is unavailable",
                    true,
                )
            })
    }

    async fn set_tun_routing(&self, auto_route: bool, strict_route: bool) -> Result<(), Failure> {
        self.gateway
            .patch_config(serde_json::json!({
                "tun": {
                    "auto-route": auto_route,
                    "strict-route": strict_route,
                }
            }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        let observed = observed
            .tun
            .map(|tun| (tun.auto_route, tun.strict_route));
        if observed != Some((auto_route, strict_route)) {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "TUN routing readback mismatch: requested auto-route={auto_route}, strict-route={strict_route}; observed {observed:?}"
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
