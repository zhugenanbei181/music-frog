//! Profile path safety: name validation, character-level sanitization, and
//! canonicalized path construction that cannot escape the config directory.
//! Also resolves the configs storage directory (cloud-sync redirect).

use std::path::{Path, PathBuf};

use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::paths::get_home_dir;
use mihomo_platform::traits::CredentialStore;
use tokio::fs;

use super::ConfigManager;

/// 环境变量形式的 configs 目录覆盖，优先级高于 settings 的 `configs_dir`。
pub const CONFIGS_DIR_ENV: &str = "INFILTRATOR_CONFIGS_DIR";

/// 解析 configs（profiles yaml）存储目录。
/// 优先级：[`CONFIGS_DIR_ENV`] > `explicit`（settings 的 `configs_dir`）> `<home>/configs`。
/// 空串/纯空白视为未设置并落到下一优先级；前导 `~` 展开为用户主目录；
/// 相对路径按 home 拼接（与默认值 `<home>/configs` 同一基准）。
/// 只解析路径：不创建目录、不要求目录存在（目录创建归 doctor fix 与既有 save 流程）。
pub fn resolve_configs_dir(explicit: Option<&str>) -> Result<PathBuf> {
    resolve_configs_dir_in(explicit, &get_home_dir()?)
}

/// 同 [`resolve_configs_dir`]，但 home 由调用方提供（`with_home*` 构造路径）。
pub fn resolve_configs_dir_in(explicit: Option<&str>, home: &Path) -> Result<PathBuf> {
    let env_value = std::env::var(CONFIGS_DIR_ENV).ok();
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
    let home = std::env::home_dir().ok_or_else(|| {
        MihomoError::Config("could not determine user home for '~' expansion".to_string())
    })?;
    Ok(if rest.is_empty() {
        home
    } else {
        home.join(rest)
    })
}

impl<S: CredentialStore> ConfigManager<S> {
    /// 以规范化的 config 目录为根构造 profile 的 yaml 路径：
    /// 先校验、再做字符级消毒，最后验证结果仍落在规范基目录内。
    pub(super) async fn profile_yaml_path(&self, profile: &str) -> Result<PathBuf> {
        let safe = sanitized_profile_key(profile)?;
        fs::create_dir_all(&self.config_dir).await?;
        let base = fs::canonicalize(&self.config_dir)
            .await
            .map_err(|err| MihomoError::Config(format!("config dir unavailable: {err}")))?;
        let path = base.join(format!("{safe}.yaml"));
        if !path.starts_with(&base) {
            return Err(MihomoError::Config(
                "profile path escapes config dir".to_string(),
            ));
        }
        Ok(path)
    }

    /// 同 [`Self::profile_yaml_path`]，但要求文件已存在并返回其规范化路径。
    pub(super) async fn existing_profile_yaml_path(&self, profile: &str) -> Result<PathBuf> {
        // 目录不存在则任何 profile 都不可能存在，直接走 NotFound 语义。
        let base = match fs::canonicalize(&self.config_dir).await {
            Ok(base) => base,
            Err(_) => {
                return Err(MihomoError::NotFound(format!(
                    "Profile '{profile}' not found"
                )));
            }
        };
        let path = self.profile_yaml_path(profile).await?;
        let canonical = fs::canonicalize(&path)
            .await
            .map_err(|_| MihomoError::NotFound(format!("Profile '{profile}' not found")))?;
        if !canonical.starts_with(&base) {
            return Err(MihomoError::Config(
                "profile path escapes config dir".to_string(),
            ));
        }
        Ok(canonical)
    }
}

/// Profile names end up as filesystem paths (`<config_dir>/<name>.yaml`) and
/// as credential/TOML keys, and can arrive from the admin HTTP API. Reject
/// anything that could escape `config_dir` or break cross-platform file
/// semantics before it reaches a path join.
pub fn validate_profile_name(profile: &str) -> Result<()> {
    let rejected = |reason: &str| {
        Err(MihomoError::Config(format!(
            "Invalid profile name: {reason}"
        )))
    };
    if profile.is_empty() {
        return rejected("name is empty");
    }
    if profile.chars().count() > 128 {
        return rejected("name exceeds 128 characters");
    }
    if profile.trim() != profile {
        return rejected("leading or trailing whitespace");
    }
    if profile == "." || profile == ".." || profile.contains("..") {
        return rejected("relative path segments are not allowed");
    }
    if let Some(ch) = profile
        .chars()
        .find(|ch| matches!(ch, '/' | '\\' | ':') || ch.is_control())
    {
        return rejected(format!("illegal character {ch:?}").as_str());
    }
    if profile.ends_with('.') || profile.ends_with(' ') {
        return rejected("trailing dot or space (Windows filename semantics)");
    }
    Ok(())
}

/// 校验之后再做一次字符级消毒：即使校验被绕过，分隔符也会被替换为下划线。
/// 对合法名字这是恒等变换（key 与原名一致）。
pub(super) fn sanitized_profile_key(profile: &str) -> Result<String> {
    validate_profile_name(profile)?;
    Ok(profile.replace(['/', '\\', ':'], "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mihomo_platform::TEST_LOCK;
    use tempfile::TempDir;

    // 环境变量是进程级全局状态：所有涉及 configs 目录解析的测试都必须持有
    // TEST_LOCK 串行执行，避免互相串扰（与仓库既有 env 测试做法一致）。
    async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn set_env(value: &str) {
        unsafe { std::env::set_var(CONFIGS_DIR_ENV, value) };
    }

    fn clear_env() {
        unsafe { std::env::remove_var(CONFIGS_DIR_ENV) };
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
        let home = std::env::home_dir().unwrap();
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
    async fn manager_with_home_redirects_configs_but_keeps_settings_file() {
        let _guard = test_lock().await;
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().to_path_buf();
        let cloud = temp_dir.path().join("cloud").join("sync");

        set_env(cloud.to_str().unwrap());
        let manager = crate::manager::ConfigManager::with_home(home.clone()).unwrap();
        clear_env();

        assert_eq!(manager.config_dir, cloud);
        // settings 文件（当前 profile 指针）不跟随云同步目录。
        assert_eq!(manager.settings_file, home.join("config.toml"));
    }

    #[tokio::test]
    async fn manager_with_configs_dir_uses_settings_field() {
        let _guard = test_lock().await;
        clear_env();
        let temp_dir = TempDir::new().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = crate::manager::ConfigManager::with_configs_dir(Some("cloud/profiles"));
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert_eq!(manager.config_dir, temp_dir.path().join("cloud/profiles"));
        assert_eq!(manager.settings_file, temp_dir.path().join("config.toml"));

        // 空白值等价于未设置。
        let manager = crate::manager::ConfigManager::with_configs_dir(Some("  ")).unwrap();
        assert_eq!(manager.config_dir, temp_dir.path().join("configs"));

        mihomo_platform::paths::clear_home_dir_override();
    }
}
