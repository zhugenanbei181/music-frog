pub mod api;
pub mod domain_snapshot;
pub mod ffi;
#[cfg(target_os = "android")]
mod jni_bridge;
pub mod runtime;
mod tls;
mod uniffi_api;
pub mod vpn_route;

pub use api::AndroidApi;
pub use ffi::{FfiApi, FfiBoolResult, FfiErrorCode, FfiStatus, FfiStringResult};
pub use mihomo_platform::android_bridge::{
    AndroidBridge, clear_android_bridge, get_android_bridge, set_android_bridge,
};
pub use runtime::{AndroidBridgeAdapter, AndroidRuntime, android_bridge_adapter};
pub use uniffi_api::{
    BootstrapResult, BootstrapStepRecord, ConnectionRecord, ConnectionsResult,
    DoctorCheckMetaRecord, DoctorCheckMetaResult, DoctorCheckResultRecord, DoctorFixActionRecord,
    DoctorFixResult, DoctorReportRecord, DnsFallbackFilterSettings, DnsSettings, DnsSettingsPatch,
    DnsSettingsResult, FakeIpSettings, FakeIpSettingsPatch, FakeIpSettingsResult, IpCheckResult,
    IpResult, ProfileSummary, ProfilesResult, ProxyGroupSummary, ProxyGroupsResult,
    RuleEntryRecord, RuleProvidersResult, RulesResult, TrafficResult, TrafficSnapshot,
    TunStatusResult, VpnTunSettings, VpnTunSettingsPatch, VpnTunSettingsResult, WebDavSettings,
    WebDavSettingsResult, WebDavSyncResult,
};

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
