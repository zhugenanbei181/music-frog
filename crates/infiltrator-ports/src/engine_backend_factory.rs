//! Engine-scoped process, configuration, and typed operation composition.

use crate::geo_database::{GeoDatabasePort, UnsupportedGeoDatabasePort};
#[cfg(test)]
use crate::profile_configuration_transaction::ProfileConfigurationTransactionScope;
use crate::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionPort,
};
use crate::profile_document_codec::ProfileConfigurationCodecPort;
use crate::rule_provider_runtime::{RuleProviderPort, UnsupportedRuleProviderPort};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::{CoreIdentity, CoreScope, EngineId};
use lurker_core_ports::core_process::CoreProcess;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::fmt::{self, Debug, Formatter};
use std::sync::Arc;
use std::sync::Mutex;

/// Scope source installed by the single Core owner after its lifecycle actor
/// has been constructed. Backend operation adapters receive this binding at
/// provider creation time and fail closed until the owner is attached.
pub struct EngineBackendOwnerScopeBinding {
    engine_id: EngineId,
    state: Mutex<OwnerScopeSourceState>,
}

enum OwnerScopeSourceState {
    Unbound,
    AwaitingSource {
        process_identity: CoreIdentity,
    },
    Bound {
        process_identity: CoreIdentity,
        source: Arc<dyn CoreScopeSource>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerScopeIdentityState {
    MatchingInstance,
    DifferentInstance,
}

impl EngineBackendOwnerScopeBinding {
    pub fn new(engine_id: EngineId) -> Self {
        Self {
            engine_id,
            state: Mutex::new(OwnerScopeSourceState::Unbound),
        }
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    pub fn bind(&self, source: Arc<dyn CoreScopeSource>) -> Result<(), PortError> {
        let process_identity = {
            let state = self.state.lock().map_err(|_| {
                PortError::Failed("engine backend owner scope lock is poisoned".to_owned())
            })?;
            match &*state {
                OwnerScopeSourceState::AwaitingSource { process_identity } => {
                    process_identity.clone()
                }
                OwnerScopeSourceState::Unbound => {
                    return Err(PortError::Failed(
                        "engine backend owner scope has no process identity binding".to_owned(),
                    ));
                }
                OwnerScopeSourceState::Bound { .. } => {
                    return Err(PortError::Failed(
                        "engine backend owner scope source is already bound".to_owned(),
                    ));
                }
            }
        };
        match source.scope_observation() {
            CoreScopeObservation::Observed(scope) => {
                match owner_scope_identity(&process_identity, &scope) {
                    OwnerScopeIdentityState::MatchingInstance => {}
                    OwnerScopeIdentityState::DifferentInstance => {
                        return Err(PortError::Failed(format!(
                            "{} backend operation scope was bound to instance {} instead of {}",
                            self.engine_id.as_str(),
                            scope.identity.instance.as_str(),
                            process_identity.instance.as_str()
                        )));
                    }
                }
            }
            CoreScopeObservation::Unavailable => {}
        }
        let mut state = self.state.lock().map_err(|_| {
            PortError::Failed("engine backend owner scope lock is poisoned".to_owned())
        })?;
        match &*state {
            OwnerScopeSourceState::AwaitingSource {
                process_identity: current_identity,
            } if current_identity == &process_identity => {
                *state = OwnerScopeSourceState::Bound {
                    process_identity,
                    source,
                };
                Ok(())
            }
            OwnerScopeSourceState::Unbound | OwnerScopeSourceState::AwaitingSource { .. } => {
                Err(PortError::Failed(
                    "engine backend owner scope process identity changed during binding".to_owned(),
                ))
            }
            OwnerScopeSourceState::Bound { .. } => Err(PortError::Failed(
                "engine backend owner scope source is already bound".to_owned(),
            )),
        }
    }

    fn bind_process_identity(&self, process_identity: CoreIdentity) -> Result<(), PortError> {
        if process_identity.engine != self.engine_id {
            return Err(PortError::Failed(format!(
                "{} backend owner scope received process identity for {}",
                self.engine_id.as_str(),
                process_identity.engine.as_str()
            )));
        }
        let mut state = self.state.lock().map_err(|_| {
            PortError::Failed("engine backend owner scope lock is poisoned".to_owned())
        })?;
        match &*state {
            OwnerScopeSourceState::Unbound => {
                *state = OwnerScopeSourceState::AwaitingSource { process_identity };
                Ok(())
            }
            OwnerScopeSourceState::AwaitingSource { .. } | OwnerScopeSourceState::Bound { .. } => {
                Err(PortError::Failed(
                    "engine backend owner scope process identity is already bound".to_owned(),
                ))
            }
        }
    }
}

impl CoreScopeSource for EngineBackendOwnerScopeBinding {
    fn scope_observation(&self) -> CoreScopeObservation {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return CoreScopeObservation::Unavailable,
        };
        match &*state {
            OwnerScopeSourceState::Unbound | OwnerScopeSourceState::AwaitingSource { .. } => {
                CoreScopeObservation::Unavailable
            }
            OwnerScopeSourceState::Bound {
                process_identity,
                source,
            } => match source.scope_observation() {
                CoreScopeObservation::Observed(scope) => {
                    match owner_scope_identity(process_identity, &scope) {
                        OwnerScopeIdentityState::MatchingInstance => {
                            CoreScopeObservation::Observed(scope)
                        }
                        OwnerScopeIdentityState::DifferentInstance => {
                            CoreScopeObservation::Unavailable
                        }
                    }
                }
                CoreScopeObservation::Unavailable => CoreScopeObservation::Unavailable,
            },
        }
    }
}

fn owner_scope_identity(
    process_identity: &CoreIdentity,
    scope: &CoreScope,
) -> OwnerScopeIdentityState {
    if scope.identity.engine == process_identity.engine
        && scope.identity.instance == process_identity.instance
    {
        OwnerScopeIdentityState::MatchingInstance
    } else {
        OwnerScopeIdentityState::DifferentInstance
    }
}

/// Outcome of attaching the single Core owner to this bundle's runtime
/// operations. Bundles without dispatchable operations need no owner scope;
/// process, lifecycle, and local configuration ports remain usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineBackendOwnerScopeBindingResult {
    NotRequired,
    Bound,
}

/// Typed runtime operations that are actually composed for one engine.
/// `Unsupported` has no adapter to dispatch; `Available` carries the scoped
/// outbound, routing, provider and Geo operation owners for this backend.
#[derive(Clone)]
pub enum EngineOperationPort<T> {
    Unsupported { reason: String },
    Available(T),
}

impl<T> EngineOperationPort<T> {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub fn available(port: T) -> Self {
        Self::Available(port)
    }
}

pub type RuleProviderOperationPort = EngineOperationPort<Arc<dyn RuleProviderPort>>;
pub type GeoDatabaseOperationPort = EngineOperationPort<Arc<dyn GeoDatabasePort>>;

#[derive(Clone)]
pub enum EngineOperationPorts {
    Unsupported {
        reason: String,
    },
    Available {
        outbound: Arc<dyn OutboundPort>,
        routing_intent: Arc<dyn RoutingIntentPort>,
        rule_provider: RuleProviderOperationPort,
        geo_database: GeoDatabaseOperationPort,
        owner_scope: Arc<EngineBackendOwnerScopeBinding>,
    },
}

impl EngineOperationPorts {
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub fn available(
        outbound: Arc<dyn OutboundPort>,
        routing_intent: Arc<dyn RoutingIntentPort>,
        rule_provider: RuleProviderOperationPort,
        geo_database: GeoDatabaseOperationPort,
        owner_scope: Arc<EngineBackendOwnerScopeBinding>,
    ) -> Self {
        Self::Available {
            outbound,
            routing_intent,
            rule_provider,
            geo_database,
            owner_scope,
        }
    }

    pub fn outbound(&self) -> Result<Arc<dyn OutboundPort>, PortError> {
        match self {
            Self::Unsupported { reason } => Err(PortError::unsupported(
                ENGINE_BACKEND_CAPABILITY,
                reason.clone(),
            )),
            Self::Available { outbound, .. } => Ok(Arc::clone(outbound)),
        }
    }

    pub fn routing_intent(&self) -> Result<Arc<dyn RoutingIntentPort>, PortError> {
        match self {
            Self::Unsupported { reason } => Err(PortError::unsupported(
                Capability::RoutingIntent,
                reason.clone(),
            )),
            Self::Available { routing_intent, .. } => Ok(Arc::clone(routing_intent)),
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

impl Debug for EngineOperationPorts {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { reason } => formatter
                .debug_struct("Unsupported")
                .field("reason", reason)
                .finish(),
            Self::Available { owner_scope, .. } => formatter
                .debug_struct("Available")
                .field("engine_id", owner_scope.engine_id())
                .finish_non_exhaustive(),
        }
    }
}

/// The engine-owned ports used to compose Core lifecycle and configuration.
///
/// `configuration` is the pure engine document codec. `configuration_transaction`
/// owns the separate native validate/apply/readback/rollback workflow and is
/// scoped to the exact same engine/format/schema identity.
/// `CoreReadiness` has no engine-ID accessor; its binding to this bundle's
/// engine is guaranteed by the selected engine-specific provider. Typed
/// outbound and routing operations are carried here. Broad controller
/// operations remain separately composed at the host boundary.
#[derive(Clone)]
pub struct EngineBackendPorts {
    engine_id: EngineId,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    configuration: Arc<dyn ProfileConfigurationCodecPort>,
    configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
    operations: EngineOperationPorts,
}

impl EngineBackendPorts {
    pub fn new(
        engine_id: EngineId,
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        configuration: Arc<dyn ProfileConfigurationCodecPort>,
        configuration_transaction: Arc<ProfileConfigurationTransactionBinding>,
        operations: EngineOperationPorts,
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
        if let EngineOperationPorts::Available { owner_scope, .. } = &operations
            && owner_scope.engine_id() != &engine_id
        {
            return Err(PortError::Failed(format!(
                "{} backend received operation ports scoped to {}",
                engine_id.as_str(),
                owner_scope.engine_id().as_str()
            )));
        }
        if let EngineOperationPorts::Available { owner_scope, .. } = &operations {
            owner_scope.bind_process_identity(process_identity)?;
        }

        Ok(Self {
            engine_id,
            process,
            readiness,
            configuration,
            configuration_transaction,
            operations,
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

    pub fn configuration(&self) -> Arc<dyn ProfileConfigurationCodecPort> {
        Arc::clone(&self.configuration)
    }

    pub fn configuration_transaction(&self) -> Arc<dyn ProfileConfigurationTransactionPort> {
        self.configuration_transaction.clone()
    }

    pub fn configuration_transaction_binding(&self) -> Arc<ProfileConfigurationTransactionBinding> {
        Arc::clone(&self.configuration_transaction)
    }

    pub fn operations(&self) -> EngineOperationPorts {
        self.operations.clone()
    }

    pub fn bind_owner_core_scope(
        &self,
        source: Arc<dyn CoreScopeSource>,
    ) -> Result<EngineBackendOwnerScopeBindingResult, PortError> {
        match &self.operations {
            EngineOperationPorts::Unsupported { .. } => {
                Ok(EngineBackendOwnerScopeBindingResult::NotRequired)
            }
            EngineOperationPorts::Available { owner_scope, .. } => {
                owner_scope.bind(source)?;
                Ok(EngineBackendOwnerScopeBindingResult::Bound)
            }
        }
    }
}

/// Creates the small-port set for one requested engine.
///
/// Implementations must return `Unsupported` before adapter side effects when
/// the requested engine is not implemented by that product composition.
pub trait EngineBackendFactory: Send + Sync {
    /// Inspect registration without invoking a provider. Switch coordinators
    /// use this to reject missing engines before terminating the current Core.
    fn provider_availability(&self, engine_id: &EngineId) -> EngineBackendProviderAvailability;
    fn backend_ports(&self, engine_id: &EngineId) -> Result<EngineBackendPorts, PortError>;
}

/// Typed result of inspecting whether a product composition can create an
/// engine backend. It is intentionally not a boolean readiness flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineBackendProviderAvailability {
    Registered,
    Unsupported { reason: String },
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
    use crate::profile_document_codec::ProfileConfigurationCodecPort;
    use async_trait::async_trait;
    use lurker_core_contract::core_process::CoreProcessStartContext;
    use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId};
    use lurker_core_contract::lifecycle::CoreLifecycle;
    use lurker_core_contract::profile_document::DocumentValidation;
    use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
    use lurker_core_ports::core_process::CoreReadiness;
    use lurker_core_ports::core_scope::CoreScopeObservation;
    use lurker_core_ports::profile_document_codec::{
        ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    };

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

    struct MutableScopeSource(Mutex<CoreScope>);

    impl MutableScopeSource {
        fn set(&self, scope: CoreScope) {
            *self.0.lock().expect("test scope lock is not poisoned") = scope;
        }
    }

    impl CoreScopeSource for MutableScopeSource {
        fn scope_observation(&self) -> CoreScopeObservation {
            match self.0.lock() {
                Ok(scope) => CoreScopeObservation::Observed(scope.clone()),
                Err(_) => CoreScopeObservation::Unavailable,
            }
        }
    }

    fn scope(engine: EngineId, instance: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                engine,
                CoreInstanceId::new(instance).expect("test instance is valid"),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }
    }

    #[test]
    fn owner_scope_binding_rejects_a_foreign_instance_and_fences_later_changes() {
        let engine = engine_id("sing-box");
        let process_identity = CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new("provider-instance").expect("test instance is valid"),
            None,
        );
        let binding = EngineBackendOwnerScopeBinding::new(engine.clone());
        binding
            .bind_process_identity(process_identity)
            .expect("backend binds its process identity before its owner scope");
        let foreign_scope = scope(engine.clone(), "foreign-instance");
        let foreign_source = Arc::new(MutableScopeSource(Mutex::new(foreign_scope.clone())));

        assert!(matches!(
            binding.bind(foreign_source),
            Err(PortError::Failed(_))
        ));

        let process_scope = scope(engine, "provider-instance");
        let source = Arc::new(MutableScopeSource(Mutex::new(process_scope.clone())));
        binding
            .bind(source.clone())
            .expect("matching process instance binds");
        assert_eq!(
            binding.scope_observation(),
            CoreScopeObservation::Observed(process_scope)
        );

        source.set(foreign_scope);
        assert_eq!(
            binding.scope_observation(),
            CoreScopeObservation::Unavailable
        );
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
            EngineOperationPorts::unsupported("fake operations are not composed"),
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
    fn unsupported_engine_operations_need_no_scope_and_keep_local_ports_available() {
        let engine = engine_id("sing-box");
        let ports = fake_ports(engine.clone(), engine.clone(), engine.clone()).unwrap();
        let operations = ports.operations();

        assert!(matches!(
            operations.outbound(),
            Err(PortError::Unsupported { .. })
        ));
        assert!(matches!(
            operations.routing_intent(),
            Err(PortError::Unsupported { .. })
        ));
        assert_eq!(
            ports
                .bind_owner_core_scope(Arc::new(UnboundScopeSource))
                .unwrap(),
            EngineBackendOwnerScopeBindingResult::NotRequired
        );
        assert_eq!(ports.process().core_identity().unwrap().engine, engine);
        assert_eq!(ports.configuration().scope().engine_id, engine);
    }

    struct UnboundScopeSource;

    impl CoreScopeSource for UnboundScopeSource {
        fn scope_observation(&self) -> CoreScopeObservation {
            CoreScopeObservation::Unavailable
        }
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
            EngineOperationPorts::unsupported("fake operations are not composed"),
        ) {
            Ok(_) => panic!("transaction and codec scopes must be identical"),
            Err(error) => error,
        };

        assert!(matches!(error, PortError::Failed(_)));
    }
}
