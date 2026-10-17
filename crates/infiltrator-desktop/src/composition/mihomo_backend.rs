//! Mihomo backend selection and concrete controller wiring for the Desktop product.

use crate::service::ServiceManager;
use infiltrator_composition::active_profile_backend::SelectedActiveProfileBackend;
use infiltrator_composition::mihomo_engine_backend_provider::MihomoEngineBackendProvider;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_ports::core_capability_provider::CoreCapabilityProvider;
use infiltrator_ports::engine_backend::ProductEngineBackendPorts;
use lurker_core_contract::capability::Capability;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::error::PortError;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::driver::ControllerReadiness;
use std::sync::Arc;

pub(crate) struct DesktopMihomoBackend {
    pub ports: ProductEngineBackendPorts,
    pub client: MihomoClient,
    pub capability_provider: Arc<dyn CoreCapabilityProvider>,
    pub selection: SelectedActiveProfileBackend,
}

pub(crate) fn desktop_mihomo_engine_provider(
    service: &ServiceManager,
    controller_url: &str,
    secret: Option<String>,
) -> Result<(Arc<MihomoEngineBackendProvider>, MihomoClient), PortError> {
    let client = MihomoClient::new(controller_url, secret.clone())
        .map_err(|error| PortError::Network(error.to_string()))?;
    let readiness: Arc<dyn CoreReadiness> =
        Arc::new(ControllerReadiness::new(controller_url.to_owned(), secret));
    let provider = Arc::new(MihomoEngineBackendProvider::new(
        service.core_process(),
        readiness,
        client.clone(),
    )?);
    Ok((provider, client))
}

pub(crate) async fn desktop_mihomo_backend_from_selected(
    client: MihomoClient,
    selected: SelectedActiveProfileBackend,
) -> Result<DesktopMihomoBackend, PortError> {
    if selected.reference.engine_id != MihomoVersionPort::engine_identifier()
        || selected.backend.engine_id() != &selected.reference.engine_id
    {
        let failure = PortError::unsupported(
            Capability::CoreLifecycle,
            "desktop Mihomo session received a selected backend for another engine",
        );
        return match selected.abort_before_core_application().await {
            Ok(_) => Err(failure),
            Err(cleanup) => Err(PortError::Failed(format!(
                "{failure}; aborting the unconstructed Mihomo candidate failed: {cleanup}"
            ))),
        };
    }
    let ports = selected.backend.clone();
    let capability_provider = selected.capability_provider();
    Ok(DesktopMihomoBackend {
        ports,
        client,
        capability_provider,
        selection: selected,
    })
}
