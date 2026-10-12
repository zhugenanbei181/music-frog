//! The four conservative doctor fixes. Each fix acts only when the problem is
//! unambiguous (missing artifact or provably stale record) and never
//! overwrites existing content.

use super::pidfile::remove_stale_pid_file;
use super::{DoctorEnv, DoctorFixAction};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use std::io::ErrorKind;
use tokio::fs;

pub(super) async fn fix_configs_dir(env: &DoctorEnv) -> anyhow::Result<Option<DoctorFixAction>> {
    let dir = env.configs_dir().await?;
    match fs::metadata(&dir).await {
        Ok(meta) if meta.is_dir() => return Ok(None),
        Ok(_) => {
            anyhow::bail!(
                "refusing to fix configs directory '{}': it exists but is not a directory",
                dir.display()
            );
        }
        Err(err) if err.kind() != ErrorKind::NotFound => {
            return Err(err.into());
        }
        Err(_) => {}
    }
    fs::create_dir_all(&dir).await?;
    Ok(Some(DoctorFixAction {
        id: "config.configs_dir".to_string(),
        summary: format!("Created configs directory '{}'", dir.display()),
    }))
}

pub(super) async fn fix_current_yaml(env: &DoctorEnv) -> anyhow::Result<Option<DoctorFixAction>> {
    let manager = env.initialized_config_manager().await?;
    let previous_selection = ActiveProfileReferenceStore::read(&manager).await?;
    let selection = manager.migrate_legacy_mihomo_selection().await?;
    let path = manager.get_current_path().await?;
    if path.exists() {
        // An existing file is never rewritten, even when it fails to parse:
        // the content belongs to the user.
        return Ok((previous_selection != selection).then(|| DoctorFixAction {
            id: "config.current_yaml".to_string(),
            summary: format!("Initialized active profile config '{}'", path.display()),
        }));
    }
    anyhow::bail!(
        "active profile document '{}' is missing; refusing to create a different namespace-default profile",
        path.display()
    )
}

pub(super) async fn fix_external_controller(
    env: &DoctorEnv,
) -> anyhow::Result<Option<DoctorFixAction>> {
    let manager = env.initialized_config_manager().await?;
    manager.migrate_legacy_mihomo_selection().await?;
    let profile_path = manager.get_current_path().await?;
    // A write happened iff the profile content changed; the resolved URL can
    // stay identical even when the key is newly written out.
    let before = fs::read_to_string(&profile_path).await.ok();
    let url = manager.ensure_external_controller().await?;
    let wrote = fs::read_to_string(&profile_path).await.ok().as_deref() != before.as_deref();
    if !wrote {
        return Ok(None);
    }
    Ok(Some(DoctorFixAction {
        id: "controller.external_controller".to_string(),
        summary: format!("External-controller now resolves to '{url}'"),
    }))
}

pub(super) async fn fix_stale_pid(env: &DoctorEnv) -> anyhow::Result<Option<DoctorFixAction>> {
    let pid_file = env.pid_file();
    if remove_stale_pid_file(&pid_file).await? {
        Ok(Some(DoctorFixAction {
            id: "service.stale_pid".to_string(),
            summary: format!("Removed stale pid file '{}'", pid_file.display()),
        }))
    } else {
        Ok(None)
    }
}
