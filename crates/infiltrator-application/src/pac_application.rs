//! PAC generation and local-service use-cases.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::pac::{PacRequest, PacServiceState, PacSnapshot};
use infiltrator_ports::pac::PacServicePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PacApplication {
    gateway: Arc<dyn RuntimeGateway>,
    service: Arc<dyn PacServicePort>,
    next_revision: Arc<AtomicU64>,
    last_snapshot: Arc<Mutex<PacSnapshot>>,
}

impl PacApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>, service: Arc<dyn PacServicePort>) -> Self {
        Self {
            gateway,
            service,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(PacSnapshot::default())),
        }
    }

    pub async fn snapshot(&self) -> PacSnapshot {
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        let previous = self
            .last_snapshot
            .lock()
            .expect("PAC snapshot lock")
            .clone();
        let state = match self.service.status().await {
            Ok(Some(url)) => PacServiceState::Running { url },
            Ok(None) => PacServiceState::Disabled,
            Err(error) => PacServiceState::Unavailable {
                reason: error.to_string(),
            },
        };
        let snapshot = PacSnapshot {
            state,
            script_bytes: previous.script_bytes,
            bypass_domains: previous.bypass_domains,
            revision,
        };
        *self.last_snapshot.lock().expect("PAC snapshot lock") = snapshot.clone();
        snapshot
    }

    pub async fn apply(&self, request: PacRequest) -> Result<PacSnapshot, Failure> {
        let bypass_domains =
            infiltrator_domain::pac_policy::normalize_bypass_domains(&request.bypass_domains)
                .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let rules = self.gateway.get_rules().await.map_err(Failure::from)?;
        let config = self.gateway.get_config().await.map_err(Failure::from)?;
        let port = if config.mixed_port > 0 {
            config.mixed_port
        } else {
            config.port
        };
        if port == 0 {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "PAC service needs a live HTTP proxy endpoint",
                true,
            ));
        }
        let generator =
            infiltrator_domain::pac_generator::PacGenerator::new(format!("127.0.0.1:{port}"))
                .with_bypass_lan(request.bypass_lan)
                .with_bypass_domains(bypass_domains.clone())
                .with_minified(request.minify);
        let script = generator.compile_pac_script(&rules);
        infiltrator_domain::pac_generator::validate_pac_script(&script).map_err(|error| {
            Failure::new(
                ErrorCode::Configuration,
                format!("PAC script validation failed: {error}"),
                false,
            )
        })?;

        let state = if request.enabled {
            let url = self
                .service
                .start(script.clone())
                .await
                .map_err(Failure::from)?;
            let observed = self.service.status().await.map_err(Failure::from)?;
            if observed.as_deref() != Some(url.as_str()) {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    format!("PAC service readback mismatch: started {url}, observed {observed:?}"),
                    true,
                ));
            }
            PacServiceState::Running { url }
        } else {
            self.service.stop().await.map_err(Failure::from)?;
            if self
                .service
                .status()
                .await
                .map_err(Failure::from)?
                .is_some()
            {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "PAC service remained active after stop",
                    true,
                ));
            }
            PacServiceState::Disabled
        };
        let snapshot = PacSnapshot {
            state,
            script_bytes: script.len(),
            bypass_domains,
            revision: self.next_revision.fetch_add(1, Ordering::Relaxed),
        };
        *self.last_snapshot.lock().expect("PAC snapshot lock") = snapshot.clone();
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::pac::PacServiceState;
    use infiltrator_domain::runtime::{
        ConfigSnapshot, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::pac::PacServicePort;
    use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeService {
        url: Mutex<Option<String>>,
    }

    #[async_trait]
    impl PacServicePort for FakeService {
        async fn start(&self, _script: String) -> Result<String, PortError> {
            let url = "http://127.0.0.1:25211/proxy.pac".to_owned();
            *self.url.lock().expect("PAC URL lock") = Some(url.clone());
            Ok(url)
        }

        async fn stop(&self) -> Result<(), PortError> {
            *self.url.lock().expect("PAC URL lock") = None;
            Ok(())
        }

        async fn status(&self) -> Result<Option<String>, PortError> {
            Ok(self.url.lock().expect("PAC URL lock").clone())
        }
    }

    struct FakeGateway;

    #[async_trait]
    impl RuntimeGateway for FakeGateway {
        async fn get_config(&self) -> Result<ConfigSnapshot, PortError> {
            Ok(ConfigSnapshot {
                port: 7890,
                mixed_port: 0,
                ..ConfigSnapshot::default()
            })
        }
        async fn patch_config(&self, _updates: serde_json::Value) -> Result<(), PortError> {
            Ok(())
        }
        async fn set_proxy_mode(
            &self,
            _mode: infiltrator_contract::command::ProxyMode,
        ) -> Result<(), PortError> {
            Ok(())
        }
        async fn get_proxies(
            &self,
        ) -> Result<HashMap<String, infiltrator_domain::proxy::Proxy>, PortError> {
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
        async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
            Ok(ConnectionSnapshot::default())
        }
        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Ok(MemoryData::default())
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
            Ok(Box::pin(futures_util::stream::empty()))
        }
        async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
        async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
            Ok(Box::pin(futures_util::stream::empty()))
        }
    }

    #[tokio::test]
    async fn pac_application_compiles_live_rules_and_reads_back_service() {
        let application =
            PacApplication::new(Arc::new(FakeGateway), Arc::new(FakeService::default()));
        let snapshot = application
            .apply(PacRequest {
                enabled: true,
                bypass_domains: vec!["example.com".to_owned()],
                bypass_lan: true,
                minify: false,
            })
            .await
            .expect("PAC service should start");
        assert!(matches!(snapshot.state, PacServiceState::Running { .. }));
        assert!(snapshot.script_bytes > 0);
        assert_eq!(snapshot.bypass_domains, vec!["example.com"]);
    }
}
