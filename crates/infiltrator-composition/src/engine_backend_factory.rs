//! Engine selection for product composition roots.

use infiltrator_contract::core_identity::{CoreInstanceId, EngineId};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::engine_backend_factory::{
    ENGINE_BACKEND_CAPABILITY, EngineBackendFactory, EngineBackendOwnerScopeBinding,
    EngineBackendOwnerScopeBindingResult, EngineBackendPorts, EngineBackendProvider,
    EngineBackendProviderAvailability, EngineOperationPort, EngineOperationPorts,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::OutboundPort;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use singbox_api::clash_api::SingBoxClashApiClient;
use singbox_api::clash_runtime::{SingBoxOutboundAdapter, SingBoxRoutingIntentAdapter};
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::http::{ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort};
use singbox_api::readiness::SingBoxControllerReadiness;
use singbox_config::config::SingBoxProfileConfigurationAdapter;
use singbox_config::profile_configuration_transaction::SingBoxProfileConfigurationTransactionAdapter;
use singbox_config::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_process::command::{SingBoxCommandPort, TokioSingBoxCommandPort};
use singbox_process::process::{SingBoxCoreOperationGate, SingBoxProcess};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Dispatches each engine identity to its registered process/config/runtime
/// adapter factory. A missing engine is rejected before any provider runs.
pub struct EngineBackendFactoryRegistry {
    providers: HashMap<EngineId, Arc<dyn EngineBackendProvider>>,
}

impl EngineBackendFactoryRegistry {
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn EngineBackendProvider>>,
    ) -> Result<Self, PortError> {
        let mut by_engine = HashMap::new();
        for provider in providers {
            let engine_id = provider.engine_id();
            if by_engine.insert(engine_id.clone(), provider).is_some() {
                return Err(PortError::Failed(format!(
                    "duplicate backend factory for engine {}",
                    engine_id.as_str()
                )));
            }
        }
        Ok(Self {
            providers: by_engine,
        })
    }
}

impl EngineBackendFactory for EngineBackendFactoryRegistry {
    fn provider_availability(&self, engine_id: &EngineId) -> EngineBackendProviderAvailability {
        match self.providers.contains_key(engine_id) {
            true => EngineBackendProviderAvailability::Registered,
            false => EngineBackendProviderAvailability::Unsupported {
                reason: format!(
                    "no process/config/runtime backend is registered for {}",
                    engine_id.as_str()
                ),
            },
        }
    }

    fn backend_ports(&self, engine_id: &EngineId) -> Result<EngineBackendPorts, PortError> {
        let provider = self.providers.get(engine_id).ok_or_else(|| {
            PortError::unsupported(
                ENGINE_BACKEND_CAPABILITY,
                format!(
                    "no process/config/runtime backend is registered for {}",
                    engine_id.as_str()
                ),
            )
        })?;
        let ports = provider.create_backend()?;
        if ports.engine_id() != engine_id {
            return Err(PortError::Failed(format!(
                "backend factory for {} returned ports for {}",
                engine_id.as_str(),
                ports.engine_id().as_str()
            )));
        }
        Ok(ports)
    }
}

/// Production sing-box backend provider for one process/config/controller
/// identity. The returned engine bundle carries scoped Clash outbound and
/// routing ports. The host binds the Core owner and native profile transaction
/// after constructing its lifecycle actor.
pub struct SingBoxEngineBackendProvider {
    ports: EngineBackendPorts,
    configuration: Arc<SingBoxProfileConfigurationAdapter>,
    operation_gate: Arc<SingBoxCoreOperationGate>,
}

impl SingBoxEngineBackendProvider {
    pub fn with_tokio(
        binary_path: PathBuf,
        config_path: PathBuf,
        instance_id: CoreInstanceId,
        controller: SingBoxControllerConfiguration,
    ) -> Result<Self, PortError> {
        Self::new(
            binary_path,
            config_path,
            instance_id,
            controller,
            Arc::new(TokioSingBoxCommandPort),
            Arc::new(ReqwestSingBoxClashHttpPort),
        )
    }

    pub fn new(
        binary_path: PathBuf,
        config_path: PathBuf,
        instance_id: CoreInstanceId,
        controller: SingBoxControllerConfiguration,
        command: Arc<dyn SingBoxCommandPort>,
        http: Arc<dyn SingBoxClashHttpPort>,
    ) -> Result<Self, PortError> {
        let engine_id = EngineId::sing_box();
        let operation_gate = Arc::new(SingBoxCoreOperationGate::new());
        let process: Arc<dyn CoreProcess> = Arc::new(SingBoxProcess::new_with_operation_gate(
            binary_path.clone(),
            config_path.clone(),
            instance_id,
            controller.address(),
            Arc::clone(&command),
            Arc::clone(&operation_gate),
        )?);
        let api = SingBoxClashApiClient::new(controller.clone(), Arc::clone(&http))?;
        let readiness: Arc<dyn CoreReadiness> =
            Arc::new(SingBoxControllerReadiness::new(controller, http)?);
        let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(SingBoxProfileDocumentCodec);
        let transaction = Arc::new(ProfileConfigurationTransactionBinding::new(
            ProfileConfigurationTransactionScope::new(codec.scope()),
        ));
        let configuration = Arc::new(SingBoxProfileConfigurationAdapter::new(
            binary_path,
            config_path,
            command,
        ));
        let owner_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id.clone()));
        let outbound: Arc<dyn OutboundPort> = Arc::new(SingBoxOutboundAdapter::new(
            api.clone(),
            owner_scope.clone(),
        ));
        let routing_intent: Arc<dyn RoutingIntentPort> =
            Arc::new(SingBoxRoutingIntentAdapter::new(api, owner_scope.clone()));
        let operations = EngineOperationPorts::available(
            outbound,
            routing_intent,
            EngineOperationPort::unsupported(
                "sing-box rule-provider operations have no verified adapter evidence",
            ),
            EngineOperationPort::unsupported(
                "sing-box Geo database trigger has no verified adapter evidence",
            ),
            owner_scope,
        );
        let ports = EngineBackendPorts::new(
            engine_id,
            process,
            readiness,
            codec,
            transaction,
            operations,
        )?;
        Ok(Self {
            ports,
            configuration,
            operation_gate,
        })
    }

    pub fn bind_profile_configuration_transaction(
        &self,
        ports: &EngineBackendPorts,
        profile_store: Arc<dyn ProfileStore>,
        session: Arc<dyn CoreLifecyclePort>,
    ) -> Result<(), PortError> {
        self.verify_bundle(ports)?;
        let process_identity = ports.process().core_identity().ok_or_else(|| {
            PortError::Failed("sing-box process adapter has no Core identity".to_owned())
        })?;
        let session_identity = session.core_identity().ok_or_else(|| {
            PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "sing-box configuration transaction requires an identified Core owner",
                false,
            ))
        })?;
        if session_identity.engine != process_identity.engine
            || session_identity.instance != process_identity.instance
        {
            return Err(PortError::Failed(
                "sing-box configuration transaction session does not own the provider process instance"
                    .to_owned(),
            ));
        }
        let transaction = Arc::new(SingBoxProfileConfigurationTransactionAdapter::new(
            profile_store,
            session,
            Arc::clone(&self.configuration),
            Arc::clone(&self.operation_gate),
            process_identity,
        )?);
        ports.configuration_transaction_binding().bind(transaction)
    }

    pub fn bind_core_owner_scope(
        &self,
        ports: &EngineBackendPorts,
        source: Arc<dyn CoreScopeSource>,
    ) -> Result<EngineBackendOwnerScopeBindingResult, PortError> {
        self.verify_bundle(ports)?;
        ports.bind_owner_core_scope(source)
    }

    fn verify_bundle(&self, ports: &EngineBackendPorts) -> Result<(), PortError> {
        let provider_binding = self.ports.configuration_transaction_binding();
        let bundle_binding = ports.configuration_transaction_binding();
        if ports.engine_id() == &self.engine_id() && Arc::ptr_eq(&provider_binding, &bundle_binding)
        {
            Ok(())
        } else {
            Err(PortError::Failed(
                "sing-box operation binding received a foreign backend bundle".to_owned(),
            ))
        }
    }
}

impl EngineBackendProvider for SingBoxEngineBackendProvider {
    fn engine_id(&self) -> EngineId {
        self.ports.engine_id().clone()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        Ok(self.ports.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::capability::Capability;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion,
    };
    use infiltrator_contract::routing_intent::RoutingIntent;
    use infiltrator_ports::core_scope::CoreScopeObservation;
    use singbox_api::http::{
        SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
    };
    use singbox_process::command::{NativeConfigCheck, SingBoxChildProcess, SingBoxCommandPort};
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tempfile::TempDir;

    struct FakeBackendProvider {
        engine_id: EngineId,
        calls: Arc<AtomicUsize>,
    }

    impl EngineBackendProvider for FakeBackendProvider {
        fn engine_id(&self) -> EngineId {
            self.engine_id.clone()
        }

        fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "fake backend intentionally unavailable",
            ))
        }
    }

    fn engine_id(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine id is valid")
    }

    struct FakeSingBoxCommand;

    #[async_trait]
    impl SingBoxCommandPort for FakeSingBoxCommand {
        async fn check_config(
            &self,
            _binary_path: &Path,
            _config_path: &Path,
        ) -> Result<NativeConfigCheck, PortError> {
            Ok(NativeConfigCheck::Accepted)
        }

        async fn spawn_core(
            &self,
            _binary_path: &Path,
            _config_path: &Path,
        ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
            Err(PortError::unsupported(
                Capability::CoreLifecycle,
                "factory test does not start Core",
            ))
        }
    }

    struct FakeSingBoxHttp {
        requests: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl SingBoxClashHttpPort for FakeSingBoxHttp {
        async fn request(
            &self,
            _url: &url::Url,
            _method: SingBoxClashMethod,
            _authorization: &SingBoxControllerAuthorization,
            _body: Option<Vec<u8>>,
            _timeout: Duration,
        ) -> Result<SingBoxClashResponse, PortError> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Err(PortError::unsupported(
                Capability::RoutingIntent,
                "factory test does not dispatch HTTP",
            ))
        }
    }

    struct MutableScope(Mutex<CoreScope>);

    impl MutableScope {
        fn set(&self, scope: CoreScope) {
            *self.0.lock().expect("test scope lock is not poisoned") = scope;
        }
    }

    impl CoreScopeSource for MutableScope {
        fn scope_observation(&self) -> CoreScopeObservation {
            match self.0.lock() {
                Ok(scope) => CoreScopeObservation::Observed(scope.clone()),
                Err(_) => CoreScopeObservation::Unavailable,
            }
        }
    }

    #[test]
    fn unknown_engine_is_typed_unsupported_without_constructing_any_backend() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let factory = EngineBackendFactoryRegistry::new([provider]).unwrap();

        let error = match factory.backend_ports(&engine_id("sing-box")) {
            Ok(_) => panic!("an unknown engine must not receive ports"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::CoreLifecycle,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn registered_engine_dispatches_to_its_factory_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let provider: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let factory = EngineBackendFactoryRegistry::new([provider]).unwrap();

        let error = match factory.backend_ports(&engine_id("mihomo")) {
            Ok(_) => panic!("the fake provider reports an unavailable backend"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::CoreLifecycle,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn duplicate_engine_registration_is_rejected() {
        let calls = Arc::new(AtomicUsize::new(0));
        let first: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let second: Arc<dyn EngineBackendProvider> = Arc::new(FakeBackendProvider {
            engine_id: engine_id("mihomo"),
            calls: Arc::clone(&calls),
        });
        let providers = vec![first, second];

        let error = match EngineBackendFactoryRegistry::new(providers) {
            Ok(_) => panic!("duplicate engine factories must be rejected"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn sing_box_provider_composes_scoped_clash_ports_without_http_side_effects() {
        let directory = TempDir::new().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(
            SingBoxEngineBackendProvider::new(
                directory.path().join("sing-box"),
                directory.path().join("active.json"),
                CoreInstanceId::new("singbox-provider-test").unwrap(),
                SingBoxControllerConfiguration::from_profile_json(
                    br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090","secret":"test-token"}}}"#,
                )
                .unwrap(),
                Arc::new(FakeSingBoxCommand),
                Arc::new(FakeSingBoxHttp {
                    requests: Arc::clone(&requests),
                }),
            )
            .unwrap(),
        );
        let provider_for_factory: Arc<dyn EngineBackendProvider> = provider.clone();
        let registry = EngineBackendFactoryRegistry::new([provider_for_factory]).unwrap();
        let engine = EngineId::sing_box();
        let backend = registry.backend_ports(&engine).unwrap();
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("singbox-provider-test").unwrap(),
                Some(CoreVersion::new("v1.14.3").unwrap()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let foreign_scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("another-singbox-instance").unwrap(),
                Some(CoreVersion::new("v1.14.3").unwrap()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };

        assert_eq!(backend.engine_id(), &engine);
        assert_eq!(backend.process().core_identity().unwrap().engine, engine);
        assert_eq!(backend.configuration().scope().engine_id, engine);
        assert!(matches!(
            backend.operations(),
            EngineOperationPorts::Available { .. }
        ));
        let wrong_source = Arc::new(MutableScope(Mutex::new(foreign_scope.clone())));
        assert!(matches!(
            provider.bind_core_owner_scope(&backend, wrong_source),
            Err(PortError::Failed(_))
        ));
        let owner_source = Arc::new(MutableScope(Mutex::new(scope.clone())));
        assert_eq!(
            provider
                .bind_core_owner_scope(&backend, owner_source.clone())
                .unwrap(),
            EngineBackendOwnerScopeBindingResult::Bound
        );
        assert!(matches!(
            provider.bind_core_owner_scope(&backend, owner_source.clone()),
            Err(PortError::Failed(_))
        ));

        let rule_providers = backend.operations().rule_provider();
        let error = rule_providers.inventory(scope.clone()).await.unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
        let geo = backend.operations().geo_database();
        let error = geo.trigger(scope.clone()).await.unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
        assert_eq!(requests.load(Ordering::SeqCst), 0);

        let routing = backend.operations().routing_intent().unwrap();
        let error = routing
            .apply(&scope, RoutingIntent::FollowRules)
            .await
            .unwrap_err();
        assert!(matches!(error, PortError::Unsupported { .. }));
        assert_eq!(requests.load(Ordering::SeqCst), 0);

        let outbound = backend.operations().outbound().unwrap();
        let error = outbound.inventory(scope.clone()).await.unwrap_err();
        assert!(matches!(error, PortError::Unsupported { .. }));
        assert_eq!(requests.load(Ordering::SeqCst), 1);

        owner_source.set(foreign_scope);
        let error = outbound.inventory(scope).await.unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }
}
