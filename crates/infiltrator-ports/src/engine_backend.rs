//! Product extension of the neutral engine backend bundle.
//!
//! The neutral crate owns the engine identity, process/readiness, document
//! inspection and outbound/routing operation vocabulary. This module adds the
//! product-owned native configuration transaction and the rule-provider and
//! Geo database operation owners for one engine.

use crate::geo_database::{GeoDatabasePort, UnsupportedGeoDatabasePort};
#[cfg(test)]
use crate::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use crate::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionPort,
};
use crate::profile_document_codec::ProfileConfigurationCodecPort;
use crate::rule_provider_runtime::{RuleProviderPort, UnsupportedRuleProviderPort};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::core_scope::CoreScopeSource;
use lurker_core_ports::engine_backend::{
    EngineBackendBundle, EngineBackendOwnerScopeBindingResult, EngineBackendPorts,
    EngineOperationPort, EngineOperationPorts,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;

/// Product-typed rule-provider runtime operation.
pub type RuleProviderOperationPort = EngineOperationPort<Arc<dyn RuleProviderPort>>;
/// Product-typed Geo database runtime operation.
pub type GeoDatabaseOperationPort = EngineOperationPort<Arc<dyn GeoDatabasePort>>;

/// Runtime operations that exist only in the product composition. The neutral
/// outbound/routing operations stay in the core bundle so an engine without a
/// product controller adapter can still expose Core lifecycle.
#[derive(Clone)]
pub enum ProductEngineOperationPorts {
    Unsupported {
        reason: String,
    },
    Available {
        rule_provider: RuleProviderOperationPort,
        geo_database: GeoDatabaseOperationPort,
    },
}

impl ProductEngineOperationPorts {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub fn available(
        rule_provider: RuleProviderOperationPort,
        geo_database: GeoDatabaseOperationPort,
    ) -> Self {
        Self::Available {
            rule_provider,
            geo_database,
        }
    }

    pub fn rule_provider(&self) -> Arc<dyn RuleProviderPort> {
        match self {
            Self::Unsupported { reason } => {
                Arc::new(UnsupportedRuleProviderPort::new(reason.clone()))
            }
            Self::Available {
                rule_provider: EngineOperationPort::Unsupported { reason },
                ..
            } => Arc::new(UnsupportedRuleProviderPort::new(reason.clone())),
            Self::Available {
                rule_provider: EngineOperationPort::Available(port),
                ..
            } => Arc::clone(port),
        }
    }

    pub fn geo_database(&self) -> Arc<dyn GeoDatabasePort> {
        match self {
            Self::Unsupported { reason } => {
                Arc::new(UnsupportedGeoDatabasePort::new(reason.clone()))
            }
            Self::Available {
                geo_database: EngineOperationPort::Unsupported { reason },
                ..
            } => Arc::new(UnsupportedGeoDatabasePort::new(reason.clone())),
            Self::Available {
                geo_database: EngineOperationPort::Available(port),
                ..
            } => Arc::clone(port),
        }
    }
}

impl Debug for ProductEngineOperationPorts {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { reason } => formatter
                .debug_struct("Unsupported")
                .field("reason", reason)
                .finish(),
            Self::Available { .. } => formatter.debug_struct("Available").finish_non_exhaustive(),
        }
    }
}

/// Product backend bundle: the neutral core bundle plus the native
/// configuration transaction and product runtime operations.
///
/// `configuration` stays available as the product codec for typed profile
/// editing while the core bundle exposes the same adapter through the neutral
/// inspection trait. The transaction must describe the exact codec
/// engine/format/schema identity.
#[derive(Clone)]
pub struct ProductEngineBackendPorts {
    core: EngineBackendPorts,
    configuration: Arc<dyn ProfileConfigurationCodecPort>,
    configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
    operations: ProductEngineOperationPorts,
}

impl EngineBackendBundle for ProductEngineBackendPorts {
    fn engine_id(&self) -> &EngineId {
        self.core.engine_id()
    }
}

impl ProductEngineBackendPorts {
    pub fn new(
        engine_id: EngineId,
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        configuration: Arc<dyn ProfileConfigurationCodecPort>,
        configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
        operations: EngineOperationPorts,
        product_operations: ProductEngineOperationPorts,
    ) -> Result<Self, PortError> {
        let codec_scope = configuration.scope();
        let document_codec: Arc<dyn ProfileDocumentCodecPort> = configuration.clone();
        let core = EngineBackendPorts::new(
            engine_id.clone(),
            process,
            readiness,
            document_codec,
            operations,
        )?;
        let transaction_scope = configuration_transaction.scope();
        if transaction_scope.document != codec_scope {
            return Err(PortError::Failed(format!(
                "{} backend configuration transaction scope differs from its codec scope",
                engine_id.as_str()
            )));
        }
        Ok(Self {
            core,
            configuration,
            configuration_transaction,
            operations: product_operations,
        })
    }

    pub fn engine_id(&self) -> &EngineId {
        self.core.engine_id()
    }

    pub fn core(&self) -> &EngineBackendPorts {
        &self.core
    }

    pub fn process(&self) -> Arc<dyn CoreProcess> {
        self.core.process()
    }

    pub fn readiness(&self) -> Arc<dyn CoreReadiness> {
        self.core.readiness()
    }

    pub fn configuration(&self) -> Arc<dyn ProfileConfigurationCodecPort> {
        Arc::clone(&self.configuration)
    }

    pub fn configuration_transaction(&self) -> Arc<dyn ProfileConfigurationTransactionPort> {
        self.configuration_transaction.clone()
    }

    pub fn configuration_transaction_binding(&self) -> Arc<ProfileConfigurationTransactionBinding> {
        Arc::clone(&self.configuration_transaction)
    }

    pub fn operations(&self) -> ProductEngineOperationPorts {
        self.operations.clone()
    }

    pub fn core_operations(&self) -> EngineOperationPorts {
        self.core.operations()
    }

    pub fn bind_owner_core_scope(
        &self,
        source: Arc<dyn CoreScopeSource>,
    ) -> Result<EngineBackendOwnerScopeBindingResult, PortError> {
        self.core.bind_owner_core_scope(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lurker_core_contract::capability::Capability;
    use lurker_core_contract::core_process::CoreProcessStartContext;
    use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope};
    use lurker_core_contract::lifecycle::CoreLifecycle;
    use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
    use lurker_core_contract::outbound::inventory::{
        OutboundInventorySnapshot, OutboundProbeSnapshot,
    };
    use lurker_core_contract::profile_document::DocumentValidation;
    use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
    use lurker_core_contract::routing_intent::RoutingIntent;
    use lurker_core_ports::core_scope::CoreScopeObservation;
    use lurker_core_ports::engine_backend::EngineBackendOwnerScopeBinding;
    use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
    use lurker_core_ports::profile_document_codec::ProfileDocumentCodecScope;
    use lurker_core_ports::routing_intent::RoutingIntentPort;

    struct FakeProcess {
        identity: CoreIdentity,
    }

    #[async_trait]
    impl CoreProcess for FakeProcess {
        fn core_identity(&self) -> Option<CoreIdentity> {
            Some(self.identity.clone())
        }

        async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
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

    impl ProfileConfigurationCodecPort for FakeConfiguration {}

    fn engine_id(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine ID is valid")
    }

    struct UnboundScopeSource;

    impl CoreScopeSource for UnboundScopeSource {
        fn scope_observation(&self) -> CoreScopeObservation {
            CoreScopeObservation::Unavailable
        }
    }

    fn fake_ports(
        selected_engine: EngineId,
        process_engine: EngineId,
        codec_engine: EngineId,
    ) -> Result<ProductEngineBackendPorts, PortError> {
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: codec_engine,
            format_id: ProfileFormatId::new("fake-yaml").expect("test format is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("test schema version is valid"),
        };
        ProductEngineBackendPorts::new(
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
            EngineOperationPorts::unsupported("fake core operations are not composed"),
            ProductEngineOperationPorts::unsupported("fake product operations are not composed"),
        )
    }

    #[test]
    fn product_bundle_accepts_matching_process_and_configuration_adapters() {
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

    #[tokio::test]
    async fn unsupported_product_operations_need_no_scope_and_keep_local_ports_available() {
        let engine = engine_id("sing-box");
        let ports = fake_ports(engine.clone(), engine.clone(), engine.clone()).unwrap();
        let operations = ports.operations();

        assert!(matches!(
            ports.core_operations().outbound(),
            Err(PortError::Unsupported { .. })
        ));
        assert!(matches!(
            ports.core_operations().routing_intent(),
            Err(PortError::Unsupported { .. })
        ));
        assert_eq!(
            ports
                .bind_owner_core_scope(Arc::new(UnboundScopeSource))
                .unwrap(),
            EngineBackendOwnerScopeBindingResult::NotRequired
        );
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("test-instance").expect("test instance is valid"),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let rule_error = operations
            .rule_provider()
            .inventory(scope.clone())
            .await
            .unwrap_err();
        assert!(matches!(rule_error, PortError::Rejected(_)));
        let geo_error = operations.geo_database().trigger(scope).await.unwrap_err();
        assert!(matches!(geo_error, PortError::Rejected(_)));
        assert_eq!(ports.process().core_identity().unwrap().engine, engine);
        assert_eq!(ports.configuration().scope().engine_id, engine);
    }

    #[test]
    fn product_bundle_rejects_process_from_another_engine() {
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
    fn product_bundle_rejects_configuration_codec_from_another_engine() {
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
    fn product_bundle_rejects_transaction_with_a_different_format_or_schema() {
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
        let error = match ProductEngineBackendPorts::new(
            engine.clone(),
            Arc::new(FakeProcess {
                identity: CoreIdentity::new(
                    engine.clone(),
                    CoreInstanceId::new("test-instance").expect("test instance is valid"),
                    None,
                ),
            }),
            Arc::new(FakeReadiness),
            Arc::new(FakeConfiguration {
                scope: codec_scope.clone(),
            }),
            Arc::new(ProfileConfigurationTransactionBinding::new(
                ProfileConfigurationTransactionScope::new(transaction_scope),
            )),
            EngineOperationPorts::unsupported("fake core operations are not composed"),
            ProductEngineOperationPorts::unsupported("fake product operations are not composed"),
        ) {
            Ok(_) => panic!("transaction and codec scopes must be identical"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }

    #[test]
    fn product_bundle_rejects_core_operations_scoped_to_another_engine() {
        let engine = engine_id("mihomo");
        let codec_scope = ProfileDocumentCodecScope {
            engine_id: engine.clone(),
            format_id: ProfileFormatId::new("fake-yaml").expect("test format is valid"),
            schema_version: ProfileSchemaVersion::new(1).expect("test schema version is valid"),
        };
        let foreign_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id("sing-box")));
        let error = match ProductEngineBackendPorts::new(
            engine.clone(),
            Arc::new(FakeProcess {
                identity: CoreIdentity::new(
                    engine.clone(),
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
            EngineOperationPorts::Available {
                outbound: Arc::new(FakeOutbound),
                routing_intent: Arc::new(FakeRoutingIntent),
                owner_scope: foreign_scope,
            },
            ProductEngineOperationPorts::unsupported("fake product operations are not composed"),
        ) {
            Ok(_) => panic!("operation owner scope must match the requested engine"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }

    struct FakeOutbound;

    #[async_trait]
    impl OutboundPort for FakeOutbound {
        async fn inventory(
            &self,
            _scope: CoreScope,
        ) -> Result<OutboundInventorySnapshot, PortError> {
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "test outbound is not composed",
            ))
        }

        async fn select(
            &self,
            _scope: CoreScope,
            _expected_revision: SnapshotRevision,
            _collection_id: OutboundId,
            _outbound_id: OutboundId,
        ) -> Result<OutboundInventorySnapshot, PortError> {
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "test outbound is not composed",
            ))
        }

        async fn probe(
            &self,
            _scope: CoreScope,
            _expected_revision: SnapshotRevision,
            _outbound_id: OutboundId,
            _request: OutboundProbeRequest,
        ) -> Result<OutboundProbeSnapshot, PortError> {
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "test outbound is not composed",
            ))
        }
    }

    struct FakeRoutingIntent;

    #[async_trait]
    impl RoutingIntentPort for FakeRoutingIntent {
        async fn observe(&self, _scope: &CoreScope) -> Result<RoutingIntent, PortError> {
            Err(PortError::unsupported(
                Capability::RoutingIntent,
                "test routing intent is not composed",
            ))
        }

        async fn apply(
            &self,
            _scope: &CoreScope,
            _intent: RoutingIntent,
        ) -> Result<RoutingIntent, PortError> {
            Err(PortError::unsupported(
                Capability::RoutingIntent,
                "test routing intent is not composed",
            ))
        }
    }
}
