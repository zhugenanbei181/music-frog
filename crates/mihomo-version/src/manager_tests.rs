use super::*;
use crate::engine;
#[cfg(test)]
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(test)]
#[cfg(unix)]
use std::fs::create_dir_all;
#[cfg(test)]
#[cfg(unix)]
use std::fs::set_permissions;
#[cfg(test)]
#[cfg(unix)]
use std::fs::write;
use tempfile::TempDir;

#[cfg(unix)]
fn write_script(dir: &TempDir, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.path().join(name);
    write(&path, format!("#!/bin/sh\n{body}")).unwrap();
    set_permissions(&path, Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[tokio::test]
async fn smoke_check_accepts_version_printing_binary() {
    let dir = TempDir::new().unwrap();
    let fake = write_script(&dir, "mihomo", "echo \"Mihomo Meta v1.19.18 test\"");
    assert!(smoke_check_binary(&fake).await.is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn smoke_check_rejects_failing_binary() {
    let dir = TempDir::new().unwrap();
    let fake = write_script(&dir, "mihomo", "exit 3");
    let err = smoke_check_binary(&fake).await.unwrap_err();
    assert!(err.to_string().contains("exited"), "{err}");
}

#[cfg(unix)]
#[tokio::test]
async fn smoke_check_rejects_silent_binary() {
    let dir = TempDir::new().unwrap();
    let fake = write_script(&dir, "mihomo", "exit 0");
    let err = smoke_check_binary(&fake).await.unwrap_err();
    assert!(err.to_string().contains("no version output"), "{err}");
}

fn setup_test_manager(temp_dir: &TempDir) -> VersionManager {
    let home = temp_dir.path().to_path_buf();
    VersionManager::for_engine(home, engine::engine_id()).unwrap()
}

#[tokio::test]
async fn cancelled_install_stops_before_network_or_filesystem_work() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let error = manager
        .install_with_progress_and_cancel("v-test", |_| {}, || true)
        .await
        .unwrap_err();
    assert!(matches!(error, MihomoError::Canceled), "{error}");
    assert!(!temp_dir.path().join("engines/mihomo/versions").exists());
}

/// set_default now smoke-checks the candidate binary; tests that merely
/// install a fake version must provide a runnable stand-in.
#[cfg(unix)]
fn plant_runnable_fake_binary(home: &path::Path, version: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join("engines/mihomo/versions").join(version);
    create_dir_all(&dir).unwrap();
    let bin = dir.join("mihomo");
    write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
    set_permissions(&bin, Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn test_version_manager_new() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    assert_eq!(manager.engine_id(), &engine::engine_id());
    assert_eq!(
        manager.install_dir,
        temp_dir.path().join("engines/mihomo/versions")
    );
    assert_eq!(
        manager.config_file,
        temp_dir.path().join("engines/mihomo/config.toml")
    );
}

#[tokio::test]
async fn observing_default_binary_on_a_fresh_home_does_not_create_namespace() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    assert_eq!(
        manager.observe_default_binary().await.unwrap(),
        DefaultBinaryObservation::NoConfiguration
    );
    assert!(!temp_dir.path().join("engines").exists());
    assert!(!temp_dir.path().join("versions").exists());
}

#[tokio::test]
async fn observing_legacy_default_binary_does_not_migrate_legacy_state() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let binary_name = if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    let legacy_binary = temp_dir.path().join("versions/v1.19.12").join(binary_name);
    fs::create_dir_all(legacy_binary.parent().unwrap())
        .await
        .unwrap();
    fs::write(&legacy_binary, "legacy binary").await.unwrap();
    let legacy_config = temp_dir.path().join("config.toml");
    let legacy_contents = "[default]\nversion = \"v1.19.12\"\nversion_history = []\n";
    fs::write(&legacy_config, legacy_contents).await.unwrap();

    assert_eq!(
        manager.observe_default_binary().await.unwrap(),
        DefaultBinaryObservation::BinaryFound {
            version: "v1.19.12".to_string(),
            path: legacy_binary.clone(),
        }
    );
    assert!(!temp_dir.path().join("engines").exists());
    assert!(legacy_binary.exists());
    assert_eq!(
        fs::read_to_string(legacy_config).await.unwrap(),
        legacy_contents
    );
}

#[tokio::test]
async fn first_access_migrates_legacy_versions_and_journal_to_mihomo_scope() {
    let temp_dir = TempDir::new().unwrap();
    let legacy_version = temp_dir.path().join("versions/v1.19.12");
    fs::create_dir_all(&legacy_version).await.unwrap();
    fs::write(legacy_version.join("mihomo"), "legacy binary")
        .await
        .unwrap();
    fs::write(
        temp_dir.path().join("config.toml"),
        "[default]\nversion = \"v1.19.12\"\nversion_history = []\n",
    )
    .await
    .unwrap();
    let manager = setup_test_manager(&temp_dir);

    let installed = manager.list_installed().await.unwrap();

    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].version, "v1.19.12");
    assert_eq!(
        installed[0].path,
        manager
            .install_dir
            .join("v1.19.12")
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(manager.get_default().await.unwrap(), "v1.19.12");
    assert!(!temp_dir.path().join("versions").exists());
    assert!(manager.config_file.exists());
    assert!(temp_dir.path().join("config.toml").exists());
}

#[tokio::test]
async fn scoped_version_and_journal_win_conflicts_without_duplicate_reads() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let scoped_existing = manager.install_dir.join("v1.19.20");
    fs::create_dir_all(&scoped_existing).await.unwrap();
    fs::write(scoped_existing.join("mihomo"), "scoped binary")
        .await
        .unwrap();
    let legacy_conflict = temp_dir.path().join("versions/v1.19.20");
    let legacy_only = temp_dir.path().join("versions/v1.19.19");
    fs::create_dir_all(&legacy_conflict).await.unwrap();
    fs::write(legacy_conflict.join("mihomo"), "legacy duplicate")
        .await
        .unwrap();
    fs::create_dir_all(&legacy_only).await.unwrap();
    fs::write(legacy_only.join("mihomo"), "legacy unique")
        .await
        .unwrap();
    fs::write(
        &manager.config_file,
        "[default]\nversion = \"v1.19.20\"\nversion_history = []\n",
    )
    .await
    .unwrap();
    fs::write(
        temp_dir.path().join("config.toml"),
        "[default]\nversion = \"v1.19.19\"\nversion_history = []\n",
    )
    .await
    .unwrap();

    let first = manager.list_installed().await.unwrap();
    let second = manager.list_installed().await.unwrap();

    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert_eq!(manager.get_default().await.unwrap(), "v1.19.20");
    assert_eq!(
        fs::read_to_string(scoped_existing.join("mihomo"))
            .await
            .unwrap(),
        "scoped binary"
    );
    assert!(legacy_conflict.join("mihomo").exists());
    assert!(!legacy_only.exists());
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("config.toml"))
            .await
            .unwrap(),
        "[default]\nversion = \"v1.19.19\"\nversion_history = []\n"
    );
}

#[tokio::test]
async fn legacy_config_migration_retries_after_read_failure_without_partial_target() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let legacy_config = temp_dir.path().join("config.toml");
    fs::create_dir_all(&legacy_config).await.unwrap();

    assert!(manager.get_default().await.is_err());
    assert!(!manager.config_file.exists());

    fs::remove_dir_all(&legacy_config).await.unwrap();
    let contents = "[default]\nversion = \"v1.19.23\"\nversion_history = []\n";
    fs::write(&legacy_config, contents).await.unwrap();

    assert_eq!(manager.get_default().await.unwrap(), "v1.19.23");
    assert_eq!(
        fs::read_to_string(manager.config_file).await.unwrap(),
        contents
    );
}

#[test]
fn mihomo_version_manager_rejects_unknown_engine_without_creating_a_namespace() {
    let temp_dir = TempDir::new().unwrap();
    let result = VersionManager::for_engine(
        temp_dir.path().to_path_buf(),
        EngineId::new("sing-box").unwrap(),
    );

    assert!(result.is_err());
    assert!(!temp_dir.path().join("engines").exists());
}

#[tokio::test]
async fn test_list_installed_empty() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let result = manager.list_installed().await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_list_installed_with_versions() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    // Create version directories
    fs::create_dir_all(manager.install_dir.join("v1.19.0"))
        .await
        .unwrap();
    fs::create_dir_all(manager.install_dir.join("v1.20.0"))
        .await
        .unwrap();

    let result = manager.list_installed().await;
    assert!(result.is_ok());
    let versions = result.unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions.iter().any(|v| v.version == "v1.19.0"));
    assert!(versions.iter().any(|v| v.version == "v1.20.0"));
}

#[tokio::test]
async fn test_list_installed_sorted() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    // Create version directories
    fs::create_dir_all(manager.install_dir.join("v1.18.0"))
        .await
        .unwrap();
    fs::create_dir_all(manager.install_dir.join("v1.20.0"))
        .await
        .unwrap();
    fs::create_dir_all(manager.install_dir.join("v1.19.0"))
        .await
        .unwrap();

    let result = manager.list_installed().await;
    assert!(result.is_ok());
    let versions = result.unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(versions[0].version, "v1.20.0");
    assert_eq!(versions[1].version, "v1.19.0");
    assert_eq!(versions[2].version, "v1.18.0");
}

#[tokio::test]
async fn test_set_default() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    // Create version directory with a binary that passes smoke check
    #[cfg(unix)]
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");
    #[cfg(not(unix))]
    fs::create_dir_all(manager.install_dir.join("v1.19.0"))
        .await
        .unwrap();

    let result = manager.set_default("v1.19.0").await;
    assert!(result.is_ok());

    let default = manager.get_default().await;
    assert!(default.is_ok());
    assert_eq!(default.unwrap(), "v1.19.0");
}

#[cfg(unix)]
#[tokio::test]
async fn version_selection_preserves_profile_and_rolls_back_without_network() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    fs::write(
        temp_dir.path().join("config.toml"),
        "[default]\nprofile = \"work\"\n",
    )
    .await
    .unwrap();
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.28");
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.29");
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.30");

    manager.set_default("v1.19.28").await.unwrap();
    manager.set_default("v1.19.29").await.unwrap();
    manager.set_default("v1.19.30").await.unwrap();

    let config = fs::read_to_string(&manager.config_file).await.unwrap();
    let config: toml::Value = toml::from_str(&config).unwrap();
    assert_eq!(
        config["default"]["profile"].as_str(),
        Some("work"),
        "version selection must not overwrite profile selection"
    );
    assert_eq!(
        config["default"]["version_history"].as_array(),
        Some(&vec![
            toml::Value::String("v1.19.29".to_owned()),
            toml::Value::String("v1.19.28".to_owned()),
        ])
    );

    let info = manager.rollback_info().await.unwrap();
    assert_eq!(info.current.as_deref(), Some("v1.19.30"));
    assert_eq!(info.target.as_deref(), Some("v1.19.29"));

    let selected = manager.rollback().await.unwrap();
    assert_eq!(selected, "v1.19.29");
    assert_eq!(manager.get_default().await.unwrap(), "v1.19.29");
    let next = manager.rollback_info().await.unwrap();
    assert_eq!(next.target.as_deref(), Some("v1.19.28"));
    assert_eq!(next.history.first().map(String::as_str), Some("v1.19.28"));
}

#[tokio::test]
async fn rollback_requires_an_installed_previous_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let error = manager.rollback().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("No installed previous core version"),
        "{error}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn rollback_health_gate_keeps_current_version_on_bad_candidate() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.28");
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.29");
    manager.set_default("v1.19.28").await.unwrap();
    manager.set_default("v1.19.29").await.unwrap();

    fs::write(
        manager.install_dir.join("v1.19.28/mihomo"),
        "#!/bin/sh\nexit 9\n",
    )
    .await
    .unwrap();
    let error = manager.rollback().await.unwrap_err();
    assert!(error.to_string().contains("smoke check"), "{error}");
    assert_eq!(manager.get_default().await.unwrap(), "v1.19.29");
    assert_eq!(
        manager.rollback_info().await.unwrap().target.as_deref(),
        Some("v1.19.28")
    );
}

#[tokio::test]
async fn test_set_nonexistent_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let result = manager.set_default("v1.19.0").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not installed"));
}

#[tokio::test]
async fn test_get_default_not_set() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let result = manager.get_default().await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No default version set")
    );
}

#[tokio::test]
async fn test_get_binary_path_with_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();

    let binary_name = if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    let binary_path = version_dir.join(binary_name);
    fs::write(&binary_path, "binary").await.unwrap();

    let result = manager.get_binary_path(Some("v1.19.0")).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), binary_path);
}

#[tokio::test]
async fn test_get_binary_path_with_default() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();

    let binary_name = if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    let binary_path = version_dir.join(binary_name);
    #[cfg(unix)]
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");
    #[cfg(not(unix))]
    fs::write(&binary_path, "binary").await.unwrap();

    manager.set_default("v1.19.0").await.unwrap();

    let result = manager.get_binary_path(None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), binary_path);
}

#[tokio::test]
async fn test_get_binary_path_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();

    let result = manager.get_binary_path(Some("v1.19.0")).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Binary not found"));
}

#[tokio::test]
async fn test_install_version_already_installed() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();

    let result = manager.install("v1.19.0").await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already installed")
    );
}

#[tokio::test]
async fn test_uninstall_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();

    let result = manager.uninstall("v1.19.0").await;
    assert!(result.is_ok());
    assert!(!version_dir.exists());
}

#[tokio::test]
async fn test_uninstall_nonexistent_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let result = manager.uninstall("v1.19.0").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not installed"));
}

#[tokio::test]
async fn test_uninstall_default_version() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);

    let version_dir = manager.install_dir.join("v1.19.0");
    fs::create_dir_all(&version_dir).await.unwrap();
    #[cfg(unix)]
    plant_runnable_fake_binary(temp_dir.path(), "v1.19.0");

    manager.set_default("v1.19.0").await.unwrap();

    let result = manager.uninstall("v1.19.0").await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Cannot uninstall the default version")
    );
}

#[tokio::test]
async fn test_install_cleanup_on_failure() {
    let temp_dir = TempDir::new().unwrap();
    let manager = setup_test_manager(&temp_dir);
    let version = "v9.9.9";

    // 预先创建一个文件占坑，导致目录创建失败
    let conflict_path = manager.install_dir.join(version);
    fs::create_dir_all(&manager.install_dir).await.unwrap();
    fs::write(&conflict_path, "I am a file, not a dir")
        .await
        .unwrap();

    // 尝试安装 (由于 Downloader 会先下载，这里可能先报下载错误)
    // 但如果我们模拟一个下载成功但后续失败的场景...
    // 鉴于目前代码结构，我增加一个内部验证：
    // 如果安装过程抛出任何错误，install_dir/version 应该不存在或者保持原样。
    let _ = manager.install(version).await;

    // 如果安装失败，它不应该留下一个半成品目录（如果是文件占坑，它不应该被删掉，但也不应该变成目录）
    assert!(fs::metadata(&conflict_path).await.unwrap().is_file());
}
