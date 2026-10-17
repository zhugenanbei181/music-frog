//! Shared production sing-box engine backend provider.

use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::engine_backend::{ProductEngineBackendPorts, ProductEngineOperationPorts};
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{CoreInstanceId, EngineId};
use lurker_core_ports::core_process::CoreProcess;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::core_scope::CoreScopeSource;
use lurker_core_ports::engine_backend::{
    EngineBackendOwnerScopeBinding, EngineBackendOwnerScopeBindingResult, EngineBackendProvider,
    EngineOperationPort, EngineOperationPorts,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use singbox_config::profile_configuration_transaction::SingBoxProfileConfigurationTransactionAdapter;
use singbox_config::profile_document_codec::SingBoxProfileConfigurationCodec;
use singbox_driver::api::clash_api::SingBoxClashApiClient;
use singbox_driver::api::controller_configuration::SingBoxControllerConfiguration;
use singbox_driver::api::http::{ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort};
use singbox_driver::codec::config::SingBoxProfileConfigurationAdapter;
use singbox_driver::driver::{SingBoxCoreOperationGate, SingBoxProcess};
use singbox_driver::outbound::SingBoxOutboundAdapter;
use singbox_driver::process::command::{SingBoxCommandPort, TokioSingBoxCommandPort};
use singbox_driver::readiness::SingBoxControllerReadiness;
use singbox_driver::routing_intent::SingBoxRoutingIntentAdapter;
use std::path::PathBuf;
use std::sync::Arc;

/// Production sing-box backend provider for one process/config/controller
/// identity. The returned engine bundle carries scoped Clash outbound and
/// routing ports. The host binds the Core owner and native profile transaction
/// after constructing its lifecycle actor.
pub struct SingBoxEngineBackendProvider {
    ports: ProductEngineBackendPorts,
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
        let codec: Arc<dyn ProfileConfigurationCodecPort> =
            Arc::new(SingBoxProfileConfigurationCodec::new());
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
        let operations = EngineOperationPorts::available(outbound, routing_intent, owner_scope);
        let product_operations = ProductEngineOperationPorts::available(
            EngineOperationPort::unsupported(
                "sing-box rule-provider operations have no verified adapter evidence",
            ),
            EngineOperationPort::unsupported(
                "sing-box Geo database trigger has no verified adapter evidence",
            ),
        );
        let ports = ProductEngineBackendPorts::new(
            engine_id,
            process,
            readiness,
            codec,
            transaction,
            operations,
            product_operations,
        )?;
        Ok(Self {
            ports,
            configuration,
            operation_gate,
        })
    }

    pub fn bind_profile_configuration_transaction(
        &self,
        ports: &ProductEngineBackendPorts,
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
        ports: &ProductEngineBackendPorts,
        source: Arc<dyn CoreScopeSource>,
    ) -> Result<EngineBackendOwnerScopeBindingResult, PortError> {
        self.verify_bundle(ports)?;
        ports.bind_owner_core_scope(source)
    }

    fn verify_bundle(&self, ports: &ProductEngineBackendPorts) -> Result<(), PortError> {
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

impl EngineBackendProvider<ProductEngineBackendPorts> for SingBoxEngineBackendProvider {
    fn engine_id(&self) -> EngineId {
        self.ports.engine_id().clone()
    }

    fn create_backend(&self) -> Result<ProductEngineBackendPorts, PortError> {
        Ok(self.ports.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lurker_core_contract::capability::Capability;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion,
    };
    use lurker_core_contract::routing_intent::RoutingIntent;
    use lurker_core_ports::core_scope::CoreScopeObservation;
    use lurker_engine_runtime::registry::EngineBackendProviderRegistry;
    use singbox_driver::api::http::{
        SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
    };
    use singbox_driver::process::command::{
        NativeConfigCheck, SingBoxChildProcess, SingBoxCommandPort,
    };
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tempfile::TempDir;

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
                "provider test does not start Core",
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
                "provider test does not dispatch HTTP",
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
        let provider_for_registry: Arc<dyn EngineBackendProvider<ProductEngineBackendPorts>> =
            provider.clone();
        let registry = EngineBackendProviderRegistry::new([provider_for_registry]).unwrap();
        let engine = EngineId::sing_box();
        let backend = registry.backend(&engine).unwrap();
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
            backend.core_operations(),
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

        let routing = backend.core_operations().routing_intent().unwrap();
        let error = routing
            .apply(&scope, RoutingIntent::FollowRules)
            .await
            .unwrap_err();
        assert!(matches!(error, PortError::Unsupported { .. }));
        assert_eq!(requests.load(Ordering::SeqCst), 0);

        let outbound = backend.core_operations().outbound().unwrap();
        let error = outbound.inventory(scope.clone()).await.unwrap_err();
        assert!(matches!(error, PortError::Unsupported { .. }));
        assert_eq!(requests.load(Ordering::SeqCst), 1);

        owner_source.set(foreign_scope);
        let error = outbound.inventory(scope).await.unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }
}
