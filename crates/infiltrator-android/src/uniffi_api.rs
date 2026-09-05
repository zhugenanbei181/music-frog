//! UniFFI FFI surface consumed by the Android (Kotlin) side. This file is
//! the module root only: the exported functions/records/enums live in
//! semantic submodules and are re-exported here so the `crate::uniffi_api`
//! paths (and the generated FFI names) stay stable.

mod app_routing;
mod controller;
mod doctor;
mod logs;
mod profiles;
mod rules;
mod settings;
mod vpn;
mod webdav;

// Public record surface re-exported by the crate root (lib.rs).
pub use app_routing::{
    AndroidAppCategory, AndroidAppInfo, AndroidVpnPerAppPlan, MobileCloudProvider,
};
pub use controller::{
    ConnectionRecord, ConnectionsResult, IpCheckResult, IpResult, ProxyGroupSummary,
    ProxyGroupsResult, TrafficResult, TrafficSnapshot,
};
pub use doctor::{
    BootstrapResult, BootstrapStepRecord, DoctorCheckMetaRecord, DoctorCheckMetaResult,
    DoctorCheckResultRecord, DoctorFixActionRecord, DoctorFixResult, DoctorReportRecord,
};
pub use profiles::{ProfileSummary, ProfilesResult};
pub use rules::{RuleEntryRecord, RuleProvidersResult, RulesResult};
pub use settings::{
    DnsFallbackFilterSettings, DnsSettings, DnsSettingsPatch, DnsSettingsResult, FakeIpSettings,
    FakeIpSettingsPatch, FakeIpSettingsResult,
};
pub use vpn::{TunStatusResult, VpnTunSettings, VpnTunSettingsPatch, VpnTunSettingsResult};
pub use webdav::{WebDavSettings, WebDavSettingsResult, WebDavSyncResult};

// Names referenced by the unit tests below via `use super::*`.
#[cfg(test)]
use app_routing::{
    AppRoutingMode, app_routing_load, app_routing_save, app_routing_set_mode,
    app_routing_toggle_package,
};
#[cfg(test)]
use settings::{
    build_dns_settings_patch, core_dns_fallback_filter_to_record,
    record_to_core_dns_fallback_filter,
};
#[cfg(test)]
use vpn::build_tun_patch;

use crate::ffi::FfiStatus;
use mihomo_platform::android_bridge::{clear_android_bridge, get_android_bridge};

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
    use crate::ffi::FfiErrorCode;

    use mihomo_platform::TEST_LOCK;
    use mihomo_platform::paths::set_home_dir_override;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 串行化全局 home override：doctor.rs / support.rs 的测试同样切换
    /// override 并以 TEST_LOCK 互斥，这里保持同一把锁，避免并行测试在
    /// 「写配置 → 读回」之间被换走 home 目录。
    async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("infiltrator-android-{tag}-{unique}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    #[tokio::test]
    async fn test_app_routing_load_defaults() {
        let _guard = test_lock().await;
        let home = make_test_home("routing-default");
        set_home_dir_override(home.clone());

        let result = app_routing_load();
        assert_eq!(result.status.code, FfiErrorCode::Ok);
        let config = result.config.expect("config should be present");
        assert_eq!(config.mode, AppRoutingMode::ProxyAll);
        assert!(config.packages.is_empty());

        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn test_app_routing_set_mode_and_toggle_package() {
        let _guard = test_lock().await;
        let home = make_test_home("routing-toggle");
        set_home_dir_override(home.clone());

        let set_status = app_routing_set_mode(AppRoutingMode::ProxySelected);
        assert_eq!(set_status.code, FfiErrorCode::Ok);

        let toggle_first = app_routing_toggle_package("com.example.app".to_string());
        assert_eq!(toggle_first.status.code, FfiErrorCode::Ok);
        assert!(toggle_first.is_selected);

        let loaded = app_routing_load();
        assert_eq!(loaded.status.code, FfiErrorCode::Ok);
        let config = loaded.config.expect("config should be present");
        assert_eq!(config.mode, AppRoutingMode::ProxySelected);
        assert!(config.packages.contains(&"com.example.app".to_string()));

        let toggle_second = app_routing_toggle_package("com.example.app".to_string());
        assert_eq!(toggle_second.status.code, FfiErrorCode::Ok);
        assert!(!toggle_second.is_selected);

        let loaded_again = app_routing_load();
        assert_eq!(loaded_again.status.code, FfiErrorCode::Ok);
        let config_again = loaded_again.config.expect("config should be present");
        assert!(
            !config_again
                .packages
                .contains(&"com.example.app".to_string())
        );

        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn test_app_routing_save_deduplicates_packages() {
        let _guard = test_lock().await;
        let home = make_test_home("routing-save");
        set_home_dir_override(home.clone());

        let status = app_routing_save(
            AppRoutingMode::BypassSelected,
            vec![
                "com.alpha".to_string(),
                "com.alpha".to_string(),
                "com.beta".to_string(),
            ],
        );
        assert_eq!(status.code, FfiErrorCode::Ok);

        let loaded = app_routing_load();
        assert_eq!(loaded.status.code, FfiErrorCode::Ok);
        let config = loaded.config.expect("config should be present");
        assert_eq!(config.mode, AppRoutingMode::BypassSelected);
        assert_eq!(config.packages.len(), 2);
        assert!(config.packages.contains(&"com.alpha".to_string()));
        assert!(config.packages.contains(&"com.beta".to_string()));

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn test_build_tun_patch_includes_stack_and_auto_detect_interface() {
        let patch = VpnTunSettingsPatch {
            mtu: None,
            auto_route: None,
            strict_route: None,
            dns_servers: None,
            ipv6: None,
            stack: Some(" gvisor ".to_string()),
            auto_detect_interface: Some(true),
        };

        let (core_patch, has_patch) = build_tun_patch(&patch);
        assert!(has_patch);
        assert_eq!(core_patch.stack, Some("gvisor".to_string()));
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
        let core = infiltrator_domain::dns::DnsFallbackFilter {
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
