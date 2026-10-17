//! Shared production Mihomo engine backend provider.

use infiltrator_ports::engine_backend::{ProductEngineBackendPorts, ProductEngineOperationPorts};
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationTransactionBinding, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::engine_backend::{
    EngineBackendOwnerScopeBinding, EngineBackendProvider, EngineOperationPort,
    EngineOperationPorts,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use mihomo_api::geo_database::MihomoGeoDatabaseAdapter;
use mihomo_api::rule_provider::MihomoRuleProviderAdapter;
use mihomo_config::profile_document_codec::MihomoProfileConfigurationCodec;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::outbound::MihomoOutboundAdapter;
use mihomo_driver::routing_intent::MihomoRoutingIntentAdapter;
use std::sync::Arc;

/// The controller can be unavailable during host setup. Keep that state
/// explicit so the provider cannot accidentally expose dispatchable operations.
enum MihomoControllerState {
    Available(MihomoClient),
    Unavailable(String),
}

/// Production provider for a host-owned Mihomo process and controller.
///
/// The process and readiness adapters remain host supplied. This provider is
/// the single owner of the shared Mihomo codec, outbound, and routing ports;
/// the host binds its typed native configuration transaction after creating
/// the Core lifecycle owner.
pub struct MihomoEngineBackendProvider {
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    controller: MihomoControllerState,
}

impl MihomoEngineBackendProvider {
    pub fn new(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        client: MihomoClient,
    ) -> Result<Self, PortError> {
        Self::build(process, readiness, MihomoControllerState::Available(client))
    }

    pub fn without_controller(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        reason: impl Into<String>,
    ) -> Result<Self, PortError> {
        Self::build(
            process,
            readiness,
            MihomoControllerState::Unavailable(reason.into()),
        )
    }

    fn build(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        controller: MihomoControllerState,
    ) -> Result<Self, PortError> {
        let identity = process.core_identity().ok_or_else(|| {
            PortError::Failed("Mihomo process provider has no Core identity".to_owned())
        })?;
        if identity.engine != mihomo_config::engine_id() {
            return Err(PortError::Failed(format!(
                "Mihomo provider received a process for {}",
                identity.engine.as_str()
            )));
        }
        Ok(Self {
            process,
            readiness,
            controller,
        })
    }
}

impl EngineBackendProvider<ProductEngineBackendPorts> for MihomoEngineBackendProvider {
    fn engine_id(&self) -> EngineId {
        mihomo_config::engine_id()
    }

    fn create_backend(&self) -> Result<ProductEngineBackendPorts, PortError> {
        let engine_id = self.engine_id();
        let codec: Arc<dyn ProfileConfigurationCodecPort> =
            Arc::new(MihomoProfileConfigurationCodec);
        let transaction = Arc::new(ProfileConfigurationTransactionBinding::new(
            ProfileConfigurationTransactionScope::new(codec.scope()),
        ));
        let (operations, product_operations) = match &self.controller {
            MihomoControllerState::Available(client) => {
                let owner_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id.clone()));
                let outbound: Arc<dyn OutboundPort> = Arc::new(MihomoOutboundAdapter::new(
                    Arc::new(client.clone()),
                    owner_scope.clone(),
                ));
                let routing: Arc<dyn RoutingIntentPort> =
                    Arc::new(MihomoRoutingIntentAdapter::new(client.clone()));
                let rule_provider = Arc::new(MihomoRuleProviderAdapter::new(
                    Arc::new(client.clone()),
                    owner_scope.clone(),
                ));
                let geo_database = Arc::new(MihomoGeoDatabaseAdapter::new(
                    Arc::new(client.clone()),
                    owner_scope.clone(),
                ));
                (
                    EngineOperationPorts::available(outbound, routing, owner_scope),
                    ProductEngineOperationPorts::available(
                        EngineOperationPort::available(rule_provider),
                        EngineOperationPort::available(geo_database),
                    ),
                )
            }
            MihomoControllerState::Unavailable(reason) => {
                let reason = format!("Mihomo typed engine operations are unavailable: {reason}");
                (
                    EngineOperationPorts::unsupported(reason.clone()),
                    ProductEngineOperationPorts::unsupported(reason),
                )
            }
        };
        ProductEngineBackendPorts::new(
            engine_id,
            Arc::clone(&self.process),
            Arc::clone(&self.readiness),
            codec,
            transaction,
            operations,
            product_operations,
        )
    }
}
