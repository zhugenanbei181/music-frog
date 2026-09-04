//! Doctor diagnostics and startup bootstrap for the Kotlin side: read-only
//! health checks with safe auto-repair, the static check catalog, and the
//! one-shot configs/default-config/controller bootstrap.
//!
//! Errors travel in the crate's `FfiStatus` records (the existing error
//! channel); check statuses are flattened to "pass"/"warn"/"fail"/"skip".

use infiltrator_contract::doctor::{
    BootstrapStep, DoctorCheckMeta, DoctorCheckResult, DoctorFixAction, DoctorStatus,
};

use super::support::{doctor_application, get_runtime, map_application_failure};
use crate::ffi::{FfiErrorCode, FfiStatus};

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorCheckResultRecord {
    pub id: String,
    pub category: String,
    pub status: String,
    pub summary: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorReportRecord {
    pub status: FfiStatus,
    pub started_at: u64,
    pub finished_at: u64,
    pub has_failures: bool,
    pub checks: Vec<DoctorCheckResultRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorFixActionRecord {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorFixResult {
    pub status: FfiStatus,
    pub actions: Vec<DoctorFixActionRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorCheckMetaRecord {
    pub id: String,
    pub category: String,
    pub summary: String,
    pub why: String,
    pub fail_means: String,
    pub hint: String,
    pub fixable: bool,
    pub default_enabled: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DoctorCheckMetaResult {
    pub status: FfiStatus,
    pub check: Option<DoctorCheckMetaRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BootstrapStepRecord {
    pub id: String,
    pub executed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BootstrapResult {
    pub status: FfiStatus,
    pub steps: Vec<BootstrapStepRecord>,
}

#[uniffi::export]
pub async fn doctor_run(only: Option<String>) -> DoctorReportRecord {
    get_runtime()
        .spawn(async move {
            match doctor_application() {
                Ok(application) => match application.run(only).await {
                    Ok(report) => DoctorReportRecord {
                        status: FfiStatus::ok(),
                        started_at: report.started_at,
                        finished_at: report.finished_at,
                        has_failures: report.has_failures(),
                        checks: report.checks.into_iter().map(check_to_record).collect(),
                    },
                    Err(failure) => DoctorReportRecord {
                        status: map_application_failure(failure),
                        started_at: 0,
                        finished_at: 0,
                        has_failures: true,
                        checks: Vec::new(),
                    },
                },
                Err(status) => DoctorReportRecord {
                    status,
                    started_at: 0,
                    finished_at: 0,
                    has_failures: true,
                    checks: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| DoctorReportRecord {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            started_at: 0,
            finished_at: 0,
            has_failures: true,
            checks: Vec::new(),
        })
}

#[uniffi::export]
pub async fn doctor_fix(only: Option<String>) -> DoctorFixResult {
    get_runtime()
        .spawn(async move {
            match doctor_application() {
                Ok(application) => match application.fix(only).await {
                    Ok(report) => DoctorFixResult {
                        status: FfiStatus::ok(),
                        actions: report
                            .actions
                            .into_iter()
                            .map(fix_action_to_record)
                            .collect(),
                    },
                    Err(failure) => DoctorFixResult {
                        status: map_application_failure(failure),
                        actions: Vec::new(),
                    },
                },
                Err(status) => DoctorFixResult {
                    status,
                    actions: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| DoctorFixResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            actions: Vec::new(),
        })
}

#[uniffi::export]
pub fn doctor_checks() -> Vec<DoctorCheckMetaRecord> {
    doctor_application()
        .map(|application| application.list_checks())
        .unwrap_or_default()
        .into_iter()
        .map(check_meta_to_record)
        .collect()
}

#[uniffi::export]
pub fn doctor_explain(id: String) -> DoctorCheckMetaResult {
    match doctor_application().and_then(|application| {
        application
            .explain(&id)
            .map_err(map_application_failure)
    }) {
        Ok(meta) => DoctorCheckMetaResult {
            status: FfiStatus::ok(),
            check: Some(check_meta_to_record(meta)),
        },
        Err(err) => DoctorCheckMetaResult {
            status: err,
            check: None,
        },
    }
}

#[uniffi::export]
pub async fn bootstrap_now() -> BootstrapResult {
    get_runtime()
        .spawn(async move {
            match bootstrap_internal().await {
                Ok(steps) => BootstrapResult {
                    status: FfiStatus::ok(),
                    steps,
                },
                Err(status) => BootstrapResult {
                    status,
                    steps: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| BootstrapResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            steps: Vec::new(),
        })
}

async fn bootstrap_internal() -> Result<Vec<BootstrapStepRecord>, FfiStatus> {
    let report = doctor_application()?
        .bootstrap()
        .await
        .map_err(map_application_failure)?;
    Ok(report.steps.into_iter().map(step_to_record).collect())
}

fn status_to_string(status: DoctorStatus) -> String {
    match status {
        DoctorStatus::Pass => "pass",
        DoctorStatus::Warn => "warn",
        DoctorStatus::Fail => "fail",
        DoctorStatus::Skip => "skip",
    }
    .to_string()
}

fn check_to_record(check: DoctorCheckResult) -> DoctorCheckResultRecord {
    DoctorCheckResultRecord {
        id: check.id,
        category: check.category,
        status: status_to_string(check.status),
        summary: check.summary,
        detail: check.detail,
        hint: check.hint,
    }
}

fn fix_action_to_record(action: DoctorFixAction) -> DoctorFixActionRecord {
    DoctorFixActionRecord {
        id: action.id,
        summary: action.summary,
    }
}

fn check_meta_to_record(meta: DoctorCheckMeta) -> DoctorCheckMetaRecord {
    DoctorCheckMetaRecord {
        id: meta.id.to_string(),
        category: meta.category.to_string(),
        summary: meta.summary.to_string(),
        why: meta.why.to_string(),
        fail_means: meta.fail_means.to_string(),
        hint: meta.hint.to_string(),
        fixable: meta.fixable,
        default_enabled: meta.default_enabled,
    }
}

fn step_to_record(step: BootstrapStep) -> BootstrapStepRecord {
    BootstrapStepRecord {
        id: step.id,
        executed: step.executed,
        detail: step.detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::FfiErrorCode;
    use mihomo_platform::TEST_LOCK;
    use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

    async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn make_test_home(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("infiltrator-android-doctor-{tag}-{unique}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create test home dir");
        path
    }

    fn set_env(value: &str) {
        unsafe { std::env::set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { std::env::remove_var(CONFIGS_DIR_ENV) };
    }

    fn restore_env(saved: Option<String>) {
        match saved {
            Some(value) => set_env(&value),
            None => clear_env(),
        }
    }

    fn check_of<'a>(report: &'a DoctorReportRecord, id: &str) -> &'a DoctorCheckResultRecord {
        report
            .checks
            .iter()
            .find(|check| check.id == id)
            .unwrap_or_else(|| panic!("check {id} missing"))
    }

    fn step_of<'a>(result: &'a BootstrapResult, id: &str) -> &'a BootstrapStepRecord {
        result
            .steps
            .iter()
            .find(|step| step.id == id)
            .unwrap_or_else(|| panic!("bootstrap step {id} missing"))
    }

    #[test]
    fn doctor_status_strings_match_core_serialization() {
        assert_eq!(status_to_string(DoctorStatus::Pass), "pass");
        assert_eq!(status_to_string(DoctorStatus::Warn), "warn");
        assert_eq!(status_to_string(DoctorStatus::Fail), "fail");
        assert_eq!(status_to_string(DoctorStatus::Skip), "skip");
    }

    #[test]
    fn doctor_checks_lists_the_catalog() {
        let checks = doctor_checks();
        assert!(!checks.is_empty());
        let mut ids: Vec<&str> = checks.iter().map(|check| check.id.as_str()).collect();
        let id_count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), id_count, "check ids must be unique");
        assert!(checks.iter().any(|check| check.id == "config.configs_dir"));
        assert!(
            checks
                .iter()
                .any(|check| check.id == "config.configs_dir" && check.fixable)
        );
    }

    #[test]
    fn doctor_explain_known_and_unknown() {
        let known = doctor_explain("config.configs_dir".to_string());
        assert_eq!(known.status.code, FfiErrorCode::Ok);
        let meta = known.check.expect("known id must carry meta");
        assert_eq!(meta.id, "config.configs_dir");
        assert_eq!(meta.category, "config");
        assert!(!meta.why.is_empty());

        let unknown = doctor_explain("no.such_check".to_string());
        assert_eq!(unknown.status.code, FfiErrorCode::Unknown);
        assert!(unknown.check.is_none());
        assert!(
            unknown
                .status
                .message
                .is_some_and(|message| message.contains("unknown doctor check"))
        );
    }

    #[tokio::test]
    async fn doctor_run_before_and_after_bootstrap() {
        let _guard = test_lock().await;
        let home = make_test_home("bootstrap-run");
        set_home_dir_override(home.clone());

        let before = doctor_run(None).await;
        assert_eq!(before.status.code, FfiErrorCode::Ok);
        assert_eq!(check_of(&before, "config.configs_dir").status, "warn");

        let boot = bootstrap_now().await;
        assert_eq!(boot.status.code, FfiErrorCode::Ok);
        let ids: Vec<String> = boot.steps.iter().map(|step| step.id.clone()).collect();
        assert_eq!(
            ids,
            vec![
                "configs_dir".to_string(),
                "default_config".to_string(),
                "external_controller".to_string()
            ]
        );
        assert!(step_of(&boot, "configs_dir").executed);

        let rerun = bootstrap_now().await;
        assert!(!step_of(&rerun, "configs_dir").executed);

        let after = doctor_run(None).await;
        assert_eq!(after.status.code, FfiErrorCode::Ok);
        assert_eq!(check_of(&after, "config.configs_dir").status, "pass");
        assert_eq!(check_of(&after, "controller.api_reachable").status, "skip");
        assert!(after.started_at > 0);
        assert!(after.finished_at >= after.started_at);
        assert_eq!(
            after.has_failures,
            after.checks.iter().any(|check| check.status == "fail")
        );

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn doctor_run_honors_only_filter() {
        let _guard = test_lock().await;
        let home = make_test_home("only-filter");
        set_home_dir_override(home.clone());

        let report = doctor_run(Some("config".to_string())).await;
        assert_eq!(report.status.code, FfiErrorCode::Ok);
        assert!(!report.checks.is_empty());
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.category == "config" && check.id.starts_with("config."))
        );

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn doctor_fix_creates_missing_configs_dir() {
        let _guard = test_lock().await;
        let home = make_test_home("fix-configs");
        set_home_dir_override(home.clone());

        let fixed = doctor_fix(Some("config.configs_dir".to_string())).await;
        assert_eq!(fixed.status.code, FfiErrorCode::Ok);
        assert_eq!(fixed.actions.len(), 1);
        assert_eq!(fixed.actions[0].id, "config.configs_dir");
        assert!(home.join("configs").is_dir());

        let fixed_again = doctor_fix(Some("config.configs_dir".to_string())).await;
        assert_eq!(fixed_again.status.code, FfiErrorCode::Ok);
        assert!(fixed_again.actions.is_empty());

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn doctor_follows_configs_dir_env() {
        let _guard = test_lock().await;
        let home = make_test_home("env-dir");
        set_home_dir_override(home.clone());

        let saved = std::env::var(CONFIGS_DIR_ENV).ok();
        let env_dir = home.join("env-cloud");
        set_env(env_dir.to_str().unwrap());

        let report = doctor_run(Some("config.configs_dir".to_string())).await;
        assert_eq!(check_of(&report, "config.configs_dir").status, "warn");

        let fixed = doctor_fix(Some("config.configs_dir".to_string())).await;
        assert_eq!(fixed.status.code, FfiErrorCode::Ok);
        assert!(env_dir.is_dir());

        restore_env(saved);
        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn doctor_run_reads_settings_through_settings_path() {
        let _guard = test_lock().await;
        let home = make_test_home("settings-parse");
        set_home_dir_override(home.clone());
        fs::write(settings_toml_path(&home), "not = [valid").expect("write broken settings");

        let report = doctor_run(Some("config.settings_parse".to_string())).await;
        assert_eq!(report.status.code, FfiErrorCode::Ok);
        assert_eq!(check_of(&report, "config.settings_parse").status, "fail");

        clear_home_dir_override();
        let _ = fs::remove_dir_all(home);
    }

    fn settings_toml_path(home: &std::path::Path) -> PathBuf {
        home.join("settings.toml")
    }
}
