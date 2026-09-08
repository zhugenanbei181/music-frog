//! Android host/application composition for the 0.30 seam.

use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::overview::UnavailableOverviewReader;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_platform::android_bridge::AndroidBridge;
use std::sync::Arc;

use crate::runtime::AndroidBridgeAdapter;
use crate::vpn_service::AndroidVpnServicePort;

/// Compose the Android host's local-only startup proof for either UI surface.
pub fn offline_startup_application<B>(bridge: B) -> OfflineStartupApplication
where
    B: AndroidBridge + 'static,
{
    OfflineStartupApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge)))
}

/// Compose the Android native physical-link MTU observer. A bridge without
/// link metrics yields a typed unsupported result rather than a fake value.
pub fn mtu_application<B>(bridge: B) -> MtuApplication
where
    B: AndroidBridge + 'static,
{
    MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge)))
}

/// Assemble the shared application service with Android's bridge-backed Core
/// process port and the Mihomo controller readiness adapter.
pub fn core_application<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> CoreApplication
where
    B: AndroidBridge + 'static,
{
    let controller_url = controller_url.into();
    let bridge: Arc<dyn AndroidBridge> = Arc::new(bridge);
    let process = Arc::new(AndroidBridgeAdapter::new(bridge.clone()));
    let readiness: Arc<dyn CoreReadiness> = Arc::new(ControllerReadiness::new(
        controller_url.clone(),
        secret.clone(),
    ));
    let (reader, gateway): (
        Arc<dyn OverviewReader>,
        Option<Arc<dyn infiltrator_ports::runtime_gateway::RuntimeGateway>>,
    ) = match MihomoClient::new(&controller_url, secret.clone()) {
        Ok(client) => (
            Arc::new(ControllerOverviewReader::new(client.clone())) as Arc<dyn OverviewReader>,
            Some(Arc::new(client)
                as Arc<
                    dyn infiltrator_ports::runtime_gateway::RuntimeGateway,
                >),
        ),
        Err(error) => (
            Arc::new(UnavailableOverviewReader::new(PortError::Network(
                error.to_string(),
            ))) as Arc<dyn OverviewReader>,
            None,
        ),
    };
    let runtime = infiltrator_composition::tokio_application_runtime()
        .expect("Tokio application runtime must be constructible");
    let application = CoreApplication::new_with_overview(process, readiness, reader, runtime);
    let mtu = MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge.clone())));
    let mut handler = CommandApplication::new()
        .with_mtu(mtu)
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )));
    if let Some(gateway) = gateway {
        handler = handler.with_runtime(gateway);
    }
    application.install_command_handler(Arc::new(handler));
    application
}
