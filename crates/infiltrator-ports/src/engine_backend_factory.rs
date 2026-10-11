//! Engine-scoped process, configuration, and runtime adapter composition.

use crate::core_process::CoreProcess;
use crate::core_process::CoreReadiness;
use crate::error::PortError;
#[cfg(test)]
use crate::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use crate::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionPort,
};
use crate::profile_document_codec::ProfileDocumentCodecPort;
use crate::runtime_gateway::RuntimeGateway;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use std::sync::Arc;

/// The engine-owned small ports used to compose Core configuration behavior.
///
/// `configuration` is the pure engine document codec. `configuration_transaction`
/// owns the separate native validate/apply/readback/rollback workflow and is
/// scoped to the exact same engine/format/schema identity.
/// `CoreReadiness` and `RuntimeGateway` also lack engine-ID accessors; their
/// binding to this bundle's engine is therefore guaranteed by the selected
/// engine-specific provider, not independently inspected here.
#[derive(Clone)]
pub struct EngineBackendPorts {
    engine_id: EngineId,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    configuration: Arc<dyn ProfileDocumentCodecPort>,
    configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
    runtime: Arc<dyn RuntimeGateway>,
}

impl EngineBackendPorts {
    pub fn new(
        engine_id: EngineId,
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        configuration: Arc<dyn ProfileDocumentCodecPort>,
        configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
        runtime: Arc<dyn RuntimeGateway>,
    ) -> Result<Self, PortError> {
        let process_identity = process.core_identity().ok_or_else(|| {
            PortError::Failed(format!(
                "{} backend process adapter has no engine identity",
                engine_id.as_str()
            ))
        })?;
        if process_identity.engine != engine_id {
            return Err(PortError::Failed(format!(
                "{} backend received process adapter for {}",
                engine_id.as_str(),
                process_identity.engine.as_str()
            )));
        }

        let codec_scope = configuration.scope();
        if codec_scope.engine_id != engine_id {
            return Err(PortError::Failed(format!(
                "{} backend received configuration adapter for {}",
                engine_id.as_str(),
                codec_scope.engine_id.as_str()
            )));
        }
        let transaction_scope = configuration_transaction.scope();
        if transaction_scope.document != codec_scope {
            return Err(PortError::Failed(format!(
                "{} backend configuration transaction scope differs from its codec scope",
                engine_id.as_str()
            )));
        }

        Ok(Self {
            engine_id,
            process,
            readiness,
            configuration,
            configuration_transaction,
            runtime,
        })
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    pub fn process(&self) -> Arc<dyn CoreProcess> {
        Arc::clone(&self.process)
    }

    pub fn readiness(&self) -> Arc<dyn CoreReadiness> {
        Arc::clone(&self.readiness)
    }

    pub fn configuration(&self) -> Arc<dyn ProfileDocumentCodecPort> {
        Arc::clone(&self.configuration)
    }

    pub fn configuration_transaction(&self) -> Arc<dyn ProfileConfigurationTransactionPort> {
        self.configuration_transaction.clone()
    }

    pub fn configuration_transaction_binding(&self) -> Arc<ProfileConfigurationTransactionBinding> {
        Arc::clone(&self.configuration_transaction)
    }

    pub fn runtime(&self) -> Arc<dyn RuntimeGateway> {
        Arc::clone(&self.runtime)
    }
}

/// Creates the small-port set for one requested engine.
///
/// Implementations must return `Unsupported` before adapter side effects when
/// the requested engine is not implemented by that product composition.
pub trait EngineBackendFactory: Send + Sync {
    fn backend_ports(&self, engine_id: &EngineId) -> Result<EngineBackendPorts, PortError>;
}

/// An engine-specific factory registered in a product composition.
/// Implementations assemble adapters only; starting a process remains an
/// explicit application operation after the returned bundle is accepted.
pub trait EngineBackendProvider: Send + Sync {
    fn engine_id(&self) -> EngineId;
    fn create_backend(&self) -> Result<EngineBackendPorts, PortError>;
}

/// Capability used when a composition has no adapter for the requested engine.
pub const ENGINE_BACKEND_CAPABILITY: Capability = Capability::CoreLifecycle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_process::CoreReadiness;
    use crate::profile_document_codec::ProfileDocumentCodecScope;
    use crate::runtime_gateway::{RuntimeStream, ScopedRuntimeStream};
    use async_trait::async_trait;
    use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId};
    use infiltrator_contract::profile_document::DocumentValidation;
    use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_domain::runtime::{
        ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
    };

    struct FakeProcess {
        identity: CoreIdentity,
    }

    #[async_trait]
    impl CoreProcess for FakeProcess {
        fn core_identity(&self) -> Option<CoreIdentity> {
            Some(self.identity.clone())
        }

        async fn start(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn stop(&self) -> Result<(), PortError> {
            Ok(())
        }

        async fn status(&self) -> Result<CoreLifecycle, PortError> {
            Ok(CoreLifecycle::Stopped)
        }

        fn controller_endpoint(&self) -> Option<String> {
            None
        }
    }

    struct FakeReadiness;

    #[async_trait]
    impl CoreReadiness for FakeReadiness {
        async fn probe(&self) -> Result<String, PortError> {
            Ok("fake-ready".to_owned())
        }
    }

    struct FakeConfiguration {
        scope: ProfileDocumentCodecScope,
    }

    impl ProfileDocumentCodecPort for FakeConfiguration {
        fn scope(&self) -> ProfileDocumentCodecScope {
            self.scope.clone()
        }

        fn inspect(&self, _raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
            Ok(DocumentValidation::Valid)
        }
    }

    struct FakeRuntime;

    #[async_trait]
    impl RuntimeGateway for FakeRuntime {
        async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
            Err(unsupported_runtime())
        }

        async fn test_delay(
            &self,
            _proxy: &str,
            _url: &str,
            _timeout_ms: u32,
        ) -> Result<u32, PortError> {
            Err(unsupported_runtime())
        }

        async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
            Err(unsupported_runtime())
        }

        async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
            Err(unsupported_runtime())
        }

        async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
            Err(unsupported_runtime())
        }

        async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
            Err(unsupported_runtime())
        }

        async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
            Err(unsupported_runtime())
        }

        async fn get_memory(&self) -> Result<MemoryData, PortError> {
            Err(unsupported_runtime())
        }

        async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
            Err(unsupported_runtime())
        }

        async fn close_all_connections(&self) -> Result<(), PortError> {
            Err(unsupported_runtime())
        }

        async fn stream_logs(
            &self,
            _level: Option<String>,
        ) -> Result<RuntimeStream<String>, PortError> {
            Err(unsupported_runtime())
        }

        async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
            Err(unsupported_runtime())
        }

        async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
            Err(unsupported_runtime())
        }

        async fn stream_logs_scoped(
            &self,
            _scope: infiltrator_contract::core_identity::CoreScope,
            _level: Option<String>,
        ) -> Result<ScopedRuntimeStream<String>, PortError> {
            Err(unsupported_runtime())
        }
    }

    fn unsupported_runtime() -> PortError {
        PortError::unsupported(Capability::CoreLifecycle, "fake runtime")
    }

    fn engine_id(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine ID is valid")
    }

    fn fake_ports(
        selected_engine: EngineId,
        process_engine: EngineId,
        codec_engine: EngineId,
    ) -> Result<EngineBackendPorts, PortError> {
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: codec_engine,
            format_id: ProfileFormatId::new("fake-yaml").expect("test format is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("test schema version is valid"),
        };
        EngineBackendPorts::new(
            selected_engine,
            Arc::new(FakeProcess {
                identity: CoreIdentity::new(
                    process_engine,
                    CoreInstanceId::new("test-instance").expect("test instance is valid"),
                    None,
                ),
            }),
            Arc::new(FakeReadiness),
            Arc::new(FakeConfiguration {
                scope: codec_scope.clone(),
            }),
            Arc::new(ProfileConfigurationTransactionBinding::new(
                ProfileConfigurationTransactionScope::new(codec_scope),
            )),
            Arc::new(FakeRuntime),
        )
    }

    #[test]
    fn factory_bundle_accepts_matching_process_and_configuration_adapters() {
        let engine = engine_id("mihomo");
        let ports = fake_ports(engine.clone(), engine.clone(), engine.clone()).unwrap();

        assert_eq!(ports.engine_id(), &engine);
        assert_eq!(ports.process().core_identity().unwrap().engine, engine);
        assert_eq!(ports.configuration().scope().engine_id, engine);
        assert_eq!(
            ports.configuration_transaction().scope().document,
            ports.configuration().scope()
        );
    }

    #[test]
    fn factory_bundle_rejects_process_from_another_engine() {
        let error = match fake_ports(
            engine_id("mihomo"),
            engine_id("sing-box"),
            engine_id("mihomo"),
        ) {
            Ok(_) => panic!("process identity must match the requested engine"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }

    #[test]
    fn factory_bundle_rejects_configuration_codec_from_another_engine() {
        let error = match fake_ports(
            engine_id("mihomo"),
            engine_id("mihomo"),
            engine_id("sing-box"),
        ) {
            Ok(_) => panic!("configuration identity must match the requested engine"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }

    #[test]
    fn factory_bundle_rejects_transaction_with_a_different_format_or_schema() {
        let engine = engine_id("mihomo");
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: engine.clone(),
            format_id: ProfileFormatId::new("fake-yaml").expect("test format is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("test schema version is valid"),
        };
        let transaction_scope = ProfileDocumentCodecScope {
            schema_version: ProfileSchemaVersion::new(2).expect("test schema version is valid"),
            ..codec_scope.clone()
        };
        let error = match EngineBackendPorts::new(
            engine.clone(),
            Arc::new(FakeProcess {
                identity: CoreIdentity::new(
                    engine.clone(),
                    CoreInstanceId::new("test-instance").expect("test instance is valid"),
                    None,
                ),
            }),
            Arc::new(FakeReadiness),
            Arc::new(FakeConfiguration { scope: codec_scope }),
            Arc::new(ProfileConfigurationTransactionBinding::new(
                ProfileConfigurationTransactionScope::new(transaction_scope),
            )),
            Arc::new(FakeRuntime),
        ) {
            Ok(_) => panic!("transaction and codec scopes must be identical"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }
}
