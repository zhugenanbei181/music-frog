pub mod api;
pub mod ffi;
pub mod runtime;
#[cfg(target_os = "android")]
mod jni_bridge;
mod tls;
mod uniffi_api;

pub use api::AndroidApi;
pub use ffi::{FfiApi, FfiBoolResult, FfiErrorCode, FfiStatus, FfiStringResult};
pub use uniffi_api::{
    IpCheckResult, IpResult, ProfileSummary, ProfilesResult, ProxyGroupSummary,
    ProxyGroupsResult, TrafficResult, TrafficSnapshot, TunStatusResult,
};
pub use mihomo_platform::{clear_android_bridge, get_android_bridge, set_android_bridge};
pub use runtime::{android_bridge_adapter, AndroidBridge, AndroidBridgeAdapter, AndroidRuntime};

uniffi::setup_scaffolding!("infiltrator_android");

pub struct AndroidHost {
    bridge: AndroidBridgeAdapter<Box<dyn AndroidBridge>>,
}

impl AndroidHost {
    pub fn new(bridge: Box<dyn AndroidBridge>) -> Self {
        Self {
            bridge: AndroidBridgeAdapter::new(bridge),
        }
    }

    pub fn bridge(&self) -> &AndroidBridgeAdapter<Box<dyn AndroidBridge>> {
        &self.bridge
    }
}
