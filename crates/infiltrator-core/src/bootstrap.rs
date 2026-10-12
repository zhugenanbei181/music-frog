//! One-shot startup bootstrap: make sure the configs directory, the current
//! profile's config file, and a usable external-controller all exist before
//! anything tries to use them.
//!
//! Thin orchestration only — every step delegates to the existing
//! `mihomo-config` manager capabilities and just records whether the step
//! had to change anything or was already satisfied.

use crate::settings_io::{app_config_manager_in, load_settings, settings_path};
use infiltrator_platform::paths::get_home_dir;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use mihomo_config::manager::paths::resolve_configs_dir_in;
use serde::Serialize;
use std::path::Path;
use tokio::fs::{create_dir_all, read_to_string};

#[cfg(test)]
#[path = "bootstrap_test.rs"]
mod bootstrap_test;

/// One bootstrap step and whether it had to change anything.
#[derive(Debug, Clone, Serialize)]
pub struct BootstrapStep {
    pub id: &'static str,
    /// `true` when this call performed the step, `false` when it was
    /// already satisfied (skipped).
    pub executed: bool,
    pub detail: String,
}

/// Summary of a bootstrap run, in execution order.
#[derive(Debug, Clone, Serialize)]
pub struct BootstrapReport {
    pub steps: Vec<BootstrapStep>,
}

impl BootstrapReport {
    pub fn executed_steps(&self) -> impl Iterator<Item = &BootstrapStep> {
        self.steps.iter().filter(|step| step.executed)
    }

    pub fn skipped_steps(&self) -> impl Iterator<Item = &BootstrapStep> {
        self.steps.iter().filter(|step| !step.executed)
    }

    pub fn any_executed(&self) -> bool {
        self.executed_steps().next().is_some()
    }
}

/// Bootstrap the real installation (home from `infiltrator-platform`).
pub async fn ensure_bootstrap() -> anyhow::Result<BootstrapReport> {
    let home = get_home_dir()?;
    ensure_bootstrap_at(&home).await
}

/// Bootstrap the installation rooted at `home`. Idempotent: a second run on
/// an already-initialized home reports every step as skipped.
pub async fn ensure_bootstrap_at(home: &Path) -> anyhow::Result<BootstrapReport> {
    // Capture the root state before manager initialization. Namespace setup
    // creates the shared root as part of acquiring its reset fence, so
    // measuring it afterward would report a freshly-created directory as
    // already present and hide the bootstrap step from callers.
    let settings = load_settings(&settings_path(home)?).await?;
    let configs_dir = resolve_configs_dir_in(settings.configs_dir.as_deref(), home)?;
    let had_dir = configs_dir.is_dir();

    // 经由核心规范工厂：settings 的 `configs_dir` 指向云同步目录时，
    // 默认配置必须建到解析后的目录。
    let manager = app_config_manager_in(home).await?;
    let mut steps = Vec::new();

    // Step 1: the shared configs root must exist before profile migration.
    create_dir_all(&configs_dir).await?;
    steps.push(BootstrapStep {
        id: "configs_dir",
        executed: !had_dir,
        detail: format!("configs directory '{}'", configs_dir.display()),
    });

    // Step 2: the explicit V0 migration selects the historical Mihomo profile
    // and creates the default document only for a new install. A cleared or
    // invalid global reference never falls back to the namespace pointer.
    let profile_store = manager.engine_profile_store();
    let previous_selection = ActiveProfileReferenceStore::read(profile_store).await?;
    let selection = manager.migrate_legacy_mihomo_selection().await?;
    let profile_path = manager.get_current_path().await?;
    steps.push(BootstrapStep {
        id: "default_config",
        executed: previous_selection != selection,
        detail: format!("active Mihomo profile config '{}'", profile_path.display()),
    });

    // Step 3: derive/repair external-controller. `ensure_external_controller`
    // only writes when the config needs it; comparing the profile file before
    // and after tells whether the step actually had to change anything (the
    // resolved URL alone can stay identical, e.g. when the default endpoint
    // is written out verbatim).
    let profile_path = manager.get_current_path().await?;
    let before = read_to_string(&profile_path).await.ok();
    let controller_url = manager.ensure_external_controller().await?;
    let after = read_to_string(&profile_path).await.ok();
    steps.push(BootstrapStep {
        id: "external_controller",
        executed: before != after,
        detail: format!("external-controller resolves to '{controller_url}'"),
    });

    Ok(BootstrapReport { steps })
}
