//! The four conservative doctor fixes. Each fix acts only when the problem is
//! unambiguous (missing artifact or provably stale record) and never
//! overwrites existing content.

use tokio::fs;

use super::pidfile::remove_stale_pid_file;
use super::{DoctorEnv, DoctorFixAction};

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
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
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
    let manager = env.config_manager().await?;
    let path = manager.get_current_path().await?;
    if path.exists() {
        // An existing file is never rewritten, even when it fails to parse:
        // the content belongs to the user.
        return Ok(None);
    }
    manager.ensure_default_config().await?;
    Ok(Some(DoctorFixAction {
        id: "config.current_yaml".to_string(),
        summary: format!("Created default config '{}'", path.display()),
    }))
}

pub(super) async fn fix_external_controller(
    env: &DoctorEnv,
) -> anyhow::Result<Option<DoctorFixAction>> {
    let manager = env.config_manager().await?;
    let profile_path = manager.get_current_path().await?;
    // A write happened iff the profile content changed; the resolved URL can
    // stay identical even when the key is newly written out.
    let before = tokio::fs::read_to_string(&profile_path).await.ok();
    let url = manager.ensure_external_controller().await?;
    let wrote = tokio::fs::read_to_string(&profile_path)
        .await
        .ok()
        .as_deref()
        != before.as_deref();
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
