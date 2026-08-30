//! Profile path safety: name validation, character-level sanitization, and
//! canonicalized path construction that cannot escape the config directory.

use std::path::PathBuf;

use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::traits::CredentialStore;
use tokio::fs;

use super::ConfigManager;

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
