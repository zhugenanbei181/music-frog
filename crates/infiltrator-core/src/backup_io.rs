//! Filesystem adapter for the runtime-neutral backup bundle.

use infiltrator_domain::backup::{BackupBundle, BackupError, ProfileBackupItem, Result};
use std::path::Path;

use crate::settings_io::settings_path;

/// Export all active configs and settings into a single JSON backup bundle string.
pub async fn export_all_configs_bundle(base_dir: &Path) -> anyhow::Result<String> {
    let bundle = export_bundle_from_dir(base_dir).await?;
    Ok(serde_json::to_string_pretty(&bundle)?)
}

/// Reads local configurations, profiles, and options from `base_dir` into a `BackupBundle`.
pub async fn export_bundle_from_dir(base_dir: &Path) -> Result<BackupBundle> {
    let settings_p =
        settings_path(base_dir).map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
    let settings_toml = if tokio::fs::try_exists(&settings_p).await.unwrap_or(false) {
        tokio::fs::read_to_string(&settings_p).await?
    } else {
        String::new()
    };

    let mixin_p = base_dir.join("mixin.yaml");
    let mixin_yaml = if tokio::fs::try_exists(&mixin_p).await.unwrap_or(false) {
        tokio::fs::read_to_string(&mixin_p).await?
    } else {
        String::new()
    };

    let configs_dir = base_dir.join("configs");
    let options_dir = base_dir.join("options");
    let mut profiles = Vec::new();

    if tokio::fs::try_exists(&configs_dir).await.unwrap_or(false) {
        let mut entries = tokio::fs::read_dir(&configs_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let is_yaml =
                path.is_file() && path.extension().is_some_and(|e| e == "yaml" || e == "yml");
            if is_yaml && let Ok(content) = tokio::fs::read_to_string(&path).await {
                let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                let opt_path = options_dir.join(format!("{stem}.yaml"));
                let options_yaml = if tokio::fs::try_exists(&opt_path).await.unwrap_or(false) {
                    tokio::fs::read_to_string(&opt_path).await.ok()
                } else {
                    None
                };

                profiles.push(ProfileBackupItem {
                    name: stem,
                    content,
                    options_yaml,
                });
            }
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(BackupBundle::new(profiles, settings_toml, mixin_yaml))
}

/// Restores a `BackupBundle` into `base_dir`.
pub async fn restore_bundle_to_dir(
    bundle: &BackupBundle,
    base_dir: &Path,
    overwrite: bool,
) -> Result<()> {
    bundle.validate_checksum()?;

    let settings_p =
        settings_path(base_dir).map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
    if let Some(parent) = settings_p.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if overwrite || !tokio::fs::try_exists(&settings_p).await.unwrap_or(false) {
        tokio::fs::write(&settings_p, &bundle.settings_toml).await?;
    }

    let mixin_p = base_dir.join("mixin.yaml");
    if !bundle.mixin_yaml.is_empty()
        && (overwrite || !tokio::fs::try_exists(&mixin_p).await.unwrap_or(false))
    {
        tokio::fs::write(&mixin_p, &bundle.mixin_yaml).await?;
    }

    let configs_dir = base_dir.join("configs");
    let options_dir = base_dir.join("options");
    tokio::fs::create_dir_all(&configs_dir).await?;

    for profile in &bundle.profiles {
        let p_path = configs_dir.join(format!("{}.yaml", profile.name));
        if overwrite || !tokio::fs::try_exists(&p_path).await.unwrap_or(false) {
            tokio::fs::write(&p_path, &profile.content).await?;
        }

        if let Some(opts) = &profile.options_yaml {
            tokio::fs::create_dir_all(&options_dir).await?;
            let opt_path = options_dir.join(format!("{}.yaml", profile.name));
            if overwrite || !tokio::fs::try_exists(&opt_path).await.unwrap_or(false) {
                tokio::fs::write(&opt_path, opts).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "backup_test.rs"]
mod tests;
