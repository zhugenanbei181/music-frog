//! Profile path safety: name validation, character-level sanitization, and
//! canonicalized path construction that cannot escape the config directory.
//! Also resolves the configs storage directory (cloud-sync redirect).

use super::ConfigManager;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profiles::validate_profile_name;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::identity::EngineId;
use mihomo_api::error::{MihomoError, Result};
use std::env::{home_dir, var};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// 环境变量形式的 configs 目录覆盖，优先级高于 settings 的 `configs_dir`。
pub const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

/// Return the filesystem namespace owned by one engine below a shared config
/// root. Profile IDs are only unique inside this directory.
pub fn engine_config_dir(config_root: &Path, engine: &EngineId) -> PathBuf {
    config_root.join("engines").join(engine.as_str())
}

/// Resolve a shared config root or an already scoped engine directory to the
/// owned namespace. Reject an explicitly foreign `engines/<id>` path so a
/// profile ID can never redirect Mihomo persistence into another engine.
pub fn scope_config_dir(config_root_or_scoped: &Path, engine: &EngineId) -> Result<PathBuf> {
    let file_name = config_root_or_scoped
        .file_name()
        .and_then(|name| name.to_str());
    let parent_name = config_root_or_scoped
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    match (parent_name, file_name) {
        (Some("engines"), Some(namespace)) if namespace == engine.as_str() => {
            Ok(config_root_or_scoped.to_path_buf())
        }
        (Some("engines"), Some(namespace)) => Err(MihomoError::Config(format!(
            "config namespace `{namespace}` does not match `{}`",
            engine.as_str()
        ))),
        (_, Some("engines")) => Ok(config_root_or_scoped.join(engine.as_str())),
        _ => Ok(engine_config_dir(config_root_or_scoped, engine)),
    }
}

/// 由调用方提供 home，解析 env/settings 目录重定向。
pub fn resolve_configs_dir_in(explicit: Option<&str>, home: &Path) -> Result<PathBuf> {
    let env_value = var(CONFIGS_DIR_ENV).ok();
    for candidate in [env_value.as_deref(), explicit] {
        if let Some(dir) = candidate.map(str::trim).filter(|dir| !dir.is_empty()) {
            return expand_tilde(dir).map(|dir| {
                if dir.is_absolute() {
                    dir
                } else {
                    home.join(dir)
                }
            });
        }
    }
    Ok(home.join("configs"))
}

/// 展开用户主目录别名 `~` / `~/…` / `~\…`；其余输入原样返回。
/// `~user`（他人主目录）不支持，直接报错而不是猜测。
fn expand_tilde(path: &str) -> Result<PathBuf> {
    if !path.starts_with('~') {
        return Ok(PathBuf::from(path));
    }
    let rest = if path == "~" {
        ""
    } else {
        path[1..]
            .strip_prefix(['/', '\\'])
            .map(|rest| rest.trim_start_matches(['/', '\\']))
            .ok_or_else(|| MihomoError::Config(format!("unsupported '~' path form: {path}")))?
    };
    let home = home_dir().ok_or_else(|| {
        MihomoError::Config("could not determine user home for '~' expansion".to_string())
    })?;
    Ok(if rest.is_empty() {
        home
    } else {
        home.join(rest)
    })
}

impl<S: SecureStore> ConfigManager<S> {
    /// Resolve an existing profile document through the shared engine owner.
    pub async fn existing_profile_document_path(&self, profile: &str) -> Result<PathBuf> {
        let id = profile_id(profile)?;
        self.profile_store
            .existing_profile_document_path(&id)
            .await
            .map_err(profile_path_error)
    }
}

pub(crate) fn profile_id(profile: &str) -> Result<ProfileId> {
    validate_profile_name(profile).map_err(|error| MihomoError::Config(error.to_string()))?;
    ProfileId::new(profile.to_owned()).map_err(|error| MihomoError::Config(error.to_string()))
}

fn profile_path_error(error: PortError) -> MihomoError {
    match error {
        PortError::Rejected(failure) => MihomoError::Config(failure.message),
        PortError::NotFound(message) => MihomoError::NotFound(message),
        PortError::PermissionDenied(message) => {
            MihomoError::Io(io::Error::new(ErrorKind::PermissionDenied, message))
        }
        PortError::Io(message) => MihomoError::Io(io::Error::other(message)),
        error => MihomoError::Config(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use crate::manager;
    #[cfg(test)]
    use infiltrator_ports::error::PortError;
    use infiltrator_profile_store::namespace::ProfileNamespaceInitialization;
    #[cfg(test)]
    use std::env::remove_var;
    #[cfg(test)]
    use std::env::set_var;
    #[cfg(test)]
    use std::result;
    use tempfile::TempDir;
    #[cfg(test)]
    use tokio::sync::Mutex;
    #[cfg(test)]
    use tokio::sync::MutexGuard;

    static TEST_LOCK: Mutex<()> = Mutex::const_new(());

    // 环境变量是进程级全局状态：所有涉及 configs 目录解析的测试都必须持有
    // TEST_LOCK 串行执行，避免互相串扰（与仓库既有 env 测试做法一致）。
    async fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn set_env(value: &str) {
        unsafe { set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { remove_var(CONFIGS_DIR_ENV) };
    }

    #[tokio::test]
    async fn resolve_defaults_to_home_configs() {
        let _guard = test_lock().await;
        clear_env();
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(None, &home).unwrap(),
            home.join("configs")
        );
    }

    #[test]
    fn config_scope_accepts_shared_or_owned_roots_and_rejects_siblings() {
        let engine = EngineId::new("mihomo").unwrap();
        let shared = PathBuf::from("/shared/configs");
        let scoped = engine_config_dir(&shared, &engine);
        assert_eq!(scope_config_dir(&shared, &engine).unwrap(), scoped);
        assert_eq!(scope_config_dir(&scoped, &engine).unwrap(), scoped);
        assert_eq!(
            scope_config_dir(&shared.join("engines"), &engine).unwrap(),
            scoped
        );
        assert!(scope_config_dir(&shared.join("engines/sing-box"), &engine).is_err());
    }

    #[tokio::test]
    async fn blank_explicit_falls_back_to_default() {
        let _guard = test_lock().await;
        clear_env();
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(Some("   "), &home).unwrap(),
            home.join("configs")
        );
    }

    #[tokio::test]
    async fn explicit_wins_over_default_and_relative_joins_home() {
        let _guard = test_lock().await;
        clear_env();
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(Some("/abs/cloud"), &home).unwrap(),
            PathBuf::from("/abs/cloud")
        );
        assert_eq!(
            resolve_configs_dir_in(Some("cloud/profiles"), &home).unwrap(),
            home.join("cloud/profiles")
        );
    }

    #[tokio::test]
    async fn env_overrides_explicit() {
        let _guard = test_lock().await;
        set_env("/env/cloud");
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(Some("/settings/cloud"), &home).unwrap(),
            PathBuf::from("/env/cloud")
        );
        clear_env();
    }

    #[tokio::test]
    async fn env_blank_falls_through_to_explicit() {
        let _guard = test_lock().await;
        set_env("   ");
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(Some("/settings/cloud"), &home).unwrap(),
            PathBuf::from("/settings/cloud")
        );
        clear_env();
    }

    #[tokio::test]
    async fn env_relative_joins_home() {
        let _guard = test_lock().await;
        set_env("  cloud-dir  ");
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(None, &home).unwrap(),
            home.join("cloud-dir")
        );
        clear_env();
    }

    #[tokio::test]
    async fn env_blank_only_whitespace_is_ignored() {
        let _guard = test_lock().await;
        set_env("\t ");
        let home = PathBuf::from("/app-home");
        assert_eq!(
            resolve_configs_dir_in(None, &home).unwrap(),
            home.join("configs")
        );
        clear_env();
    }

    #[tokio::test]
    async fn expand_tilde_forms() {
        let home = home_dir().unwrap();
        assert_eq!(expand_tilde("~").unwrap(), home);
        assert_eq!(
            expand_tilde("~/Library/Cloud").unwrap(),
            home.join("Library/Cloud")
        );
        assert_eq!(
            expand_tilde("plain/relative").unwrap(),
            PathBuf::from("plain/relative")
        );
        assert_eq!(
            expand_tilde("/already/absolute").unwrap(),
            PathBuf::from("/already/absolute")
        );
        assert!(expand_tilde("~other-user").is_err());
    }

    #[tokio::test]
    async fn manager_with_home_redirects_the_engine_namespace_and_keeps_app_settings_separate() {
        let _guard = test_lock().await;
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        let cloud = temp_dir.path().join("cloud").join("sync");

        set_env(cloud.to_str().unwrap());
        let manager = manager::ConfigManager::with_home_and_store(home.clone(), TestStore).unwrap();
        clear_env();

        assert_eq!(
            manager.config_root(),
            cloud,
            "the configured directory remains the shared root"
        );
        assert_eq!(
            manager.config_dir(),
            engine_config_dir(manager.config_root(), &crate::engine_id())
        );
        assert_eq!(
            manager.settings_file(),
            manager.config_dir().join("profile-state.toml")
        );
        match manager.profile_store.namespace_initialization() {
            ProfileNamespaceInitialization::LegacyV1(migration) => assert_eq!(
                migration.settings_file(),
                home.join("config.toml").as_path()
            ),
            ProfileNamespaceInitialization::EngineScoped => {
                panic!("Mihomo store must carry the explicit legacy namespace migration")
            }
        }
    }

    #[tokio::test]
    async fn manager_with_configs_dir_uses_settings_field() {
        let _guard = test_lock().await;
        clear_env();
        let temp_dir = TempDir::new().unwrap();
        let manager = manager::ConfigManager::with_home_configs_dir_and_store(
            temp_dir.path().to_path_buf(),
            Some("cloud/profiles"),
            TestStore,
        );
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert_eq!(
            manager.config_dir(),
            engine_config_dir(&temp_dir.path().join("cloud/profiles"), &crate::engine_id())
        );
        assert_eq!(
            manager.settings_file(),
            manager.config_dir().join("profile-state.toml")
        );

        // 空白值等价于未设置。
        let manager = manager::ConfigManager::with_home_configs_dir_and_store(
            temp_dir.path().to_path_buf(),
            Some("  "),
            TestStore,
        )
        .unwrap();
        assert_eq!(
            manager.config_dir(),
            engine_config_dir(&temp_dir.path().join("configs"), &crate::engine_id())
        );
    }

    struct TestStore;

    #[async_trait::async_trait]
    impl SecureStore for TestStore {
        async fn get(
            &self,
            _namespace: &str,
            _key: &str,
        ) -> result::Result<Option<String>, PortError> {
            Ok(None)
        }

        async fn set(
            &self,
            _namespace: &str,
            _key: &str,
            _value: &str,
        ) -> result::Result<(), PortError> {
            Ok(())
        }

        async fn delete(&self, _namespace: &str, _key: &str) -> result::Result<(), PortError> {
            Ok(())
        }
    }
}
