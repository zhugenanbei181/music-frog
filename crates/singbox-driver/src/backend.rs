//! Ready-made sing-box engine backend provider.
//!
//! A host that only needs the engine-neutral [`EngineBackendPorts`] bundle
//! composes [`SingBoxEngineBackendProvider`] instead of assembling the Clash
//! API client, process supervision, readiness, document codec and scoped
//! outbound/routing adapters itself.
//!
//! Creating a backend has no process, network or filesystem side effect: the
//! Core still starts only through an explicit `CoreProcess` operation. The
//! operation adapters receive an unbound owner-scope binding and fail closed
//! until the host binds its lifecycle owner through
//! `EngineBackendPorts::bind_owner_core_scope`.

use crate::api::clash_api::SingBoxClashApiClient;
use crate::api::controller_configuration::SingBoxControllerConfiguration;
use crate::codec::profile_document_codec::SingBoxProfileDocumentCodec;
use crate::driver::SingBoxProcess;
use crate::outbound::SingBoxOutboundAdapter;
use crate::readiness::SingBoxControllerReadiness;
use crate::routing_intent::SingBoxRoutingIntentAdapter;
use lurker_core_contract::identity::CoreInstanceId;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::engine_backend::EngineBackendOwnerScopeBinding;
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::engine_backend::EngineBackendProvider;
use lurker_core_ports::engine_backend::EngineOperationPorts;
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use serde_json::json;
use serde_json::to_vec;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Readiness bound used when the host does not choose its own.
pub const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Already-resolved host inputs for one sing-box backend.
///
/// The driver never resolves product paths: the host supplies the installed
/// binary, the active profile document, the stable Core instance identity and
/// the loopback Clash API coordinates that the profile declares.
pub struct SingBoxBackendOptions {
    binary_path: PathBuf,
    config_path: PathBuf,
    instance_id: CoreInstanceId,
    controller: SingBoxControllerConfiguration,
    readiness_timeout: Duration,
}

impl SingBoxBackendOptions {
    /// Validates the host inputs with the default readiness bound.
    pub fn new(
        binary_path: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>,
        instance_id: CoreInstanceId,
        controller_endpoint: impl Into<String>,
        secret: Option<String>,
    ) -> Result<Self, PortError> {
        Self::with_readiness_timeout(
            binary_path,
            config_path,
            instance_id,
            controller_endpoint,
            secret,
            DEFAULT_READINESS_TIMEOUT,
        )
    }

    /// Validates the host inputs together with an explicit readiness bound.
    pub fn with_readiness_timeout(
        binary_path: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>,
        instance_id: CoreInstanceId,
        controller_endpoint: impl Into<String>,
        secret: Option<String>,
        readiness_timeout: Duration,
    ) -> Result<Self, PortError> {
        let binary_path = binary_path.into();
        let config_path = config_path.into();
        let controller_endpoint = controller_endpoint.into();

        if binary_path.as_os_str().is_empty() {
            return Err(PortError::Failed(
                "sing-box backend binary path must not be empty".to_owned(),
            ));
        }
        if config_path.as_os_str().is_empty() {
            return Err(PortError::Failed(
                "sing-box backend config path must not be empty".to_owned(),
            ));
        }
        if readiness_timeout.is_zero() {
            return Err(PortError::Failed(
                "sing-box backend readiness timeout must be greater than zero".to_owned(),
            ));
        }
        let controller = controller_configuration(&controller_endpoint, secret.as_deref())?;

        Ok(Self {
            binary_path,
            config_path,
            instance_id,
            controller,
            readiness_timeout,
        })
    }

    pub fn binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn instance_id(&self) -> &CoreInstanceId {
        &self.instance_id
    }

    pub fn controller_address(&self) -> SocketAddr {
        self.controller.address()
    }

    pub fn controller_endpoint(&self) -> String {
        self.controller.endpoint()
    }

    /// Bound the host applies while waiting for Clash API readiness after it
    /// explicitly starts the Core. Bundle creation never waits.
    pub const fn readiness_timeout(&self) -> Duration {
        self.readiness_timeout
    }
}

/// Builds the driver's typed controller configuration from the same
/// endpoint/secret pair the profile declares.
fn controller_configuration(
    endpoint: &str,
    secret: Option<&str>,
) -> Result<SingBoxControllerConfiguration, PortError> {
    let document = json!({
        "experimental": {
            "clash_api": {
                "external_controller": endpoint,
                "secret": secret,
            }
        }
    });
    let raw = to_vec(&document).map_err(|error| {
        PortError::Failed(format!(
            "sing-box backend controller configuration could not be encoded: {error}"
        ))
    })?;
    SingBoxControllerConfiguration::from_profile_json(&raw).map_err(|error| {
        PortError::Failed(format!(
            "sing-box backend controller endpoint is invalid: {error}"
        ))
    })
}

/// Creates the neutral sing-box backend bundle from validated host inputs.
pub struct SingBoxEngineBackendProvider {
    options: SingBoxBackendOptions,
}

impl SingBoxEngineBackendProvider {
    pub fn new(options: SingBoxBackendOptions) -> Self {
        Self { options }
    }
}

impl EngineBackendProvider<EngineBackendPorts> for SingBoxEngineBackendProvider {
    fn engine_id(&self) -> EngineId {
        EngineId::sing_box()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        let engine_id = self.engine_id();
        let controller = self.options.controller.clone();
        let process = SingBoxProcess::with_tokio(
            self.options.binary_path().clone(),
            self.options.config_path().clone(),
            self.options.instance_id().clone(),
            controller.address(),
        )?;
        let api = SingBoxClashApiClient::with_reqwest(controller.clone())?;
        let readiness = SingBoxControllerReadiness::with_reqwest(controller)?;
        let configuration = SingBoxProfileDocumentCodec;

        let owner_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id.clone()));
        let outbound: Arc<dyn OutboundPort> = Arc::new(SingBoxOutboundAdapter::new(
            api.clone(),
            owner_scope.clone(),
        ));
        let routing_intent: Arc<dyn RoutingIntentPort> =
            Arc::new(SingBoxRoutingIntentAdapter::new(api, owner_scope.clone()));
        let operations = EngineOperationPorts::available(outbound, routing_intent, owner_scope);

        EngineBackendPorts::new(
            engine_id,
            Arc::new(process),
            Arc::new(readiness),
            Arc::new(configuration),
            operations,
        )
    }
}
