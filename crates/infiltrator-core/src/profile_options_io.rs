//! Tokio/filesystem adapter for per-profile option sidecars.
//!
//! The option schema and composition algorithm live in `infiltrator-domain`.
//! This module owns only path I/O and the settings/config-directory lookup
//! needed by existing subscription update entry points.

use crate::settings_io::{load_settings, settings_path};
use infiltrator_contract::subscription_filter_result::FilterReport;
use infiltrator_domain::profile_options::compose_content;
use mihomo_config::manager::paths::resolve_configs_dir_in;
use mihomo_config::profile_option_store::load_options;
use mihomo_platform::paths::get_home_dir;
use std::path::Path;

/// Resolve the configured sidecar directory and compose options onto freshly
/// fetched subscription content.
pub async fn apply_saved_options_for(
    profile: &str,
    content: &str,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    let home = get_home_dir()?;
    let settings_file = settings_path(&home)?;
    let settings = load_settings(&settings_file).await?;
    let config_dir = resolve_configs_dir_in(settings.configs_dir.as_deref(), &home)?;
    apply_saved_options(&config_dir, profile, content).await
}

/// Load the sidecar for `profile` and compose it onto freshly fetched
/// subscription content. Composition itself is domain code.
pub async fn apply_saved_options(
    config_dir: &Path,
    profile: &str,
    content: &str,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    let options = load_options(config_dir, profile).await?;
    compose_content(content, &options)
}

#[cfg(test)]
#[path = "profile_options_test.rs"]
mod profile_options_test;
