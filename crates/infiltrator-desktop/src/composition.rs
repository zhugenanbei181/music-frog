//! Desktop composition root helpers.
//!
//! This module is the only place in the desktop host that assembles a
//! Tokio-backed application service with a concrete process and controller
//! adapter. UI crates receive the resulting `CoreApplication` handle.

use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::version_application::VersionApplication;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;

use crate::service::ServiceManager;

/// Build the 0.30 lifecycle application over the desktop process host and
/// Mihomo controller readiness adapter.
pub fn core_application(
    service: &ServiceManager,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> anyhow::Result<CoreApplication> {
    let controller_url = controller_url.into();
    let client = MihomoClient::new(&controller_url, secret.clone())?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let application = CoreApplication::new_with_overview(
        service.core_process(),
        std::sync::Arc::new(ControllerReadiness::new(controller_url, secret)),
        std::sync::Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime,
    );
    // Keep the Bevy command seam live in the desktop composition: version
    // rollback is an application use-case, not a UI-local file operation.
    let versions = VersionApplication::new(std::sync::Arc::new(crate::storage::version()?));
    let service_mode = infiltrator_application::service_mode_application::ServiceModeApplication::new(
        std::sync::Arc::new(crate::service_mode::DesktopServiceMode::new(
            service.binary_path().to_path_buf(),
        )),
    );
    let port_conflicts = PortConflictApplication::new(std::sync::Arc::new(
        crate::storage::port_conflict()?,
    ));
    let pac = PacApplication::new(
        std::sync::Arc::new(client.clone()),
        std::sync::Arc::new(crate::pac_service::DesktopPacServicePort::shared()),
    );
    let network_roaming = NetworkRoamingApplication::new(
        std::sync::Arc::new(crate::network_roaming::DesktopNetworkRoamingPort::shared()),
        Some(std::sync::Arc::new(client.clone())),
    );
    application.install_command_handler(std::sync::Arc::new(
        CommandApplication::new()
            .with_runtime(std::sync::Arc::new(client.clone()))
            .with_mtu(MtuApplication::new(std::sync::Arc::new(
                crate::mtu::DesktopMtuProbe::new(),
            )))
            .with_system_proxy(SystemProxyApplication::new(std::sync::Arc::new(
                crate::system_proxy::DesktopSystemProxy::new(),
            )))
            .with_uwp_loopback(UwpLoopbackApplication::new(std::sync::Arc::new(
                crate::uwp_loopback_port::DesktopUwpLoopbackPort,
            )))
            .with_pac(pac)
            .with_network_roaming(network_roaming)
            .with_versions(versions)
            .with_service_mode(service_mode)
            .with_port_conflicts(port_conflicts),
    ));
    Ok(application)
}
