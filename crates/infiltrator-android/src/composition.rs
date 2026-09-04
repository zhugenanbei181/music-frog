//! Android host/application composition for the 0.30 seam.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_ports::core_process::CoreReadiness;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_platform::android_bridge::AndroidBridge;

use crate::runtime::AndroidBridgeAdapter;

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
    let process = std::sync::Arc::new(AndroidBridgeAdapter::new(bridge));
    let readiness: std::sync::Arc<dyn CoreReadiness> =
        std::sync::Arc::new(ControllerReadiness::new(controller_url, secret));
    CoreApplication::new(process, readiness)
}

