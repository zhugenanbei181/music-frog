//! UniFFI FFI surface consumed by the Android (Kotlin) side. This file is
//! the module root only: the exported functions/records/enums live in
//! semantic submodules and are re-exported here so the `crate::uniffi_api`
//! paths (and the generated FFI names) stay stable.

mod app_routing;
mod control;
mod controller;
mod core_owner_session;
mod doctor;
mod localization;
mod logs;
mod owner_operation;
mod product;
mod profiles;
mod rules;
mod settings;
mod vpn;
mod webdav;

// Public record surface re-exported by the crate root (lib.rs).
pub use app_routing::{
    AndroidAppRoutingMode, AndroidAppRoutingPackageSelection, AndroidAppRoutingResult,
    AndroidAppRoutingSnapshot, AndroidVpnRoutingActivationLease,
    AndroidVpnRoutingActivationReleaseResult, AndroidVpnRoutingActivationResult,
    AndroidVpnRoutingSnapshot, android_begin_vpn_routing_activation,
    android_finish_vpn_routing_activation, android_vpn_routing_snapshot,
};
pub use control::{
    AndroidControlIntent, AndroidCoreLifecycle, AndroidCoreLifecycleResult,
    AndroidHostProtocolVersion, AndroidHostReply, AndroidHostRequest, VpnAuthorizationResult,
    android_control, android_core_lifecycle_status, vpn_authorization_result,
};
pub use controller::{IpCheckResult, IpResult};
pub use core_owner_session::{android_begin_core_owner_session, android_close_core_owner};
pub use doctor::{
    BootstrapResult, BootstrapStepRecord, DoctorCheckMetaRecord, DoctorCheckMetaResult,
    DoctorCheckResultRecord, DoctorFixActionRecord, DoctorFixResult, DoctorReportRecord,
};
pub use localization::{
    AndroidRoutingIntentCopyKey, android_engine_defined_routing_intent_copy,
    android_routing_intent_copy,
};
pub use owner_operation::wire::{
    android_dispatch_owner_operation_wire_json, android_execute_owner_operation,
};
pub use owner_operation::{
    AndroidOwnerDataDomain, AndroidOwnerDataSnapshot, AndroidOwnerDataSnapshotData,
    AndroidOwnerOperation, AndroidOwnerOperationClientResult, AndroidOwnerOperationContext,
    AndroidOwnerOperationIpcResult, AndroidOwnerOperationIpcState,
    AndroidOwnerOperationProtocolVersion, AndroidOwnerOperationReadback,
    AndroidOwnerOperationReply, android_owner_data_revision,
};
pub use product::{android_execute_product_command_json, android_surface_snapshot_json};
pub use profiles::{ProfileSummary, ProfilesResult};
pub use rules::{
    AndroidJsonEntry, AndroidJsonValue, AndroidRuleEntry, AndroidRuleProviderEntry,
    AndroidRuleProvidersResult, AndroidRulesResult,
};
pub use settings::{
    DnsFallbackFilterSettings, DnsSettings, DnsSettingsPatch, DnsSettingsResult, FakeIpSettings,
    FakeIpSettingsPatch, FakeIpSettingsResult,
};
pub use vpn::{
    AndroidVpnConfigurationActivationLease, AndroidVpnConfigurationActivationReleaseResult,
    AndroidVpnConfigurationActivationResult, VpnPreparationResult, VpnTunSettings,
    VpnTunSettingsPatch, VpnTunSettingsResult, VpnTunStack, VpnTunStackUpdate,
    android_begin_vpn_configuration_activation, android_finish_vpn_configuration_activation,
    cancel_vpn_preparation, prepare_vpn, start_vpn,
};
pub use webdav::{WebDavSettings, WebDavSettingsResult, WebDavSyncResult};

#[cfg(test)]
use settings::{
    build_dns_settings_patch, core_dns_fallback_filter_to_record,
    record_to_core_dns_fallback_filter,
};
#[cfg(test)]
use vpn::build_tun_patch;

use crate::ffi::FfiStatus;
use infiltrator_platform::android_bridge::{clear_android_bridge, get_android_bridge};

#[uniffi::export]
pub fn ping() -> String {
    "ok".to_string()
}

#[uniffi::export]
pub fn bridge_ready() -> bool {
    get_android_bridge().is_some()
}

#[uniffi::export]
pub fn bridge_shutdown() -> FfiStatus {
    clear_android_bridge();
    FfiStatus::ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use infiltrator_domain::dns::FallbackFilter;

    #[test]
    fn test_build_tun_patch_includes_stack_and_auto_detect_interface() {
        let patch = VpnTunSettingsPatch {
            mtu: None,
            auto_route: None,
            strict_route: None,
            stack_update: VpnTunStackUpdate::Set {
                stack: VpnTunStack::Gvisor,
            },
            auto_detect_interface: Some(true),
        };

        let core_patch = build_tun_patch(&patch);
        assert_eq!(
            core_patch.stack,
            infiltrator_contract::tun::TunStackUpdate::Set(
                infiltrator_contract::tun::TunStack::Gvisor,
            ),
        );
        assert_eq!(core_patch.auto_detect_interface, Some(true));
    }

    #[test]
    fn test_build_dns_settings_patch_maps_fallback_filter() {
        let patch = DnsSettingsPatch {
            enable: Some(true),
            ipv6: Some(false),
            enhanced_mode: Some(" fake-ip ".to_string()),
            nameserver: Some(vec![" 1.1.1.1 ".to_string(), " ".to_string()]),
            default_nameserver: Some(vec!["".to_string()]),
            fallback: Some(vec!["https://dns.example/dns-query".to_string()]),
            fallback_filter: Some(DnsFallbackFilterSettings {
                geoip: Some(true),
                geoip_code: Some(" CN ".to_string()),
                ipcidr: vec![" 240.0.0.0/4 ".to_string(), "".to_string()],
                domain: vec!["example.com".to_string()],
                domain_suffix: vec![" internal ".to_string()],
            }),
        };

        let core_patch = build_dns_settings_patch(patch);
        assert_eq!(core_patch.enable, Some(true));
        assert_eq!(core_patch.ipv6, Some(false));
        assert_eq!(core_patch.enhanced_mode, Some("fake-ip".to_string()));
        assert_eq!(core_patch.nameserver, Some(vec!["1.1.1.1".to_string()]));
        assert_eq!(core_patch.default_nameserver, Some(Vec::new()));
        assert_eq!(
            core_patch.fallback,
            Some(vec!["https://dns.example/dns-query".to_string()])
        );

        let filter = core_patch
            .fallback_filter
            .expect("fallback_filter should exist");
        assert_eq!(filter.geoip, Some(true));
        assert_eq!(filter.geoip_code, Some("CN".to_string()));
        assert_eq!(filter.ipcidr, Some(vec!["240.0.0.0/4".to_string()]));
        assert_eq!(filter.domain, Some(vec!["example.com".to_string()]));
        assert_eq!(filter.domain_suffix, Some(vec!["internal".to_string()]));
    }

    #[test]
    fn test_dns_fallback_filter_roundtrip_record_conversion() {
        let core = FallbackFilter {
            geoip: Some(false),
            geoip_code: Some("US".to_string()),
            ipcidr: Some(vec!["198.18.0.0/16".to_string()]),
            domain: Some(vec!["example.org".to_string()]),
            domain_suffix: Some(vec!["local".to_string()]),
            geosite: None,
        };

        let record = core_dns_fallback_filter_to_record(core.clone());
        assert_eq!(record.geoip, Some(false));
        assert_eq!(record.geoip_code, Some("US".to_string()));
        assert_eq!(record.ipcidr, vec!["198.18.0.0/16".to_string()]);
        assert_eq!(record.domain, vec!["example.org".to_string()]);
        assert_eq!(record.domain_suffix, vec!["local".to_string()]);

        let converted = record_to_core_dns_fallback_filter(record);
        assert_eq!(converted.geoip, core.geoip);
        assert_eq!(converted.geoip_code, core.geoip_code);
        assert_eq!(converted.ipcidr, core.ipcidr);
        assert_eq!(converted.domain, core.domain);
        assert_eq!(converted.domain_suffix, core.domain_suffix);
    }
}
