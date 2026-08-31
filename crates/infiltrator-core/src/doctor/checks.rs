//! The nine doctor checks. Each function turns filesystem / process / API
//! state into one [`DoctorCheckResult`] and never mutates anything.

use std::path::Path;

use mihomo_api::client::MihomoClient;
use mihomo_api::error::MihomoError;
use tokio::fs;
use yaml_rust2::YamlLoader;

use super::pidfile::{PidFileState, ProcessState, read_pid_state, service_running};
use super::{DoctorCheckResult, DoctorEnv, DoctorStatus};
use crate::settings::AppSettings;

const FIX_CONFIGS_DIR: &str = "Run doctor fix --only config.configs_dir to create it.";
const FIX_CURRENT_YAML: &str =
    "Run doctor fix --only config.current_yaml to create the default config.";
const FIX_STALE_PID: &str = "Run doctor fix --only service.stale_pid to remove the stale pid file.";

pub(super) async fn check_settings_parse(env: &DoctorEnv) -> DoctorCheckResult {
    let path = env.settings_file();
    if !path.exists() {
        return pass(
            "config.settings_parse",
            "config",
            "Settings file not found; defaults apply",
            Some(path.display().to_string()),
            None,
        );
    }
    let content = match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) => {
            return fail(
                "config.settings_parse",
                "config",
                format!("Failed to read settings file '{}': {err}", path.display()),
                None,
                None,
            );
        }
    };
    match toml::from_str::<AppSettings>(&content) {
        Ok(_) => pass(
            "config.settings_parse",
            "config",
            "Settings file parses as AppSettings TOML",
            Some(path.display().to_string()),
            None,
        ),
        Err(err) => fail(
            "config.settings_parse",
            "config",
            format!("Invalid settings TOML in '{}': {err}", path.display()),
            None,
            Some("Fix the TOML syntax in the settings file or remove the broken fields.".into()),
        ),
    }
}

pub(super) async fn check_configs_dir(env: &DoctorEnv) -> DoctorCheckResult {
    let dir = match env.configs_dir().await {
        Ok(dir) => dir,
        Err(reason) => {
            return fail(
                "config.configs_dir",
                "config",
                format!("Cannot resolve configs directory: {reason}"),
                None,
                Some("Check the settings file and the configs directory override.".into()),
            );
        }
    };
    match fs::metadata(&dir).await {
        Ok(meta) if meta.is_dir() => pass(
            "config.configs_dir",
            "config",
            "Configs directory exists",
            Some(dir.display().to_string()),
            None,
        ),
        Ok(_) => warn(
            "config.configs_dir",
            "config",
            format!(
                "Resolved configs directory '{}' exists but is not a directory",
                dir.display()
            ),
            None,
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => warn(
            "config.configs_dir",
            "config",
            format!("Configs directory '{}' does not exist yet", dir.display()),
            Some(FIX_CONFIGS_DIR.into()),
        ),
        Err(err) => warn(
            "config.configs_dir",
            "config",
            format!(
                "Configs directory '{}' is not accessible: {err}",
                dir.display()
            ),
            None,
        ),
    }
}

pub(super) async fn check_current_profile(env: &DoctorEnv) -> DoctorCheckResult {
    let manager = match env.config_manager().await {
        Ok(manager) => manager,
        Err(err) => return manager_fail("config.current_profile", "config", err),
    };
    let profile = match manager.get_current().await {
        Ok(profile) => profile,
        Err(err) => {
            return fail(
                "config.current_profile",
                "config",
                format!("Cannot determine current profile: {err}"),
                None,
                None,
            );
        }
    };
    let path = match manager.get_current_path().await {
        Ok(path) => path,
        Err(err) => {
            return fail(
                "config.current_profile",
                "config",
                format!("Current profile '{profile}' is unusable: {err}"),
                None,
                None,
            );
        }
    };
    if path.is_file() {
        pass(
            "config.current_profile",
            "config",
            format!("Current profile '{profile}' exists"),
            Some(path.display().to_string()),
            None,
        )
    } else {
        fail(
            "config.current_profile",
            "config",
            format!(
                "Current profile '{profile}' has no config file at '{}'",
                path.display()
            ),
            None,
            Some(FIX_CURRENT_YAML.into()),
        )
    }
}

pub(super) async fn check_current_yaml(env: &DoctorEnv) -> DoctorCheckResult {
    let path = match env.current_profile_path().await {
        Ok(path) => path,
        Err(err) => {
            return skip(
                "config.current_yaml",
                "config",
                format!("Skipped because the current profile path is unavailable: {err}"),
            );
        }
    };
    if !path.exists() {
        return fail(
            "config.current_yaml",
            "config",
            format!("Current config '{}' does not exist", path.display()),
            None,
            Some(FIX_CURRENT_YAML.into()),
        );
    }
    let content = match fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(err) => {
            return fail(
                "config.current_yaml",
                "config",
                format!("Failed to read '{}': {err}", path.display()),
                None,
                None,
            );
        }
    };
    let docs = match YamlLoader::load_from_str(&content) {
        Ok(docs) => docs,
        Err(err) => {
            return fail(
                "config.current_yaml",
                "config",
                format!("Invalid YAML in '{}': {err}", path.display()),
                None,
                Some("Fix the YAML syntax in the current profile file.".into()),
            );
        }
    };
    // 基本合法的最低要求：顶层是映射，mihomo 才能把它当配置加载。
    if !docs.first().is_some_and(|doc| doc.as_hash().is_some()) {
        return fail(
            "config.current_yaml",
            "config",
            format!(
                "'{}' parses as YAML but its top level is not a mapping",
                path.display()
            ),
            None,
            Some("The current profile must be a YAML mapping of mihomo settings.".into()),
        );
    }
    pass(
        "config.current_yaml",
        "config",
        "Current config YAML parses and is a mapping",
        Some(path.display().to_string()),
        None,
    )
}

pub(super) async fn check_binary_available(env: &DoctorEnv) -> DoctorCheckResult {
    let manager = match env.version_manager() {
        Ok(manager) => manager,
        Err(err) => return manager_fail("version.binary_available", "version", err),
    };
    let default = match manager.get_default().await {
        Ok(default) => default,
        Err(err) => {
            return fail(
                "version.binary_available",
                "version",
                format!("No usable default version: {err}"),
                None,
                Some("Install a core version and set it as default.".into()),
            );
        }
    };
    let path = match manager.get_binary_path(None).await {
        Ok(path) => path,
        Err(err) => {
            return fail(
                "version.binary_available",
                "version",
                format!("Binary for default version '{default}' unavailable: {err}"),
                None,
                Some("Reinstall the default version to restore its binary.".into()),
            );
        }
    };
    if !is_executable(&path) {
        return fail(
            "version.binary_available",
            "version",
            format!("Binary '{}' is not executable", path.display()),
            None,
            Some("Restore the executable bit or reinstall the version.".into()),
        );
    }
    pass(
        "version.binary_available",
        "version",
        format!("Default core binary for version '{default}' is available"),
        Some(path.display().to_string()),
        None,
    )
}

pub(super) async fn check_pid_state(env: &DoctorEnv) -> DoctorCheckResult {
    let pid_file = env.pid_file();
    match read_pid_state(&pid_file).await {
        PidFileState::Absent => pass(
            "service.pid_state",
            "service",
            "Service is stopped and the pid state is clean",
            None,
            None,
        ),
        PidFileState::Unreadable(err) => fail(
            "service.pid_state",
            "service",
            format!("Pid file '{}' cannot be read: {err}", pid_file.display()),
            None,
            None,
        ),
        PidFileState::Malformed(raw) => warn(
            "service.pid_state",
            "service",
            format!(
                "Pid file '{}' is malformed (content {:?}); see service.stale_pid",
                pid_file.display(),
                raw
            ),
            None,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::AliveCore,
        } => pass(
            "service.pid_state",
            "service",
            format!("Service is running (pid {pid})"),
            None,
            None,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::AliveForeign,
        } => warn(
            "service.pid_state",
            "service",
            format!("Pid file records pid {pid}, which is alive but not a mihomo process"),
            None,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::Gone,
        } => warn(
            "service.pid_state",
            "service",
            format!("Pid file records pid {pid}, which is not running"),
            None,
        ),
    }
}

pub(super) async fn check_stale_pid(env: &DoctorEnv) -> DoctorCheckResult {
    let pid_file = env.pid_file();
    let fix_hint = Some(FIX_STALE_PID.to_string());
    match read_pid_state(&pid_file).await {
        PidFileState::Absent => pass(
            "service.stale_pid",
            "service",
            "No pid file is present",
            None,
            None,
        ),
        PidFileState::Unreadable(err) => fail(
            "service.stale_pid",
            "service",
            format!("Pid file '{}' cannot be read: {err}", pid_file.display()),
            None,
            None,
        ),
        PidFileState::Malformed(raw) => fail(
            "service.stale_pid",
            "service",
            format!(
                "Pid file '{}' is malformed (content {:?})",
                pid_file.display(),
                raw
            ),
            None,
            fix_hint,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::AliveCore,
        } => pass(
            "service.stale_pid",
            "service",
            format!("Pid file tracks the running core process (pid {pid})"),
            None,
            None,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::AliveForeign,
        } => fail(
            "service.stale_pid",
            "service",
            format!(
                "Pid file records pid {pid}, which was reused by another process (stale record)"
            ),
            None,
            fix_hint,
        ),
        PidFileState::Recorded {
            pid,
            process: ProcessState::Gone,
        } => fail(
            "service.stale_pid",
            "service",
            format!("Pid file records dead pid {pid}"),
            None,
            fix_hint,
        ),
    }
}

pub(super) async fn check_external_controller(env: &DoctorEnv) -> DoctorCheckResult {
    let manager = match env.config_manager().await {
        Ok(manager) => manager,
        Err(err) => return manager_fail("controller.external_controller", "controller", err),
    };
    match manager.get_external_controller().await {
        Ok(url) => pass(
            "controller.external_controller",
            "controller",
            "External-controller resolves to a usable URL",
            Some(url),
            None,
        ),
        Err(MihomoError::NotFound(reason)) => skip(
            "controller.external_controller",
            "controller",
            format!("Skipped because the current profile config is missing: {reason}"),
        ),
        Err(err) => fail(
            "controller.external_controller",
            "controller",
            format!("Invalid external-controller: {err}"),
            None,
            Some(
                "Set external-controller to host:port, http(s)://host:port, or a unix socket path."
                    .into(),
            ),
        ),
    }
}

pub(super) async fn check_api_reachable(env: &DoctorEnv) -> DoctorCheckResult {
    if !service_running(&env.pid_file()).await {
        return skip(
            "controller.api_reachable",
            "controller",
            "Skipped because the core service is not running",
        );
    }
    let manager = match env.config_manager().await {
        Ok(manager) => manager,
        Err(err) => return manager_fail("controller.api_reachable", "controller", err),
    };
    let url = match manager.get_external_controller().await {
        Ok(url) => url,
        Err(err) => {
            return fail(
                "controller.api_reachable",
                "controller",
                format!("Cannot resolve external-controller: {err}"),
                None,
                None,
            );
        }
    };
    let client = match MihomoClient::new(&url, None) {
        Ok(client) => client,
        Err(err) => {
            return fail(
                "controller.api_reachable",
                "controller",
                format!("Cannot create controller client for '{url}': {err}"),
                None,
                None,
            );
        }
    };
    match client.get_version().await {
        Ok(version) => pass(
            "controller.api_reachable",
            "controller",
            format!("Controller '{url}' responded (mihomo {})", version.version),
            None,
            None,
        ),
        Err(err) => fail(
            "controller.api_reachable",
            "controller",
            format!("Controller '{url}' is unreachable: {err}"),
            None,
            Some(
                "Check whether the external-controller value matches the running instance.".into(),
            ),
        ),
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn manager_fail(id: &str, category: &str, err: MihomoError) -> DoctorCheckResult {
    fail(
        id,
        category,
        format!("Cannot create config manager: {err}"),
        None,
        None,
    )
}

fn pass(
    id: &str,
    category: &str,
    summary: impl Into<String>,
    detail: Option<String>,
    hint: Option<String>,
) -> DoctorCheckResult {
    result(id, category, DoctorStatus::Pass, summary, detail, hint)
}

fn warn(
    id: &str,
    category: &str,
    summary: impl Into<String>,
    hint: Option<String>,
) -> DoctorCheckResult {
    result(id, category, DoctorStatus::Warn, summary, None, hint)
}

fn skip(id: &str, category: &str, summary: impl Into<String>) -> DoctorCheckResult {
    result(id, category, DoctorStatus::Skip, summary, None, None)
}

fn fail(
    id: &str,
    category: &str,
    summary: impl Into<String>,
    detail: Option<String>,
    hint: Option<String>,
) -> DoctorCheckResult {
    result(id, category, DoctorStatus::Fail, summary, detail, hint)
}

fn result(
    id: &str,
    category: &str,
    status: DoctorStatus,
    summary: impl Into<String>,
    detail: Option<String>,
    hint: Option<String>,
) -> DoctorCheckResult {
    DoctorCheckResult {
        id: id.to_string(),
        category: category.to_string(),
        status,
        summary: summary.into(),
        detail,
        hint,
    }
}
