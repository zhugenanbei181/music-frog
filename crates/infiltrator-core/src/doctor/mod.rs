//! Doctor: self-diagnostics plus a safe, conservative auto-repair facade.
//!
//! `run` executes read-only health checks (ids aligned with mihomo-rs 2.2.0),
//! `fix` applies only unambiguous repairs (create missing directories, create
//! a missing default config, derive and write back external-controller,
//! delete a stale pid file), and [`exit_code`] maps a report to the CLI exit
//! status (0 = no failures, 1 = at least one failing check, 2 is reserved for
//! caller-level errors and never returned here).
//!
//! The module is split along its seams: [`checks`] holds the read-only
//! checks, [`fixes`] the repairs, and [`pidfile`] the pid-record liveness
//! logic. All filesystem inputs come from the injectable [`DoctorEnv`] so
//! tests and embedded hosts never touch global state.

mod checks;
#[cfg(test)]
#[path = "doctor_test.rs"]
mod doctor_test;
mod fixes;
mod pidfile;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mihomo_config::manager::ConfigManager;
use mihomo_platform::paths::get_home_dir;
use mihomo_version::manager::VersionManager;
use serde::{Deserialize, Serialize};

/// Outcome of a single check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

/// Result of one executed check.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheckResult {
    pub id: String,
    pub category: String,
    pub status: DoctorStatus,
    pub summary: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

/// A full doctor run. Timestamps are unix seconds.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub started_at: u64,
    pub finished_at: u64,
    pub checks: Vec<DoctorCheckResult>,
}

impl DoctorReport {
    pub fn has_failures(&self) -> bool {
        self.count_by_status(DoctorStatus::Fail) > 0
    }

    pub fn count_by_status(&self, status: DoctorStatus) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }
}

/// Static self-description of one check, backing `doctor list` and
/// `doctor explain <id>`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DoctorCheckMeta {
    pub id: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
    /// Why the check exists at all.
    pub why: &'static str,
    /// What a failing status concretely means.
    pub fail_means: &'static str,
    /// What the user should do about a failing status.
    pub hint: &'static str,
    pub fixable: bool,
    pub default_enabled: bool,
}

/// One executed repair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFixAction {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFixReport {
    /// Only the actions that actually changed something.
    pub actions: Vec<DoctorFixAction>,
}

pub const CHECKS: &[DoctorCheckMeta] = &[
    DoctorCheckMeta {
        id: "config.settings_parse",
        category: "config",
        summary: "App settings file parses as TOML",
        why: "Broken app-level settings must be flagged before other checks derive paths from them.",
        fail_means: "The settings file exists but is not valid TOML for AppSettings.",
        hint: "Fix the TOML syntax in the settings file or remove the broken fields.",
        fixable: false,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "config.configs_dir",
        category: "config",
        summary: "Configs directory exists",
        why: "Profiles and credentials live under the configs directory; most operations need it.",
        fail_means: "The configs directory cannot even be resolved.",
        hint: "Check the configs directory override and the settings file.",
        fixable: true,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "config.current_profile",
        category: "config",
        summary: "Current profile exists",
        why: "Service lifecycle and apply flows depend on the active profile.",
        fail_means: "The active profile name resolves but its config file is missing.",
        hint: "Switch to an existing profile or create the default config.",
        fixable: false,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "config.current_yaml",
        category: "config",
        summary: "Current profile YAML parses and is basically valid",
        why: "Runtime operations load the current profile YAML on every apply.",
        fail_means: "The current profile YAML is missing, unparsable, or not a mapping.",
        hint: "Create the current profile config or fix its YAML syntax.",
        fixable: true,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "version.binary_available",
        category: "version",
        summary: "Default core binary exists and is executable",
        why: "Service lifecycle commands require a runnable default binary.",
        fail_means: "No default version is set, its binary is missing, or it is not executable.",
        hint: "Install a core version and set it as default.",
        fixable: false,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "service.pid_state",
        category: "service",
        summary: "Service pid record reflects a live state",
        why: "Lifecycle diagnostics depend on the pid file matching reality.",
        fail_means: "The pid file itself could not be read.",
        hint: "If the pid file keeps reappearing while the service is stopped, inspect the app home directory.",
        fixable: false,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "service.stale_pid",
        category: "service",
        summary: "No stale or malformed pid file",
        why: "A stale or malformed pid file makes service status and restart behavior confusing.",
        fail_means: "The pid file is malformed or its pid is dead or was reused by another process.",
        hint: "Run doctor fix --only service.stale_pid to remove the stale pid file.",
        fixable: true,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "controller.external_controller",
        category: "controller",
        summary: "External-controller resolves to a usable URL",
        why: "Proxy, telemetry, and connection features rely on a valid controller endpoint.",
        fail_means: "The current config cannot provide a valid external-controller value.",
        hint: "Set external-controller to host:port, http(s)://host:port, or a unix socket path.",
        fixable: true,
        default_enabled: true,
    },
    DoctorCheckMeta {
        id: "controller.api_reachable",
        category: "controller",
        summary: "Controller API responds when the service is running",
        why: "Connection, proxy, and telemetry features depend on the controller being reachable.",
        fail_means: "The core is running but the configured controller endpoint does not respond.",
        hint: "Check the external-controller value and whether the running instance is healthy.",
        fixable: false,
        default_enabled: true,
    },
];

/// All checks in run order, for `doctor list`.
pub fn list_checks() -> &'static [DoctorCheckMeta] {
    CHECKS
}

/// Self-description of one check, for `doctor explain <id>`.
pub fn explain_check(check_id: &str) -> anyhow::Result<&'static DoctorCheckMeta> {
    CHECKS
        .iter()
        .find(|check| check.id == check_id)
        .ok_or_else(|| anyhow::anyhow!("unknown doctor check '{check_id}'"))
}

/// Filesystem inputs for a doctor run.
///
/// Every path a check needs is derived from one injectable home directory so
/// tests can run against a temp dir instead of global state. Hosts that keep
/// the app settings file elsewhere (e.g. the OS app-data dir) override it via
/// [`DoctorEnv::with_settings_file`]; the configs directory follows that
/// settings file's `configs_dir` redirect, while the versions layout under
/// `home` is resolved through the `mihomo-config` / `mihomo-version`
/// managers, so their directory conventions stay authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorEnv {
    home: PathBuf,
    settings_file: PathBuf,
}

impl DoctorEnv {
    /// Env pointing at the real installation (home from `mihomo-platform`).
    pub fn detect() -> anyhow::Result<Self> {
        let home = get_home_dir()?;
        Ok(Self::with_home(home))
    }

    /// Env rooted at `home`; the app settings file defaults to
    /// `<home>/settings.toml`.
    pub fn with_home(home: PathBuf) -> Self {
        let settings_file = home.join("settings.toml");
        Self {
            home,
            settings_file,
        }
    }

    /// Override the app settings file location (builder style).
    pub fn with_settings_file(mut self, settings_file: PathBuf) -> Self {
        self.settings_file = settings_file;
        self
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn settings_file(&self) -> &Path {
        &self.settings_file
    }

    /// Location of the core pid file (`<home>/mihomo.pid`, matching
    /// `ProcessCoreController`).
    pub fn pid_file(&self) -> PathBuf {
        self.home.join("mihomo.pid")
    }

    pub(super) async fn config_manager(
        &self,
    ) -> mihomo_api::error::Result<ConfigManager<mihomo_platform::defaults::DefaultCredentialStore>>
    {
        // configs 目录解析跟随本 env 的 settings 文件（`configs_dir` 字段）。
        // settings 解析失败时按默认值继续：坏 settings 由 config.settings_parse
        // 专门上报，其余检查不得连带失效。
        let settings = crate::settings::load_settings(&self.settings_file)
            .await
            .unwrap_or_default();
        ConfigManager::with_home_configs_dir_and_store(
            self.home.clone(),
            settings.configs_dir.as_deref(),
            mihomo_platform::defaults::DefaultCredentialStore::default(),
        )
    }

    pub(super) fn version_manager(&self) -> mihomo_api::error::Result<VersionManager> {
        VersionManager::with_home(self.home.clone())
    }

    /// Resolved current-profile YAML path.
    pub(super) async fn current_profile_path(&self) -> anyhow::Result<PathBuf> {
        let manager = self.config_manager().await?;
        Ok(manager.get_current_path().await?)
    }

    /// Resolved configs directory. Derivation goes through
    /// [`ConfigManager::get_current_path`] so it stays consistent with
    /// whatever directory resolution `mihomo-config` performs, including the
    /// cloud-sync redirect from the settings file (`configs_dir` field).
    pub(super) async fn configs_dir(&self) -> anyhow::Result<PathBuf> {
        let path = self.current_profile_path().await?;
        path.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow::anyhow!("profile path has no parent: {}", path.display()))
    }
}

/// Run the default checks against the real installation.
pub async fn run(filter: Option<&str>) -> DoctorReport {
    match DoctorEnv::detect() {
        Ok(env) => run_with(&env, filter).await,
        Err(err) => home_unavailable_report(filter, &err.to_string()),
    }
}

/// Run the default checks against an explicit environment.
pub async fn run_with(env: &DoctorEnv, filter: Option<&str>) -> DoctorReport {
    let started_at = unix_now();
    let filter = CheckFilter::parse(filter);
    let mut checks = Vec::new();
    for meta in CHECKS
        .iter()
        .filter(|meta| filter.matches(meta.id, meta.category))
    {
        checks.push(run_check(env, meta.id).await);
    }
    DoctorReport {
        started_at,
        finished_at: unix_now(),
        checks,
    }
}

async fn run_check(env: &DoctorEnv, id: &str) -> DoctorCheckResult {
    match id {
        "config.settings_parse" => checks::check_settings_parse(env).await,
        "config.configs_dir" => checks::check_configs_dir(env).await,
        "config.current_profile" => checks::check_current_profile(env).await,
        "config.current_yaml" => checks::check_current_yaml(env).await,
        "version.binary_available" => checks::check_binary_available(env).await,
        "service.pid_state" => checks::check_pid_state(env).await,
        "service.stale_pid" => checks::check_stale_pid(env).await,
        "controller.external_controller" => checks::check_external_controller(env).await,
        "controller.api_reachable" => checks::check_api_reachable(env).await,
        _ => unreachable!("CHECKS and run_check disagree about check ids"),
    }
}

/// Nothing can be checked without a home directory; keep the report shape and
/// the exit contract intact by failing every selected check.
fn home_unavailable_report(filter: Option<&str>, reason: &str) -> DoctorReport {
    let filter = CheckFilter::parse(filter);
    let checks = CHECKS
        .iter()
        .filter(|meta| filter.matches(meta.id, meta.category))
        .map(|meta| DoctorCheckResult {
            id: meta.id.to_string(),
            category: meta.category.to_string(),
            status: DoctorStatus::Fail,
            summary: format!("Check unavailable: home directory could not be resolved: {reason}"),
            detail: None,
            hint: Some("Set MIHOMO_HOME or fix the home directory resolution.".to_string()),
        })
        .collect();
    DoctorReport {
        started_at: unix_now(),
        finished_at: unix_now(),
        checks,
    }
}

/// Apply the conservative fixes to the real installation.
pub async fn fix(filter: Option<&str>) -> anyhow::Result<DoctorFixReport> {
    fix_with(&DoctorEnv::detect()?, filter).await
}

/// Apply the conservative fixes against an explicit environment.
pub async fn fix_with(env: &DoctorEnv, filter: Option<&str>) -> anyhow::Result<DoctorFixReport> {
    let filter = CheckFilter::parse(filter);
    let mut actions = Vec::new();
    if filter.matches("config.configs_dir", "config")
        && let Some(action) = fixes::fix_configs_dir(env).await?
    {
        actions.push(action);
    }
    if filter.matches("config.current_yaml", "config")
        && let Some(action) = fixes::fix_current_yaml(env).await?
    {
        actions.push(action);
    }
    if filter.matches("controller.external_controller", "controller")
        && let Some(action) = fixes::fix_external_controller(env).await?
    {
        actions.push(action);
    }
    if filter.matches("service.stale_pid", "service")
        && let Some(action) = fixes::fix_stale_pid(env).await?
    {
        actions.push(action);
    }
    Ok(DoctorFixReport { actions })
}

/// CLI exit status for a doctor report: 0 when no check failed, 1 when at
/// least one did. 2 is reserved for caller-level errors and is never
/// produced here.
pub fn exit_code(report: &DoctorReport) -> i32 {
    if report.has_failures() { 1 } else { 0 }
}

/// Filter tokens come from `--only`; a check runs when any token equals its
/// category or is a prefix of its id (`service` selects both service checks,
/// `service.stale` selects `service.stale_pid`).
#[derive(Debug, Clone, Default)]
struct CheckFilter {
    tokens: Vec<String>,
}

impl CheckFilter {
    fn parse(raw: Option<&str>) -> Self {
        let tokens = raw
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect();
        Self { tokens }
    }

    fn matches(&self, id: &str, category: &str) -> bool {
        self.tokens.is_empty()
            || self
                .tokens
                .iter()
                .any(|token| token == category || id.starts_with(token.as_str()))
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
