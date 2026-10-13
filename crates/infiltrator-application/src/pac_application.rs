//! PAC generation and local-service use-cases.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::pac::{PacRequest, PacServiceState, PacSnapshot};
use infiltrator_domain::pac_generator::{PacGenerator, validate_pac_script};
use infiltrator_domain::pac_policy::normalize_bypass_domains;
use infiltrator_ports::pac::PacServicePort;
use infiltrator_ports::runtime_configuration::ObservedValue;
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
        let configuration = crate::runtime_configuration_application::
            RuntimeConfigurationApplication::from_current_gateway(self.gateway.as_ref())?;
        self.apply_with_configuration(configuration, request).await
    }

    pub async fn apply_scoped(
        &self,
        scope: infiltrator_contract::core_identity::CoreScope,
        request: PacRequest,
    ) -> Result<PacSnapshot, Failure> {
        let configuration =
            crate::runtime_configuration_application::RuntimeConfigurationApplication::from_gateway(
                scope,
                self.gateway.as_ref(),
            );
        self.apply_with_configuration(configuration, request).await
    }

    async fn apply_with_configuration(
        &self,
        configuration: crate::runtime_configuration_application::RuntimeConfigurationApplication,
        request: PacRequest,
    ) -> Result<PacSnapshot, Failure> {
        let bypass_domains = normalize_bypass_domains(&request.bypass_domains)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        let rules = configuration
            .observe_pac_rules()
            .await
            .map_err(Failure::from)?;
        let ports = configuration
            .observe_proxy_ports()
            .await
            .map_err(Failure::from)?;
        let mixed_port = match ports.value.mixed_port {
            ObservedValue::Reported(port) => port,
            ObservedValue::Unreported => 0,
        };
        let http_port = match ports.value.http_port {
            ObservedValue::Reported(port) => port,
            ObservedValue::Unreported => 0,
        };
        let port = if mixed_port > 0 {
            mixed_port
        } else {
            http_port
        };
        if port == 0 {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "PAC service needs a live HTTP proxy endpoint",
                true,
            ));
        }
        let generator = PacGenerator::new(format!("127.0.0.1:{port}"))
            .with_bypass_lan(request.bypass_lan)
            .with_bypass_domains(bypass_domains.clone())
            .with_minified(request.minify);
        let script = generator.compile_pac_script(&rules.value);
        validate_pac_script(&script).map_err(|error| {
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
    #[cfg(test)]
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    #[cfg(test)]
    use infiltrator_contract::pac::PacServiceState;
    use infiltrator_contract::session::SessionToken;
    #[cfg(test)]
    use infiltrator_domain::rules::RuleEntry;
    use infiltrator_domain::runtime::{MemoryData, ProxyProvider, RuleProvider};
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::pac::PacServicePort;
    use infiltrator_ports::runtime_configuration::{
        ConfigurationObservation, ConfigurationRevision, PacRuleReadPort, ProxyPortsConfiguration,
        ProxyPortsReadPort, RuntimeConfigurationPorts,
    };
    use infiltrator_ports::runtime_gateway::RuntimeGateway;

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

    #[derive(Clone)]
    struct FakePacConfiguration(CoreScope);

    #[async_trait]
    impl ProxyPortsReadPort for FakePacConfiguration {
        async fn observe(
            &self,
            scope: &CoreScope,
        ) -> Result<ConfigurationObservation<ProxyPortsConfiguration>, PortError> {
            Ok(ConfigurationObservation {
                scope: scope.clone(),
                revision: ConfigurationRevision::new(1),
                value: ProxyPortsConfiguration {
                    http_port: ObservedValue::Reported(7890),
                    mixed_port: ObservedValue::Reported(0),
                },
            })
        }
    }

    #[async_trait]
    impl PacRuleReadPort for FakePacConfiguration {
        async fn observe_rules(
            &self,
            scope: &CoreScope,
        ) -> Result<ConfigurationObservation<Vec<RuleEntry>>, PortError> {
            Ok(ConfigurationObservation {
                scope: scope.clone(),
                revision: ConfigurationRevision::new(1),
                value: vec![RuleEntry {
                    rule: "MATCH,DIRECT".to_owned(),
                    enabled: true,
                }],
            })
        }
    }

    struct FakeGateway {
        config: FakePacConfiguration,
    }

    #[async_trait]
    impl RuntimeGateway for FakeGateway {
        fn current_core_scope(&self) -> Result<CoreScope, PortError> {
            Ok(self.config.0.clone())
        }

        fn configuration_ports(&self) -> RuntimeConfigurationPorts {
            let configuration = Arc::new(self.config.clone());
            let mut ports = RuntimeConfigurationPorts::unsupported();
            ports.proxy_ports = configuration.clone();
            ports.pac_rules = configuration;
            ports
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

        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Ok(MemoryData::default())
        }
    }

    #[tokio::test]
    async fn pac_application_compiles_live_rules_and_reads_back_service() {
        let application = PacApplication::new(
            Arc::new(FakeGateway {
                config: FakePacConfiguration(scope()),
            }),
            Arc::new(FakeService::default()),
        );
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

    fn scope() -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine"),
                CoreInstanceId::new("pac-core").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: Some(SessionToken::new(1)),
        }
    }
}
