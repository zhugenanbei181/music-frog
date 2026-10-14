//! Independent public-IP probe exposed to Android surfaces.

use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::{get_runtime, map_application_failure, network_application};

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpCheckResult {
    pub ip: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpResult {
    pub status: FfiStatus,
    pub value: Option<IpCheckResult>,
}

/// Public-IP probing is not a Mihomo controller read and remains an
/// independent network application operation.
#[uniffi::export]
pub async fn ip_check() -> IpResult {
    get_runtime()
        .spawn(async move {
            match fetch_ip_check().await {
                Ok(value) => IpResult {
                    status: FfiStatus::ok(),
                    value: Some(value),
                },
                Err(status) => IpResult {
                    status,
                    value: None,
                },
            }
        })
        .await
        .unwrap_or_else(|error| IpResult {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("runtime join error: {error}"),
            ),
            value: None,
        })
}

async fn fetch_ip_check() -> Result<IpCheckResult, FfiStatus> {
    let snapshot = network_application()
        .probe_public_ip_direct()
        .await
        .map_err(map_application_failure)?;
    Ok(IpCheckResult {
        ip: snapshot.ip,
        country: snapshot.country,
        region: snapshot.region,
        city: snapshot.city,
    })
}
