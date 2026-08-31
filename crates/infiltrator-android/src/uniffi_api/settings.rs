//! Persisted engine settings exposed as patchable records: DNS configuration
//! (servers, enhanced mode, fallback filter) and Fake-IP configuration
//! (range, filter list, cache store) plus cache clearing.

use infiltrator_core::dns::{load_dns_config, save_dns_config};
use infiltrator_core::fake_ip::{
    clear_fake_ip_cache, load_fake_ip_config, save_fake_ip_config};

use super::support::{get_runtime, map_anyhow_error, normalize_optional_string, sanitize_list};
use crate::ffi::{FfiBoolResult, FfiErrorCode, FfiStatus};

// --- DNS API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettings {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    pub nameserver: Vec<String>,
    pub default_nameserver: Vec<String>,
    pub fallback: Vec<String>,
    pub fallback_filter: Option<DnsFallbackFilterSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettingsPatch {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    pub nameserver: Option<Vec<String>>,
    pub default_nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
    pub fallback_filter: Option<DnsFallbackFilterSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsFallbackFilterSettings {
    pub geoip: Option<bool>,
    pub geoip_code: Option<String>,
    pub ipcidr: Vec<String>,
    pub domain: Vec<String>,
    pub domain_suffix: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<DnsSettings>,
}

// --- Fake-IP API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettings {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Vec<String>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettingsPatch {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<FakeIpSettings>,
}

#[uniffi::export]
pub async fn dns_settings() -> DnsSettingsResult {
    get_runtime()
        .spawn(async move {
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
        })
        .await
        .unwrap_or_else(|e| DnsSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn dns_settings_save(patch: DnsSettingsPatch) -> DnsSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_dns_settings(patch).await {
                Ok(settings) => DnsSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => DnsSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| DnsSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn fake_ip_settings() -> FakeIpSettingsResult {
    get_runtime()
        .spawn(async move {
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
        })
        .await
        .unwrap_or_else(|e| FakeIpSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn fake_ip_settings_save(patch: FakeIpSettingsPatch) -> FakeIpSettingsResult {
    get_runtime()
        .spawn(async move {
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
        })
        .await
        .unwrap_or_else(|e| FakeIpSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn fake_ip_cache_clear() -> FfiBoolResult {
    get_runtime()
        .spawn(async move {
            match clear_fake_ip_cache().await.map_err(map_anyhow_error) {
                Ok(removed) => FfiBoolResult::ok(removed),
                Err(status) => FfiBoolResult {
                    status,
                    value: false,
                },
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiBoolResult::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

async fn load_dns_settings() -> Result<DnsSettings, FfiStatus> {
    let config = load_dns_config().await.map_err(map_anyhow_error)?;
    Ok(build_dns_settings(config))
}

async fn save_dns_settings(patch: DnsSettingsPatch) -> Result<DnsSettings, FfiStatus> {
    let core_patch = build_dns_settings_patch(patch);
    let config = save_dns_config(core_patch)
        .await
        .map_err(map_anyhow_error)?;
    Ok(build_dns_settings(config))
}

fn build_dns_settings(config: infiltrator_core::dns::DnsConfig) -> DnsSettings {
    DnsSettings {
        enable: config.enable,
        ipv6: config.ipv6,
        enhanced_mode: config.enhanced_mode,
        nameserver: config.nameserver.unwrap_or_default(),
        default_nameserver: config.default_nameserver.unwrap_or_default(),
        fallback: config.fallback.unwrap_or_default(),
        fallback_filter: config
            .fallback_filter
            .map(core_dns_fallback_filter_to_record),
    }
}

pub(super) fn build_dns_settings_patch(patch: DnsSettingsPatch) -> infiltrator_core::dns::DnsConfigPatch {
    infiltrator_core::dns::DnsConfigPatch {
        enable: patch.enable,
        ipv6: patch.ipv6,
        enhanced_mode: normalize_optional_string(patch.enhanced_mode),
        nameserver: sanitize_list(patch.nameserver),
        default_nameserver: sanitize_list(patch.default_nameserver),
        fallback: sanitize_list(patch.fallback),
        fallback_filter: patch
            .fallback_filter
            .map(record_to_core_dns_fallback_filter),
        ..infiltrator_core::dns::DnsConfigPatch::default()
    }
}

pub(super) fn core_dns_fallback_filter_to_record(
    filter: infiltrator_core::dns::DnsFallbackFilter,
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
) -> infiltrator_core::dns::DnsFallbackFilter {
    infiltrator_core::dns::DnsFallbackFilter {
        geoip: filter.geoip,
        geoip_code: normalize_optional_string(filter.geoip_code),
        ipcidr: sanitize_list(Some(filter.ipcidr)),
        domain: sanitize_list(Some(filter.domain)),
        domain_suffix: sanitize_list(Some(filter.domain_suffix)),
    }
}

async fn load_fake_ip_settings() -> Result<FakeIpSettings, FfiStatus> {
    let config = load_fake_ip_config().await.map_err(map_anyhow_error)?;
    Ok(build_fake_ip_settings(config))
}

async fn save_fake_ip_settings(patch: FakeIpSettingsPatch) -> Result<FakeIpSettings, FfiStatus> {
    let core_patch = build_fake_ip_settings_patch(patch);
    let config = save_fake_ip_config(core_patch)
        .await
        .map_err(map_anyhow_error)?;
    Ok(build_fake_ip_settings(config))
}

fn build_fake_ip_settings(config: infiltrator_core::fake_ip::FakeIpConfig) -> FakeIpSettings {
    FakeIpSettings {
        fake_ip_range: config.fake_ip_range,
        fake_ip_filter: config.fake_ip_filter.unwrap_or_default(),
        store_fake_ip: config.store_fake_ip,
    }
}

fn build_fake_ip_settings_patch(patch: FakeIpSettingsPatch) -> infiltrator_core::fake_ip::FakeIpConfigPatch {
    infiltrator_core::fake_ip::FakeIpConfigPatch {
        fake_ip_range: normalize_optional_string(patch.fake_ip_range),
        fake_ip_filter: sanitize_list(patch.fake_ip_filter),
        store_fake_ip: patch.store_fake_ip,
    }
}
