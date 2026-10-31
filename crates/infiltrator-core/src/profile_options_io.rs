//! Core entry points for the Mihomo-owned per-profile options adapter.

use crate::error::CoreError;
use crate::profile_store_io::config_dir;
use infiltrator_contract::subscription_filter_result::FilterReport;
use mihomo_config::profile_options_codec;
use std::path::Path;

/// Resolve the configured sidecar directory and compose options onto freshly
/// fetched subscription content.
pub async fn apply_saved_options_for(
    profile: &str,
    content: &str,
) -> Result<(String, Option<FilterReport>), CoreError> {
    let config_dir = config_dir().await?;
    apply_saved_options(&config_dir, profile, content).await
}

/// Load the sidecar for `profile` and compose it onto freshly fetched
/// subscription content through the Mihomo config adapter.
pub async fn apply_saved_options(
    config_dir: &Path,
    profile: &str,
    content: &str,
) -> Result<(String, Option<FilterReport>), CoreError> {
    profile_options_codec::apply_saved(config_dir, profile, content)
        .await
        .map_err(Into::into)
}

#[cfg(test)]
#[path = "profile_options_test.rs"]
mod profile_options_test;
