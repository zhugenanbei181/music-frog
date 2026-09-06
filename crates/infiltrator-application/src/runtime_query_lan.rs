//! Live Mihomo LAN listener and access-security operations.

use super::RuntimeQueryApplication;
use infiltrator_contract::error::Failure;
use infiltrator_contract::lan::{LanCredentials, LanSecuritySnapshot, LanSharingSnapshot};
use std::net::IpAddr;
use std::sync::atomic::Ordering;

impl RuntimeQueryApplication {
    /// Apply Mihomo's Allow-LAN listener settings as one live patch and verify
    /// every field through the controller readback before reporting success.
    pub async fn set_lan_sharing(
        &self,
        enabled: bool,
        mixed_port: u16,
        bind_address: &str,
    ) -> Result<LanSharingSnapshot, Failure> {
        if enabled && mixed_port == 0 {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidInput,
                "Allow-LAN requires a non-zero mixed proxy port",
                false,
            ));
        }
        let bind_address = canonical_bind_address(bind_address)?;
        self.gateway
            .patch_config(serde_json::json!({
                "allow-lan": enabled,
                "mixed-port": mixed_port,
                "bind-address": bind_address,
            }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        if observed.allow_lan != enabled
            || observed.mixed_port != mixed_port
            || !bind_address_matches(&bind_address, &observed.bind_address)
        {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "Allow-LAN readback mismatch: requested enabled={enabled}, mixed-port={mixed_port}, bind-address={bind_address}; observed enabled={}, mixed-port={}, bind-address={}",
                    observed.allow_lan, observed.mixed_port, observed.bind_address
                ),
                true,
            ));
        }
        Ok(LanSharingSnapshot::new(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observed.allow_lan,
            observed.mixed_port,
            canonical_bind_address(&observed.bind_address)?,
        ))
    }

    /// Apply LAN CIDR ACLs and one HTTP Basic credential in one live patch.
    /// Password material is used only to build the outbound request and is
    /// never returned in the readback snapshot.
    pub async fn set_lan_security(
        &self,
        allowed_ips: &[String],
        disallowed_ips: &[String],
        skip_auth_prefixes: &[String],
        authentication_enabled: bool,
        credentials: Option<&LanCredentials>,
    ) -> Result<LanSecuritySnapshot, Failure> {
        let allowed_ips = normalize_requested_cidrs(allowed_ips, "lan-allowed-ips")?;
        if allowed_ips.is_empty() {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidInput,
                "lan-allowed-ips must contain at least one CIDR",
                false,
            ));
        }
        let disallowed_ips = normalize_requested_cidrs(disallowed_ips, "lan-disallowed-ips")?;
        let skip_auth_prefixes = normalize_requested_cidrs(skip_auth_prefixes, "skip-auth-prefixes")?;

        let authentication = if authentication_enabled {
            let credentials = credentials.ok_or_else(|| {
                Failure::new(
                    infiltrator_contract::error::ErrorCode::InvalidInput,
                    "LAN authentication requires a username and password",
                    false,
                )
            })?;
            infiltrator_domain::lan_security::validate_credentials(
                &credentials.username,
                &credentials.password,
            )
            .map_err(|message| {
                Failure::new(
                    infiltrator_contract::error::ErrorCode::InvalidInput,
                    message,
                    false,
                )
            })?;
            vec![format!("{}:{}", credentials.username.trim(), credentials.password)]
        } else {
            Vec::new()
        };
        let expected_username = credentials
            .filter(|_| authentication_enabled)
            .map(|credentials| credentials.username.trim().to_owned());

        self.gateway
            .patch_config(serde_json::json!({
                "lan-allowed-ips": allowed_ips.clone(),
                "lan-disallowed-ips": disallowed_ips.clone(),
                "skip-auth-prefixes": skip_auth_prefixes.clone(),
                "authentication": authentication.clone(),
            }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        let observed_allowed = normalize_observed_cidrs(&observed.lan_allowed_ips, "lan-allowed-ips")?;
        let observed_disallowed = normalize_observed_cidrs(
            &observed.lan_disallowed_ips,
            "lan-disallowed-ips",
        )?;
        let observed_skip = normalize_observed_cidrs(
            &observed.skip_auth_prefixes,
            "skip-auth-prefixes",
        )?;
        if observed_allowed != allowed_ips
            || observed_disallowed != disallowed_ips
            || observed_skip != skip_auth_prefixes
            || observed.authentication_enabled != authentication_enabled
            || observed.authentication_user_count != authentication.len()
            || (authentication_enabled && observed.authentication_username != expected_username)
        {
            return Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                format!(
                    "LAN security readback mismatch: requested allow={allowed_ips:?}, deny={disallowed_ips:?}, skip={skip_auth_prefixes:?}, auth={authentication_enabled}; observed allow={observed_allowed:?}, deny={observed_disallowed:?}, skip={observed_skip:?}, auth={} users={}",
                    observed.authentication_enabled, observed.authentication_user_count
                ),
                true,
            ));
        }
        Ok(LanSecuritySnapshot::new(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observed_allowed,
            observed_disallowed,
            observed_skip,
            observed.authentication_enabled,
            observed.authentication_user_count,
            observed.authentication_username,
        ))
    }
}

fn canonical_bind_address(raw: &str) -> Result<String, Failure> {
    let value = raw.trim();
    if value == "*" {
        return Ok(value.to_owned());
    }
    let unbracketed = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    let address = unbracketed.parse::<IpAddr>().map_err(|_| {
        Failure::new(
            infiltrator_contract::error::ErrorCode::InvalidInput,
            format!("invalid Mihomo bind-address: {value}"),
            false,
        )
    })?;
    Ok(match address {
        IpAddr::V4(address) => address.to_string(),
        IpAddr::V6(address) => format!("[{address}]"),
    })
}

fn bind_address_matches(expected: &str, observed: &str) -> bool {
    canonical_bind_address(observed)
        .ok()
        .is_some_and(|observed| observed == expected)
}

fn normalize_requested_cidrs(values: &[String], field: &str) -> Result<Vec<String>, Failure> {
    infiltrator_domain::lan_security::normalize_cidrs(values, field).map_err(|message| {
        Failure::new(
            infiltrator_contract::error::ErrorCode::InvalidInput,
            message,
            false,
        )
    })
}

fn normalize_observed_cidrs(values: &[String], field: &str) -> Result<Vec<String>, Failure> {
    infiltrator_domain::lan_security::normalize_cidrs(values, field).map_err(|message| {
        Failure::new(
            infiltrator_contract::error::ErrorCode::InvalidState,
            format!("Mihomo returned invalid {field}: {message}"),
            true,
        )
    })
}
