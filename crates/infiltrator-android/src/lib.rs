pub mod composition;
mod configuration_candidate_validator;
mod core_launch_spec;
pub mod domain_snapshot;
pub mod ffi;
mod host_session;
mod host_support;
#[cfg(target_os = "android")]
mod jni_bridge;
#[cfg(target_os = "android")]
mod jni_clipboard;
pub mod native_host;
pub mod product;
pub mod runtime;
pub mod service_init;
#[cfg(all(target_os = "android", feature = "slint-ui"))]
mod slint_host;
mod tls;
mod uniffi_api;
pub mod vpn_route;
pub mod vpn_service;
#[cfg(all(target_os = "android", feature = "slint-ui"))]
use infiltrator_slint::run_with_host;
#[cfg(all(target_os = "android", feature = "slint-ui"))]
use slint::android;
#[cfg(all(target_os = "android", feature = "slint-ui"))]
use std::sync::Arc;

pub use ffi::{
    FfiBoolResult, FfiErrorCode, FfiFailureLocalization, FfiFailureLocalizationKey, FfiStatus,
    FfiStringResult,
};
pub use mihomo_platform::android_bridge::{
    AndroidBridge, CoreProcessStartReceipt, CoreProcessStopReceipt, clear_android_bridge,
    get_android_bridge, set_android_bridge,
};
pub use runtime::{AndroidBridgeAdapter, AndroidRuntime, android_bridge_adapter};
pub use uniffi_api::{
    AndroidAppRoutingMode, AndroidAppRoutingPackageSelection, AndroidAppRoutingResult,
    AndroidAppRoutingSnapshot, AndroidControlIntent, AndroidCoreLifecycle,
    AndroidCoreLifecycleResult, AndroidHostProtocolVersion, AndroidHostReply, AndroidHostRequest,
    AndroidJsonEntry, AndroidJsonValue, AndroidOwnerDataDomain, AndroidOwnerDataSnapshot,
    AndroidOwnerDataSnapshotData, AndroidOwnerOperation, AndroidOwnerOperationClientResult,
    AndroidOwnerOperationContext, AndroidOwnerOperationIpcResult, AndroidOwnerOperationIpcState,
    AndroidOwnerOperationProtocolVersion, AndroidOwnerOperationReadback,
    AndroidOwnerOperationReply, AndroidRoutingIntentCopyKey, AndroidRuleEntry,
    AndroidRuleProviderEntry, AndroidRuleProvidersResult, AndroidRulesResult,
    AndroidVpnConfigurationActivationLease, AndroidVpnConfigurationActivationReleaseResult,
    AndroidVpnConfigurationActivationResult, AndroidVpnRoutingActivationLease,
    AndroidVpnRoutingActivationReleaseResult, AndroidVpnRoutingActivationResult,
    AndroidVpnRoutingSnapshot, BootstrapResult, BootstrapStepRecord, DnsFallbackFilterSettings,
    DnsSettings, DnsSettingsPatch, DnsSettingsResult, DoctorCheckMetaRecord, DoctorCheckMetaResult,
    DoctorCheckResultRecord, DoctorFixActionRecord, DoctorFixResult, DoctorReportRecord,
    FakeIpSettings, FakeIpSettingsPatch, FakeIpSettingsResult, IpCheckResult, IpResult,
    ProfileSummary, ProfilesResult, VpnAuthorizationResult, VpnPreparationResult, VpnTunSettings,
    VpnTunSettingsPatch, VpnTunSettingsResult, VpnTunStack, VpnTunStackUpdate, WebDavSettings,
    WebDavSettingsResult, WebDavSyncResult, android_begin_vpn_configuration_activation,
    android_begin_vpn_routing_activation, android_control, android_core_lifecycle_status,
    android_dispatch_owner_operation_wire_json, android_engine_defined_routing_intent_copy,
    android_execute_owner_operation, android_execute_product_command_json,
    android_finish_vpn_configuration_activation, android_finish_vpn_routing_activation,
    android_owner_data_revision, android_routing_intent_copy, android_surface_snapshot_json,
    android_vpn_routing_snapshot, cancel_vpn_preparation, prepare_vpn, start_vpn,
    vpn_authorization_result,
};
pub use vpn_service::AndroidVpnServicePort;

/// NativeActivity entry point for the registered Slint Android flavor. The
/// same `libinfiltrator_android.so` also exports the existing JNI bridge used
/// by `MihomoApplication`, so the Slint UI and host registration share one
/// process-local bridge registry.
#[cfg(all(target_os = "android", feature = "slint-ui"))]
#[unsafe(no_mangle)]
pub fn android_main(app: android::AndroidApp) {
    if let Err(error) = android::init(app) {
        eprintln!("Slint Android backend initialization failed: {error}");
        return;
    }
    if let Err(error) = run_with_host(Arc::new(slint_host::AndroidSlintHost)) {
        eprintln!("Slint Android surface failed: {error}");
    }
}

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
