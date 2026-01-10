pub mod api;
pub mod ffi;
pub mod runtime;

pub use api::AndroidApi;
pub use ffi::{FfiApi, FfiBoolResult, FfiErrorCode, FfiStatus, FfiStringResult};
pub use runtime::{android_bridge_adapter, AndroidBridge, AndroidBridgeAdapter, AndroidRuntime};

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
