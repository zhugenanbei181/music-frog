use mihomo_config::profile::Profile;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};

use crate::test_support::EnvGuard;
use super::{apply_configs_dir_override, profile_json, profile_row};

fn sample_profile(name: &str, active: bool) -> Profile {
    Profile::new(name.to_string(), format!("/x/configs/{name}.yaml").into(), active)
}

#[test]
fn profile_rows_mark_active_profile() {
    let rows = [
        profile_row(&sample_profile("default", true)),
        profile_row(&sample_profile("work", false)),
    ];
    assert_eq!(rows[0][0], "default");
    assert_eq!(rows[0][1], "*");
    assert_eq!(rows[1][1], "");
    assert!(rows[1][2].ends_with("work.yaml"));
}

#[test]
fn profile_json_carries_name_active_and_path() {
    let value = profile_json(&sample_profile("default", true));
    assert_eq!(value["name"], "default");
    assert_eq!(value["active"], true);
    assert!(value["path"]
        .as_str()
        .unwrap()
        .ends_with("default.yaml"));
}

/// The settings-backed configs-dir override must survive a save/reload round
/// trip and redirect the resolved configs directory. Runs under TEST_LOCK
/// with the home override because `ConfigManager::with_configs_dir` resolves
/// the settings-file location through `get_home_dir`.
#[tokio::test]
async fn configs_dir_override_round_trips_through_settings() {
    // EnvGuard already holds TEST_LOCK, serializing both the env var and the
    // home-dir override below.
    let _env = EnvGuard::acquire().await;
    let temp = tempfile::tempdir().unwrap();
    set_home_dir_override(temp.path().to_path_buf());

    let result = async {
        apply_configs_dir_override(temp.path(), Some("cloud/profiles".to_string())).await?;
        let runtime = crate::context::Runtime::with_home(temp.path().to_path_buf()).await?;
        assert_eq!(
            runtime.settings.configs_dir.as_deref(),
            Some("cloud/profiles")
        );
        assert_eq!(
            runtime.configs_dir()?,
            temp.path().join("cloud/profiles")
        );

        apply_configs_dir_override(temp.path(), None).await?;
        let runtime = crate::context::Runtime::with_home(temp.path().to_path_buf()).await?;
        assert!(runtime.settings.configs_dir.is_none());
        assert_eq!(runtime.configs_dir()?, temp.path().join("configs"));
        anyhow::Ok(())
    }
    .await;
    clear_home_dir_override();
    result.unwrap();
}

/// After a default config is created under the redirected configs directory,
/// `list_profiles` sees exactly that profile through the same manager.
#[tokio::test]
async fn profile_listing_follows_the_configs_dir_override() {
    let _env = EnvGuard::acquire().await;
    let temp = tempfile::tempdir().unwrap();
    set_home_dir_override(temp.path().to_path_buf());

    let result = async {
        apply_configs_dir_override(temp.path(), Some("cloud".to_string())).await?;
        let runtime = crate::context::Runtime::with_home(temp.path().to_path_buf()).await?;
        runtime.config_manager()?.ensure_default_config().await?;
        assert!(temp.path().join("cloud").join("default.yaml").exists());

        let profiles = runtime.config_manager()?.list_profiles().await?;
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "default");
        assert!(profiles[0].active);
        anyhow::Ok(())
    }
    .await;
    clear_home_dir_override();
    result.unwrap();
}
