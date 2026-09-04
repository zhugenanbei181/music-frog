//! Behavior tests for the doctor facade. Every test runs against a temp-dir
//! [`DoctorEnv`]; no global home state and no real core process or network
//! is involved (the mock controller is a local loopback server).

use super::pidfile::{PidFileState, ProcessState, inspect_process, read_pid_state};
use super::*;
use crate::settings::{AppSettings, save_settings};
use std::path::Path;
use tempfile::TempDir;

const ALL_IDS: &[&str] = &[
    "config.settings_parse",
    "config.configs_dir",
    "config.current_profile",
    "config.current_yaml",
    "version.binary_available",
    "service.pid_state",
    "service.stale_pid",
    "controller.external_controller",
    "controller.api_reachable",
];

fn temp_env(tag: &str) -> (TempDir, DoctorEnv) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("doctor-{tag}-"))
        .tempdir()
        .unwrap();
    let env = DoctorEnv::with_home(dir.path().to_path_buf());
    (dir, env)
}

fn status_of(report: &DoctorReport, id: &str) -> DoctorStatus {
    report
        .checks
        .iter()
        .find(|check| check.id == id)
        .unwrap_or_else(|| panic!("check {id} missing from report"))
        .status
}

async fn save_profile(env: &DoctorEnv, content: &str) {
    let manager = env.config_manager().await.unwrap();
    manager.save("default", content).await.unwrap();
}

fn write_pid_file(env: &DoctorEnv, pid: u32) {
    std::fs::write(env.pid_file(), pid.to_string()).unwrap();
}

fn install_fake_core(env: &DoctorEnv, version: &str, executable: bool) {
    let dir = env.home().join("versions").join(version);
    std::fs::create_dir_all(&dir).unwrap();
    let binary = dir.join(if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    });
    std::fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = executable;
    std::fs::write(
        env.home().join("config.toml"),
        format!("[default]\nversion = \"{version}\"\n"),
    )
    .unwrap();
}

/// `/bin/sleep` copied under `name` so the OS process table shows a process
/// with that name; `mihomo-*` names are treated as live core processes.
#[cfg(unix)]
fn spawn_named_sleep(dir: &Path, name: &str) -> std::process::Child {
    let program = dir.join(name);
    std::fs::copy("/bin/sleep", &program).expect("copy sleep binary");
    // ETXTBSY: the kernel may still hold a write reference right after copy;
    // retry a handful of times with a short sleep to absorb the race.
    for attempt in 0..10 {
        match std::process::Command::new(&program).arg("30").spawn() {
            Ok(child) => return child,
            Err(e) if e.raw_os_error() == Some(26 /* ETXTBSY */) && attempt < 9 => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => panic!("spawn fake process: {e}"),
        }
    }
    unreachable!()
}

#[cfg(unix)]
fn stop_child(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Poll until sysinfo observes the freshly spawned process under `hint`,
/// absorbing the fork/exec window where comm is not settled yet.
#[cfg(unix)]
async fn wait_alive(pid: u32, hint: &str) -> bool {
    for _ in 0..40 {
        if inspect_process(pid, hint) == ProcessState::AliveCore {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

#[test]
fn list_checks_aligns_with_mihomo_rs_ids() {
    let ids: Vec<&str> = list_checks().iter().map(|meta| meta.id).collect();
    assert_eq!(ids, ALL_IDS);
    assert!(list_checks().iter().all(|meta| meta.default_enabled));
    let fixable: Vec<&str> = list_checks()
        .iter()
        .filter(|meta| meta.fixable)
        .map(|meta| meta.id)
        .collect();
    assert_eq!(
        fixable,
        vec![
            "config.configs_dir",
            "config.current_yaml",
            "service.stale_pid",
            "controller.external_controller",
        ]
    );
}

#[test]
fn explain_check_returns_meta_and_rejects_unknown() {
    let meta = explain_check("service.stale_pid").unwrap();
    assert_eq!(meta.category, "service");
    assert!(meta.fixable);
    assert!(meta.fail_means.contains("pid file"));
    assert!(explain_check("no.such_check").is_err());
}

#[test]
fn filter_tokens_match_category_id_and_prefix() {
    let none = CheckFilter::parse(None);
    assert!(none.matches("config.settings_parse", "config"));
    assert!(none.matches("service.stale_pid", "service"));

    let category = CheckFilter::parse(Some("service"));
    assert!(category.matches("service.pid_state", "service"));
    assert!(category.matches("service.stale_pid", "service"));
    assert!(!category.matches("config.current_yaml", "config"));

    let prefix = CheckFilter::parse(Some("service.stale"));
    assert!(prefix.matches("service.stale_pid", "service"));
    assert!(!prefix.matches("service.pid_state", "service"));

    let mixed = CheckFilter::parse(Some(" version.binary_available , controller "));
    assert!(mixed.matches("version.binary_available", "version"));
    assert!(mixed.matches("controller.api_reachable", "controller"));
    assert!(mixed.matches("controller.external_controller", "controller"));
    assert!(!mixed.matches("config.settings_parse", "config"));

    let unknown = CheckFilter::parse(Some("nonexistent"));
    assert!(!unknown.matches("config.settings_parse", "config"));
}

#[test]
fn report_status_helpers_and_exit_code() {
    fn check(id: &str, status: DoctorStatus) -> DoctorCheckResult {
        DoctorCheckResult {
            id: id.to_string(),
            category: "config".to_string(),
            status,
            summary: "s".to_string(),
            detail: None,
            hint: None,
        }
    }
    let clean = DoctorReport {
        started_at: 1,
        finished_at: 2,
        checks: vec![
            check("a", DoctorStatus::Pass),
            check("b", DoctorStatus::Warn),
        ],
    };
    let dirty = DoctorReport {
        started_at: 1,
        finished_at: 2,
        checks: vec![
            check("a", DoctorStatus::Pass),
            check("b", DoctorStatus::Fail),
        ],
    };
    assert_eq!(clean.count_by_status(DoctorStatus::Pass), 1);
    assert_eq!(clean.count_by_status(DoctorStatus::Warn), 1);
    assert_eq!(clean.count_by_status(DoctorStatus::Skip), 0);
    assert!(!clean.has_failures());
    assert_eq!(exit_code(&clean), 0);
    assert!(dirty.has_failures());
    assert_eq!(exit_code(&dirty), 1);
}

#[tokio::test]
async fn settings_parse_check_covers_missing_valid_and_broken() {
    let (_dir, env) = temp_env("settings");
    let report = run_with(&env, Some("config.settings_parse")).await;
    assert_eq!(
        status_of(&report, "config.settings_parse"),
        DoctorStatus::Pass
    );

    save_settings(env.settings_file(), &AppSettings::default())
        .await
        .unwrap();
    let report = run_with(&env, Some("config.settings_parse")).await;
    assert_eq!(
        status_of(&report, "config.settings_parse"),
        DoctorStatus::Pass
    );

    std::fs::write(env.settings_file(), "language = [unclosed").unwrap();
    let report = run_with(&env, Some("config.settings_parse")).await;
    assert_eq!(
        status_of(&report, "config.settings_parse"),
        DoctorStatus::Fail
    );
    let check = report
        .checks
        .iter()
        .find(|check| check.id == "config.settings_parse")
        .unwrap();
    assert!(check.hint.is_some());
}

#[tokio::test]
async fn configs_dir_warns_then_fix_creates_it() {
    let (_dir, env) = temp_env("configs-dir");
    let report = run_with(&env, Some("config.configs_dir")).await;
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Warn);

    let fixed = fix_with(&env, Some("config.configs_dir")).await.unwrap();
    assert_eq!(fixed.actions.len(), 1);
    assert_eq!(fixed.actions[0].id, "config.configs_dir");

    let report = run_with(&env, Some("config.configs_dir")).await;
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Pass);

    let fixed_again = fix_with(&env, Some("config.configs_dir")).await.unwrap();
    assert!(fixed_again.actions.is_empty());
}

/// configs 目录相关检查与 fix 必须跟随 env 注入的 settings 文件里的
/// `configs_dir` 重定向；`fix_configs_dir` 建的也是解析后的目录。
#[tokio::test]
async fn config_checks_follow_settings_configs_dir_redirect() {
    let dir = tempfile::Builder::new()
        .prefix("doctor-redirect-")
        .tempdir()
        .unwrap();
    let cloud = dir.path().join("cloud").join("profiles");
    let _guard =
        crate::settings::test_support::RedirectGuard::acquire(dir.path().to_path_buf()).await;
    let env = DoctorEnv::with_home(dir.path().to_path_buf());
    save_settings(
        env.settings_file(),
        &AppSettings {
            configs_dir: Some(cloud.to_string_lossy().to_string()),
            ..AppSettings::default()
        },
    )
    .await
    .unwrap();

    // 目录缺失：warn，且 summary 指向重定向目录。
    let report = run_with(&env, Some("config.configs_dir")).await;
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Warn);
    let check = report
        .checks
        .iter()
        .find(|check| check.id == "config.configs_dir")
        .unwrap();
    assert!(
        check.summary.contains(cloud.to_str().unwrap()),
        "summary: {}",
        check.summary
    );

    // fix 建的是解析后的目录，而不是 `<home>/configs`。
    let fixed = fix_with(&env, Some("config.configs_dir")).await.unwrap();
    assert_eq!(fixed.actions.len(), 1);
    assert!(fixed.actions[0].summary.contains(cloud.to_str().unwrap()));
    assert!(cloud.is_dir());
    assert!(!dir.path().join("configs").exists());

    let report = run_with(&env, Some("config.configs_dir")).await;
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Pass);
}

#[tokio::test]
async fn profile_checks_fail_then_config_fix_heals() {
    let (_dir, env) = temp_env("profiles");
    let report = run_with(&env, Some("config")).await;
    assert_eq!(
        status_of(&report, "config.current_profile"),
        DoctorStatus::Fail
    );
    assert_eq!(
        status_of(&report, "config.current_yaml"),
        DoctorStatus::Fail
    );

    let fixed = fix_with(&env, Some("config")).await.unwrap();
    assert_eq!(
        fixed
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>(),
        vec!["config.configs_dir", "config.current_yaml"]
    );

    let report = run_with(&env, Some("config")).await;
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Pass);
    assert_eq!(
        status_of(&report, "config.current_profile"),
        DoctorStatus::Pass
    );
    assert_eq!(
        status_of(&report, "config.current_yaml"),
        DoctorStatus::Pass
    );
}

#[tokio::test]
async fn current_yaml_rejects_non_mapping_top_level() {
    let (_dir, env) = temp_env("yaml-scalar");
    save_profile(&env, "42\n").await;
    let report = run_with(&env, Some("config.current_yaml")).await;
    assert_eq!(
        status_of(&report, "config.current_yaml"),
        DoctorStatus::Fail
    );
}

#[tokio::test]
async fn current_yaml_skips_when_profile_path_unavailable() {
    let (_dir, env) = temp_env("yaml-skip");
    // Unresolvable configs dir (broken settings file) makes the path
    // derivation fail, which this check reports as Skip rather than Fail.
    std::fs::create_dir_all(env.home()).unwrap();
    std::fs::write(env.home().join("config.toml"), "= not toml").unwrap();
    let report = run_with(&env, Some("config.current_yaml")).await;
    assert_eq!(
        status_of(&report, "config.current_yaml"),
        DoctorStatus::Skip
    );
}

#[tokio::test]
async fn binary_available_requires_executable_default_core() {
    let (_dir, env) = temp_env("binary");
    let report = run_with(&env, Some("version.binary_available")).await;
    assert_eq!(
        status_of(&report, "version.binary_available"),
        DoctorStatus::Fail
    );

    install_fake_core(&env, "v1.0", true);
    let report = run_with(&env, Some("version.binary_available")).await;
    assert_eq!(
        status_of(&report, "version.binary_available"),
        DoctorStatus::Pass
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let binary = env.home().join("versions/v1.0/mihomo");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o644)).unwrap();
        let report = run_with(&env, Some("version.binary_available")).await;
        assert_eq!(
            status_of(&report, "version.binary_available"),
            DoctorStatus::Fail
        );
    }
}

#[tokio::test]
async fn dead_pid_file_warns_fails_and_fix_removes() {
    let (_dir, env) = temp_env("pid-dead");
    write_pid_file(&env, u32::MAX);

    let report = run_with(&env, Some("service")).await;
    assert_eq!(status_of(&report, "service.pid_state"), DoctorStatus::Warn);
    assert_eq!(status_of(&report, "service.stale_pid"), DoctorStatus::Fail);

    let fixed = fix_with(&env, Some("service.stale_pid")).await.unwrap();
    assert_eq!(fixed.actions.len(), 1);
    assert_eq!(fixed.actions[0].id, "service.stale_pid");
    assert!(!env.pid_file().exists());

    let report = run_with(&env, Some("service")).await;
    assert_eq!(status_of(&report, "service.pid_state"), DoctorStatus::Pass);
    assert_eq!(status_of(&report, "service.stale_pid"), DoctorStatus::Pass);
}

#[tokio::test]
async fn malformed_pid_file_fails_and_fix_removes() {
    let (_dir, env) = temp_env("pid-bad");
    std::fs::write(env.pid_file(), "not-a-pid").unwrap();

    let report = run_with(&env, Some("service.stale_pid")).await;
    assert_eq!(status_of(&report, "service.stale_pid"), DoctorStatus::Fail);

    match read_pid_state(&env.pid_file()).await {
        PidFileState::Malformed(raw) => assert_eq!(raw, "not-a-pid"),
        other => panic!("expected malformed state, got {other:?}"),
    }

    fix_with(&env, Some("service.stale_pid")).await.unwrap();
    assert!(!env.pid_file().exists());
}

#[cfg(unix)]
#[tokio::test]
async fn foreign_process_behind_pid_file_counts_as_stale() {
    let (_dir, env) = temp_env("pid-foreign");
    let child = spawn_named_sleep(_dir.path(), "plain-sleep");
    write_pid_file(&env, child.id());
    assert!(
        wait_alive(child.id(), "sleep").await,
        "sleep process visible"
    );

    let state = read_pid_state(&env.pid_file()).await;
    assert_eq!(
        state,
        PidFileState::Recorded {
            pid: child.id(),
            process: ProcessState::AliveForeign
        }
    );

    let report = run_with(&env, Some("service.stale_pid")).await;
    assert_eq!(status_of(&report, "service.stale_pid"), DoctorStatus::Fail);

    fix_with(&env, Some("service.stale_pid")).await.unwrap();
    assert!(!env.pid_file().exists());
    stop_child(child);
}

#[cfg(unix)]
#[tokio::test]
async fn mihomo_named_process_counts_as_live_core() {
    let (_dir, _env) = temp_env("pid-core");
    let child = spawn_named_sleep(_dir.path(), "mihomo-fake");
    assert!(
        wait_alive(child.id(), "mihomo").await,
        "mihomo-named process should match the core hint"
    );
    assert_eq!(
        inspect_process(child.id(), "mihomo"),
        ProcessState::AliveCore
    );
    stop_child(child);
}

#[tokio::test]
async fn api_reachable_skips_when_service_stopped() {
    let (_dir, env) = temp_env("api-skip");
    let report = run_with(&env, Some("controller.api_reachable")).await;
    assert_eq!(
        status_of(&report, "controller.api_reachable"),
        DoctorStatus::Skip
    );
}

#[cfg(unix)]
#[tokio::test]
async fn api_reachable_passes_with_local_mock_controller() {
    let (_dir, env) = temp_env("api-pass");
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/version")
        .with_status(200)
        .with_body(r#"{"version":"v1.19.2","premium":false}"#)
        .create_async()
        .await;

    let child = spawn_named_sleep(_dir.path(), "mihomo-mock");
    write_pid_file(&env, child.id());
    assert!(wait_alive(child.id(), "mihomo").await, "fake core visible");
    save_profile(&env, &format!("external-controller: {}\n", server.url())).await;

    let report = run_with(&env, Some("controller.api_reachable")).await;
    let check = report
        .checks
        .iter()
        .find(|check| check.id == "controller.api_reachable")
        .unwrap();
    assert_eq!(
        check.status,
        DoctorStatus::Pass,
        "summary: {}",
        check.summary
    );
    assert!(check.summary.contains("v1.19.2"));
    stop_child(child);
}

#[cfg(unix)]
#[tokio::test]
async fn api_reachable_fails_on_running_core_with_bad_controller_url() {
    let (_dir, env) = temp_env("api-fail");
    let child = spawn_named_sleep(_dir.path(), "mihomo-bad");
    write_pid_file(&env, child.id());
    assert!(wait_alive(child.id(), "mihomo").await, "fake core visible");
    // Parses as YAML but cannot become a URL: the client must refuse before
    // any network is touched.
    save_profile(&env, "external-controller: \"not a url\"\n").await;

    let report = run_with(&env, Some("controller.api_reachable")).await;
    assert_eq!(
        status_of(&report, "controller.api_reachable"),
        DoctorStatus::Fail
    );
    stop_child(child);
}

#[tokio::test]
async fn external_controller_check_skips_then_passes_after_fix() {
    let (_dir, env) = temp_env("controller");
    let report = run_with(&env, Some("controller.external_controller")).await;
    assert_eq!(
        status_of(&report, "controller.external_controller"),
        DoctorStatus::Skip
    );

    fix_with(&env, Some("config.current_yaml")).await.unwrap();
    let report = run_with(&env, Some("controller.external_controller")).await;
    assert_eq!(
        status_of(&report, "controller.external_controller"),
        DoctorStatus::Pass
    );

    // Already configured: normally a no-op, but ambient port occupancy can
    // force a rotation write; either way the endpoint must keep resolving.
    let fixed = fix_with(&env, Some("controller.external_controller"))
        .await
        .unwrap();
    for action in &fixed.actions {
        assert_eq!(action.id, "controller.external_controller");
    }
    let manager = env.config_manager().await.unwrap();
    assert!(manager.get_external_controller().await.is_ok());
}

#[tokio::test]
async fn full_report_keeps_check_order_and_exit_contract() {
    let (_dir, env) = temp_env("full");
    let report = run_with(&env, None).await;
    let ids: Vec<&str> = report
        .checks
        .iter()
        .map(|check| check.id.as_str())
        .collect();
    assert_eq!(ids, ALL_IDS);
    assert!(report.started_at > 0);
    assert!(report.finished_at >= report.started_at);
    assert_eq!(
        status_of(&report, "config.settings_parse"),
        DoctorStatus::Pass
    );
    assert_eq!(status_of(&report, "config.configs_dir"), DoctorStatus::Warn);
    assert_eq!(
        status_of(&report, "config.current_profile"),
        DoctorStatus::Fail
    );
    assert_eq!(
        status_of(&report, "config.current_yaml"),
        DoctorStatus::Fail
    );
    assert_eq!(
        status_of(&report, "version.binary_available"),
        DoctorStatus::Fail
    );
    assert_eq!(status_of(&report, "service.pid_state"), DoctorStatus::Pass);
    assert_eq!(status_of(&report, "service.stale_pid"), DoctorStatus::Pass);
    assert_eq!(
        status_of(&report, "controller.external_controller"),
        DoctorStatus::Skip
    );
    assert_eq!(
        status_of(&report, "controller.api_reachable"),
        DoctorStatus::Skip
    );
    assert!(report.has_failures());
    assert_eq!(exit_code(&report), 1);

    let service_only = run_with(&env, Some("service")).await;
    assert_eq!(service_only.checks.len(), 2);
}
