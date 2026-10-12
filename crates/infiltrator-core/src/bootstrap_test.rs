//! Behavior tests for the one-shot bootstrap orchestration, run against
//! temp-dir homes. No global state, no real core process.

use super::*;
use crate::settings_io::app_config_manager_in;
use crate::settings_io::test_support::RedirectGuard;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_ports::profile_store::{
    ActiveProfileWorkspacePublishOutcome, ActiveProfileWorkspaceRecoveryResolution, ProfileStore,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use mihomo_config::manager::ConfigManager;
use mihomo_platform::defaults::DefaultCredentialStore;
use tempfile::TempDir;
use tokio::fs::read_to_string;

fn temp_home(tag: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(&format!("bootstrap-{tag}-"))
        .tempdir()
        .unwrap()
}

fn step_of<'a>(report: &'a BootstrapReport, id: &str) -> &'a BootstrapStep {
    report
        .steps
        .iter()
        .find(|step| step.id == id)
        .unwrap_or_else(|| panic!("bootstrap step {id} missing"))
}

#[tokio::test]
async fn fresh_home_bootstrap_creates_dir_config_and_controller() {
    let dir = temp_home("fresh");
    let report = ensure_bootstrap_at(dir.path()).await.unwrap();

    let ids: Vec<&str> = report.steps.iter().map(|step| step.id).collect();
    assert_eq!(
        ids,
        vec!["configs_dir", "default_config", "external_controller"]
    );
    assert!(step_of(&report, "configs_dir").executed);
    assert!(step_of(&report, "default_config").executed);
    // The external-controller step's executed flag depends on ambient port
    // occupancy (parallel tests may hold the probed port and force a
    // rotation), so only the outcome is asserted here.
    assert!(
        step_of(&report, "external_controller")
            .detail
            .contains("external-controller")
    );

    let manager = ConfigManager::with_home_and_store(
        dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    let profile_path = manager.get_current_path().await.unwrap();
    assert!(profile_path.is_file());
    assert!(manager.get_external_controller().await.is_ok());
    let profile = read_to_string(profile_path).await.unwrap();
    let document = yaml_rust2::YamlLoader::load_from_str(&profile)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let secret = document["secret"].as_str().unwrap();
    assert_eq!(secret.len(), 64);
    assert!(secret.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[tokio::test]
async fn bootstrap_is_idempotent_on_initialized_home() {
    let dir = temp_home("idempotent");
    let first = ensure_bootstrap_at(dir.path()).await.unwrap();
    assert!(first.any_executed());
    // configs_dir and default_config are deterministic; the external-
    // controller step may rotate when the probed port is ambient-occupied.
    assert!(first.executed_steps().count() >= 2);

    let second = ensure_bootstrap_at(dir.path()).await.unwrap();
    assert_eq!(second.steps.len(), 3);
    let rerun_flapped = second
        .steps
        .iter()
        .any(|step| step.id == "external_controller" && step.executed);
    assert!(
        second
            .steps
            .iter()
            .filter(|step| step.id != "external_controller")
            .all(|step| !step.executed),
        "directory and config steps must be skipped on re-run"
    );
    if !rerun_flapped {
        assert!(!second.any_executed());
    }
    let manager = ConfigManager::with_home_and_store(
        dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    assert!(manager.get_external_controller().await.is_ok());
}

#[tokio::test]
async fn bootstrap_repairs_missing_external_controller() {
    let dir = temp_home("repair-controller");
    ensure_bootstrap_at(dir.path()).await.unwrap();

    let manager = ConfigManager::with_home_and_store(
        dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    replace_active_profile_document(&manager, "port: 7890\n").await;

    let report = ensure_bootstrap_at(dir.path()).await.unwrap();
    let step = step_of(&report, "external_controller");
    assert!(step.executed, "detail: {}", step.detail);
    assert!(manager.get_external_controller().await.is_ok());
}

async fn replace_active_profile_document(
    manager: &ConfigManager<DefaultCredentialStore>,
    content: &str,
) {
    let reference = match ProfileStore::read_active_profile_reference(manager)
        .await
        .expect("read active profile reference")
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected a selected profile, got {state:?}"),
    };
    let previous = ProfileStore::load_active_workspace(manager)
        .await
        .expect("load active profile workspace");
    let lease = ProfileStore::acquire_active_workspace_write_lease(
        manager,
        reference,
        previous.identity(),
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("acquire unowned active workspace lease");
    assert_eq!(lease.observed_workspace(), &previous);
    let update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: content.as_bytes().to_vec(),
        sidecar_bytes: previous.sidecar_bytes.clone(),
    };
    let published = match lease.publish(update).await {
        ActiveProfileWorkspacePublishOutcome::Published(published) => published,
        ActiveProfileWorkspacePublishOutcome::Unpublished(error) => {
            panic!("active workspace was not published: {error}")
        }
        ActiveProfileWorkspacePublishOutcome::Superseded { observed } => {
            panic!("active workspace was superseded: {observed:?}")
        }
        ActiveProfileWorkspacePublishOutcome::RecoveryRequired(recovery) => {
            match recovery.resolve().await {
                ActiveProfileWorkspaceRecoveryResolution::Published(published) => published,
                ActiveProfileWorkspaceRecoveryResolution::Restored(restored) => {
                    restored.finish();
                    panic!("active workspace was restored instead of published")
                }
                ActiveProfileWorkspaceRecoveryResolution::Unpublished(error) => {
                    panic!("active workspace publication was not completed: {error}")
                }
                ActiveProfileWorkspaceRecoveryResolution::Superseded { observed } => {
                    panic!("active workspace was superseded during recovery: {observed:?}")
                }
                ActiveProfileWorkspaceRecoveryResolution::RecoveryRequired(recovery) => {
                    panic!(
                        "active workspace recovery remains unresolved: {:?}",
                        recovery.state()
                    )
                }
            }
        }
    };
    assert_eq!(
        published
            .published_workspace()
            .document
            .raw_bytes
            .as_slice(),
        content.as_bytes()
    );
    published.commit();
}

#[tokio::test]
async fn bootstrap_report_is_serializable() {
    let dir = temp_home("serialize");
    let report = ensure_bootstrap_at(dir.path()).await.unwrap();
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("configs_dir"));
    assert!(json.contains("executed"));
}

/// settings 的 `configs_dir` 指向云同步目录时，bootstrap 必须把 configs
/// 目录与默认配置建到解析后的目录，且保持幂等。
#[tokio::test]
async fn bootstrap_follows_settings_configs_dir_redirect() {
    let dir = temp_home("redirect");
    let cloud = dir.path().join("cloud").join("sync");
    let guard = RedirectGuard::acquire(dir.path().to_path_buf()).await;
    guard
        .set_configs_dir(dir.path(), Some(cloud.to_str().unwrap()))
        .await;

    let report = ensure_bootstrap_at(dir.path()).await.unwrap();
    assert!(step_of(&report, "configs_dir").executed);
    assert!(cloud.is_dir());
    assert!(!dir.path().join("configs").exists());

    // 当前 profile 的默认配置同样落在重定向目录。
    let manager = app_config_manager_in(dir.path()).await.unwrap();
    assert!(manager.get_current_path().await.unwrap().is_file());

    // 幂等：重跑 configs_dir / default_config 步骤必须跳过。
    let second = ensure_bootstrap_at(dir.path()).await.unwrap();
    assert!(!step_of(&second, "configs_dir").executed);
    assert!(!step_of(&second, "default_config").executed);
}
