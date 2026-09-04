//! One-shot startup bootstrap: make sure the configs directory, the current
//! profile's config file, and a usable external-controller all exist before
//! anything tries to use them.
//!
//! Thin orchestration only — every step delegates to the existing
//! `mihomo-config` manager capabilities and just records whether the step
//! had to change anything or was already satisfied.

use std::path::Path;

use infiltrator_ports::secure_store::SecureStore;
use serde::Serialize;

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

/// Bootstrap the real installation (home from `mihomo-platform`).
pub async fn ensure_bootstrap() -> anyhow::Result<BootstrapReport> {
    let home = mihomo_platform::paths::get_home_dir()?;
    ensure_bootstrap_at(&home).await
}

/// Bootstrap the installation rooted at `home`. Idempotent: a second run on
/// an already-initialized home reports every step as skipped.
pub async fn ensure_bootstrap_at(home: &Path) -> anyhow::Result<BootstrapReport> {
    // 经由核心规范工厂：settings 的 `configs_dir` 指向云同步目录时，
    // 默认配置必须建到解析后的目录。
    let manager = crate::settings_io::app_config_manager_in(home).await?;
    let mut steps = Vec::new();

    // Step 1: the configs directory must exist before any profile file can.
    let configs_dir = configs_dir_of(&manager).await?;
    let had_dir = configs_dir.is_dir();
    tokio::fs::create_dir_all(&configs_dir).await?;
    steps.push(BootstrapStep {
        id: "configs_dir",
        executed: !had_dir,
        detail: format!("configs directory '{}'", configs_dir.display()),
    });

    // Step 2: the current profile needs a config file; mihomo-config writes a
    // known-good default (including external-controller) when one is missing.
    let profile_path = manager.get_current_path().await?;
    let had_config = profile_path.exists();
    manager.ensure_default_config().await?;
    steps.push(BootstrapStep {
        id: "default_config",
        executed: !had_config,
        detail: format!("current profile config '{}'", profile_path.display()),
    });

    // Step 3: derive/repair external-controller. `ensure_external_controller`
    // only writes when the config needs it; comparing the profile file before
    // and after tells whether the step actually had to change anything (the
    // resolved URL alone can stay identical, e.g. when the default endpoint
    // is written out verbatim).
    let profile_path = manager.get_current_path().await?;
    let before = tokio::fs::read_to_string(&profile_path).await.ok();
    let controller_url = manager.ensure_external_controller().await?;
    let after = tokio::fs::read_to_string(&profile_path).await.ok();
    steps.push(BootstrapStep {
        id: "external_controller",
        executed: before != after,
        detail: format!("external-controller resolves to '{controller_url}'"),
    });

    Ok(BootstrapReport { steps })
}

async fn configs_dir_of<S: SecureStore>(
    manager: &mihomo_config::manager::ConfigManager<S>,
) -> anyhow::Result<std::path::PathBuf> {
    let profile_path = manager.get_current_path().await?;
    profile_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("profile path has no parent: {}", profile_path.display()))
}
