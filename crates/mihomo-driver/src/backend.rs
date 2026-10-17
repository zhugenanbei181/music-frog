//! Ready-made Mihomo engine backend provider.
//!
//! A host that only needs the engine-neutral [`EngineBackendPorts`] bundle
//! composes [`MihomoEngineBackendProvider`] instead of assembling the
//! controller client, process supervision, readiness, document codec and
//! scoped outbound/routing adapters itself.
//!
//! Creating a backend has no process, network or filesystem side effect: the
//! Core still starts only through an explicit `CoreProcess` operation. The
//! operation adapters receive an unbound owner-scope binding and fail closed
//! until the host binds its lifecycle owner through
//! `EngineBackendPorts::bind_owner_core_scope`.

use crate::api::client::MihomoClient;
use crate::api::client::validate_loopback_controller;
use crate::codec::profile_document_codec::MihomoProfileDocumentCodec;
use crate::driver::ControllerReadiness;
use crate::driver::ProcessCoreController;
use crate::outbound::MihomoOutboundAdapter;
use crate::routing_intent::MihomoRoutingIntentAdapter;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::engine_backend::EngineBackendOwnerScopeBinding;
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::engine_backend::EngineBackendProvider;
use lurker_core_ports::engine_backend::EngineOperationPorts;
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

/// Readiness bound used when the host does not choose its own.
pub const DEFAULT_READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Already-resolved host inputs for one Mihomo backend.
///
/// The driver never resolves product paths: the host supplies the installed
/// binary, the active profile document, the data directory that owns the PID
/// record and engine logs, and the loopback controller coordinates.
pub struct MihomoBackendOptions {
    binary_path: PathBuf,
    config_path: PathBuf,
    home_dir: PathBuf,
    controller_endpoint: String,
    secret: Option<String>,
    readiness_timeout: Duration,
}

impl MihomoBackendOptions {
    /// Validates the host inputs with the default readiness bound.
    pub fn new(
        binary_path: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>,
        home_dir: impl Into<PathBuf>,
        controller_endpoint: impl Into<String>,
        secret: Option<String>,
    ) -> Result<Self, PortError> {
        Self::with_readiness_timeout(
            binary_path,
            config_path,
            home_dir,
            controller_endpoint,
            secret,
            DEFAULT_READINESS_TIMEOUT,
        )
    }

    /// Validates the host inputs together with an explicit readiness bound.
    pub fn with_readiness_timeout(
        binary_path: impl Into<PathBuf>,
        config_path: impl Into<PathBuf>,
        home_dir: impl Into<PathBuf>,
        controller_endpoint: impl Into<String>,
        secret: Option<String>,
        readiness_timeout: Duration,
    ) -> Result<Self, PortError> {
        let binary_path = binary_path.into();
        let config_path = config_path.into();
        let home_dir = home_dir.into();
        let controller_endpoint = controller_endpoint.into();

        if binary_path.as_os_str().is_empty() {
            return Err(PortError::Failed(
                "Mihomo backend binary path must not be empty".to_owned(),
            ));
        }
        if config_path.as_os_str().is_empty() {
            return Err(PortError::Failed(
                "Mihomo backend config path must not be empty".to_owned(),
            ));
        }
        if home_dir.as_os_str().is_empty() {
            return Err(PortError::Failed(
                "Mihomo backend home directory must not be empty".to_owned(),
            ));
        }
        if readiness_timeout.is_zero() {
            return Err(PortError::Failed(
                "Mihomo backend readiness timeout must be greater than zero".to_owned(),
            ));
        }
        validate_controller_endpoint(&controller_endpoint)?;

        Ok(Self {
            binary_path,
            config_path,
            home_dir,
            controller_endpoint,
            secret,
            readiness_timeout,
        })
    }

    pub fn binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn home_dir(&self) -> &PathBuf {
        &self.home_dir
    }

    pub fn controller_endpoint(&self) -> &str {
        &self.controller_endpoint
    }

    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    /// Bound the host applies while waiting for controller readiness after it
    /// explicitly starts the Core. Bundle creation never waits.
    pub const fn readiness_timeout(&self) -> Duration {
        self.readiness_timeout
    }
}

fn validate_controller_endpoint(endpoint: &str) -> Result<(), PortError> {
    let url = Url::parse(endpoint).map_err(|error| {
        PortError::Failed(format!(
            "Mihomo backend controller endpoint is invalid: {error}"
        ))
    })?;
    validate_loopback_controller(&url).map_err(|error| PortError::Failed(error.to_string()))
}

/// Creates the neutral Mihomo backend bundle from validated host inputs.
pub struct MihomoEngineBackendProvider {
    options: MihomoBackendOptions,
}

impl MihomoEngineBackendProvider {
    pub fn new(options: MihomoBackendOptions) -> Self {
        Self { options }
    }
}

impl EngineBackendProvider<EngineBackendPorts> for MihomoEngineBackendProvider {
    fn engine_id(&self) -> EngineId {
        EngineId::mihomo()
    }

    fn create_backend(&self) -> Result<EngineBackendPorts, PortError> {
        let engine_id = self.engine_id();
        let client = MihomoClient::new(
            self.options.controller_endpoint(),
            self.options.secret().map(str::to_owned),
        )
        .map_err(|error| PortError::Failed(error.to_string()))?;
        let process = ProcessCoreController::with_home(
            self.options.binary_path().clone(),
            self.options.config_path().clone(),
            self.options.home_dir().clone(),
        );
        let readiness = ControllerReadiness::new(
            self.options.controller_endpoint(),
            self.options.secret().map(str::to_owned),
        );
        let configuration = MihomoProfileDocumentCodec::new();

        let owner_scope = Arc::new(EngineBackendOwnerScopeBinding::new(engine_id.clone()));
        let outbound: Arc<dyn OutboundPort> = Arc::new(MihomoOutboundAdapter::new(
            Arc::new(client.clone()),
            owner_scope.clone(),
        ));
        let routing_intent: Arc<dyn RoutingIntentPort> =
            Arc::new(MihomoRoutingIntentAdapter::new(client));
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
