//! Tokio/filesystem adapter for per-profile option sidecars.
//!
//! The option schema and composition algorithm live in `infiltrator-domain`.
//! This module owns only path I/O and the settings/config-directory lookup
//! needed by existing subscription update entry points.

use anyhow::Context;
use infiltrator_domain::filter::FilterReport;
use infiltrator_domain::profile_options::{ProfileOptions, compose_content, options_path};
use std::path::Path;

/// Load the sidecar. A missing file yields the default (empty) options; a
/// malformed one is an error so a broken hand-edit cannot silently drop the
/// user's filter/mixin on the next subscription update.
pub async fn load_options(config_dir: &Path, profile: &str) -> anyhow::Result<ProfileOptions> {
    let path = options_path(config_dir, profile);
    let Ok(text) = tokio::fs::read_to_string(&path).await else {
        return Ok(ProfileOptions::default());
    };
    serde_yaml_ng::from_str(&text)
        .with_context(|| format!("解析配置选项文件失败: {}", path.display()))
}

/// Persist the sidecar atomically. Saving empty options removes the file so
/// stale sidecars cannot resurrect onto a future profile of the same name.
pub async fn save_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> anyhow::Result<()> {
    let path = options_path(config_dir, profile);
    if options.is_empty() {
        let _ = tokio::fs::remove_file(&path).await;
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text = serde_yaml_ng::to_string(options)?;
    let temp = path.with_file_name(format!(
        ".{}.options-tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ));
    tokio::fs::write(&temp, text).await?;
    tokio::fs::rename(&temp, &path).await?;
    Ok(())
}

/// Best-effort sidecar removal when a profile is deleted; a leftover file
/// would otherwise be picked up by a profile recreated with the same name.
pub async fn delete_options(config_dir: &Path, profile: &str) {
    let _ = tokio::fs::remove_file(options_path(config_dir, profile)).await;
}

/// Resolve the configured sidecar directory and compose options onto freshly
/// fetched subscription content.
pub async fn apply_saved_options_for(
    profile: &str,
    content: &str,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    let home = mihomo_platform::paths::get_home_dir()?;
    let settings_file = crate::settings_io::settings_path(&home)?;
    let settings = crate::settings_io::load_settings(&settings_file).await?;
    let config_dir = mihomo_config::manager::paths::resolve_configs_dir_in(
        settings.configs_dir.as_deref(),
        &home,
    )?;
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
