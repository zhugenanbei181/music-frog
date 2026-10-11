//! Persisted engine settings exposed as patchable records: DNS configuration
//! (servers, enhanced mode, fallback filter) and Fake-IP configuration
//! (range, filter list, cache store) plus cache clearing.

use crate::ffi::{FfiBoolResult, FfiStatus};
use crate::host_support::{
    build_configuration_application, cache_application, map_application_failure,
    normalize_optional_string, sanitize_list,
};
use infiltrator_domain::{dns, fake_ip};

// --- DNS API ---

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsSettings {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    /// Preserves which DNS nameserver field was configured for composite
    /// owner operations such as the Android VPN settings readback.
    pub nameserver_configured: bool,
    pub nameserver: Vec<String>,
    pub default_nameserver: Vec<String>,
    pub fallback: Vec<String>,
    pub fallback_filter: Option<DnsFallbackFilterSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct DnsSettingsPatch {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    pub nameserver: Option<Vec<String>>,
    pub default_nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub fallback_filter: Option<DnsFallbackFilterSettings>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct DnsFallbackFilterSettings {
    pub geoip: Option<bool>,
    pub geoip_code: Option<String>,
    pub ipcidr: Vec<String>,
    pub domain: Vec<String>,
    pub domain_suffix: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct DnsSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<DnsSettings>,
}

// --- Fake-IP API ---

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct FakeIpSettings {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Vec<String>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct FakeIpSettingsPatch {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct FakeIpSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<FakeIpSettings>,
}

pub(super) async fn read_dns_settings_local() -> DnsSettingsResult {
    match load_dns_settings().await {
        Ok(settings) => DnsSettingsResult {
            status: FfiStatus::ok(),
            settings: Some(settings),
        },
        Err(status) => DnsSettingsResult {
            status,
            settings: None,
        },
    }
}

pub(super) async fn read_fake_ip_settings_local() -> FakeIpSettingsResult {
    match load_fake_ip_settings().await {
        Ok(settings) => FakeIpSettingsResult {
            status: FfiStatus::ok(),
            settings: Some(settings),
        },
        Err(status) => FakeIpSettingsResult {
            status,
            settings: None,
        },
    }
}

pub(super) async fn save_fake_ip_settings_local(
    patch: FakeIpSettingsPatch,
) -> FakeIpSettingsResult {
    match save_fake_ip_settings(patch).await {
        Ok(settings) => FakeIpSettingsResult {
            status: FfiStatus::ok(),
            settings: Some(settings),
        },
        Err(status) => FakeIpSettingsResult {
            status,
            settings: None,
        },
    }
}

pub(super) async fn clear_fake_ip_cache_local() -> FfiBoolResult {
    match cache_application()
        .clear_fake_ip()
        .await
        .map_err(map_application_failure)
    {
        Ok(removed) => FfiBoolResult::ok(removed),
        Err(status) => FfiBoolResult {
            status,
            value: false,
        },
    }
}

async fn load_dns_settings() -> Result<DnsSettings, FfiStatus> {
    let config = build_configuration_application()
        .await?
        .load_dns_config()
        .await
        .map_err(map_application_failure)?;
    Ok(build_dns_settings(config))
}

pub(super) async fn dns_settings_save_local(
    patch: DnsSettingsPatch,
) -> Result<DnsSettings, FfiStatus> {
    let core_patch = build_dns_settings_patch(patch);
    let config = build_configuration_application()
        .await?
        .save_dns_config(core_patch)
        .await
        .map_err(map_application_failure)?;
    Ok(build_dns_settings(config))
}

fn build_dns_settings(config: dns::DnsConfig) -> DnsSettings {
    let nameserver_configured = config.nameserver.is_some();
    DnsSettings {
        enable: config.enable,
        ipv6: config.ipv6,
        enhanced_mode: config.enhanced_mode,
        nameserver_configured,
        nameserver: config.nameserver.unwrap_or_default(),
        default_nameserver: config.default_nameserver.unwrap_or_default(),
        fallback: config.fallback.unwrap_or_default(),
        fallback_filter: config
            .fallback_filter
            .map(core_dns_fallback_filter_to_record),
    }
}

pub(super) fn build_dns_settings_patch(patch: DnsSettingsPatch) -> dns::DnsConfigPatch {
    dns::DnsConfigPatch {
        enable: patch.enable,
        ipv6: patch.ipv6,
        enhanced_mode: normalize_optional_string(patch.enhanced_mode),
        nameserver: sanitize_list(patch.nameserver),
        default_nameserver: sanitize_list(patch.default_nameserver),
        fallback: sanitize_list(patch.fallback),
        fallback_filter: patch
            .fallback_filter
            .map(record_to_core_dns_fallback_filter),
        ..dns::DnsConfigPatch::default()
    }
}

pub(super) fn core_dns_fallback_filter_to_record(
    filter: dns::FallbackFilter,
) -> DnsFallbackFilterSettings {
    DnsFallbackFilterSettings {
        geoip: filter.geoip,
        geoip_code: filter.geoip_code,
        ipcidr: filter.ipcidr.unwrap_or_default(),
        domain: filter.domain.unwrap_or_default(),
        domain_suffix: filter.domain_suffix.unwrap_or_default(),
    }
}

pub(super) fn record_to_core_dns_fallback_filter(
    filter: DnsFallbackFilterSettings,
) -> dns::FallbackFilter {
    dns::FallbackFilter {
        geoip: filter.geoip,
        geoip_code: normalize_optional_string(filter.geoip_code),
        ipcidr: sanitize_list(Some(filter.ipcidr)),
        domain: sanitize_list(Some(filter.domain)),
        domain_suffix: sanitize_list(Some(filter.domain_suffix)),
        geosite: None,
    }
}

async fn load_fake_ip_settings() -> Result<FakeIpSettings, FfiStatus> {
    let config = build_configuration_application()
        .await?
        .load_fake_ip_config()
        .await
        .map_err(map_application_failure)?;
    Ok(build_fake_ip_settings(config))
}

async fn save_fake_ip_settings(patch: FakeIpSettingsPatch) -> Result<FakeIpSettings, FfiStatus> {
    let core_patch = build_fake_ip_settings_patch(patch);
    let config = build_configuration_application()
        .await?
        .save_fake_ip_config(core_patch)
        .await
        .map_err(map_application_failure)?;
    Ok(build_fake_ip_settings(config))
}

fn build_fake_ip_settings(config: fake_ip::FakeIpConfig) -> FakeIpSettings {
    FakeIpSettings {
        fake_ip_range: config.fake_ip_range,
        fake_ip_filter: config.fake_ip_filter.unwrap_or_default(),
        store_fake_ip: config.store_fake_ip,
    }
}

fn build_fake_ip_settings_patch(patch: FakeIpSettingsPatch) -> fake_ip::FakeIpConfigPatch {
    fake_ip::FakeIpConfigPatch {
        fake_ip_range: normalize_optional_string(patch.fake_ip_range),
        fake_ip_filter: sanitize_list(patch.fake_ip_filter),
        fake_ip_filter_mode: None,
        store_fake_ip: patch.store_fake_ip,
        ipv6_fake_ip_range: None,
    }
}
