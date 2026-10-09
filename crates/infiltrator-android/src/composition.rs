//! Android host/application composition for the 0.30 seam.

use crate::native_host::clipboard::AndroidClipboardPort;
use crate::runtime::AndroidBridgeAdapter;
use crate::vpn_service::AndroidVpnServicePort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::overview::UnavailableOverviewReader;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_ports::core_process::CoreReadiness;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_platform::android_bridge::AndroidBridge;
use std::sync::Arc;

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
    let (reader, gateway): (Arc<dyn OverviewReader>, Option<Arc<dyn RuntimeGateway>>) =
        match MihomoClient::new(&controller_url, secret.clone()) {
            Ok(client) => (
                Arc::new(ControllerOverviewReader::new(client.clone())) as Arc<dyn OverviewReader>,
                Some(Arc::new(client) as Arc<dyn RuntimeGateway>),
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
    let application =
        CoreApplication::new_with_overview(process, readiness, reader, runtime.clone());
    let mtu = MtuApplication::new(Arc::new(AndroidBridgeAdapter::new(bridge.clone())));
    let mut handler = CommandApplication::new()
        .with_application_runtime(runtime)
        .with_mtu(mtu)
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )));
    // BANDROID-008: the shared import command reads the real system clipboard
    // through the registered Activity channel. The port resolves lazily, so a
    // composition built before the Activity registers still reaches it; until
    // then the command keeps its typed unsupported result.
    handler = handler.with_import_source(Arc::new(AndroidClipboardPort::from_registry()));
    if let Some(gateway) = gateway {
        application
            .install_log_gateway(gateway.clone())
            .expect("controller log worker must start");
        handler = handler
            .with_runtime(gateway)
            .with_logs(application.log_application());
    }
    application.install_command_handler(Arc::new(handler));
    application
}
