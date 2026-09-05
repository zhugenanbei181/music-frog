use anyhow::anyhow;
#[cfg(test)]
use infiltrator_core::settings_io::load_webdav_password;
#[cfg(test)]
use infiltrator_domain::settings::AppSettings;
#[cfg(test)]
use infiltrator_ports::secure_store::SecureStore;

use crate::commands::SyncAction;
use crate::context::Runtime;
use crate::output::{print_info, print_success};

pub(crate) async fn handle(action: SyncAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    match action {
        SyncAction::Test => test(&runtime).await?,
        SyncAction::Now => now(&runtime).await?,
    }
    Ok(())
}

/// WebDAV 端点与用户名（不含密码）。core 迁移后 settings.toml 不再携带
/// 明文密码，密码一律经 [`dav_password`] 从 OS keyring 取回。
#[cfg(test)]
pub(crate) fn dav_config(settings: &AppSettings) -> anyhow::Result<(String, String)> {
    let config = &settings.webdav;
    if config.url.trim().is_empty() {
        return Err(anyhow!(
            "WebDAV URL is empty; configure [webdav] in the settings file first"
        ));
    }
    Ok((config.url.clone(), config.username.clone()))
}

/// WebDAV 密码 helper 只保留给隔离测试；生产 CLI 通过 settings application
/// 的 hydrated store 读取（`webdav:password`）。读取
/// 失败归一为空串，由后续服务器认证显式报错，而不是在 CLI 里掩盖配置
/// 问题。
#[cfg(test)]
pub(crate) async fn dav_password<S: SecureStore>(store: &S) -> String {
    load_webdav_password(store).await.unwrap_or_default()
}

/// Connectivity check: PROPFIND against the server root.
async fn test(runtime: &Runtime) -> anyhow::Result<()> {
    let config = runtime.settings.webdav.clone();
    let entries = runtime
        .sync_application()?
        .test(config)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    print_success(&format!(
        "WebDAV server '{}' is reachable ({} top-level entries)",
        runtime.settings.webdav.url,
        entries,
    ));
    Ok(())
}

/// One full sync round over the configs directory through the shared
/// application facade.
async fn now(runtime: &Runtime) -> anyhow::Result<()> {
    if !runtime.settings.webdav.enabled {
        return Err(anyhow!(
            "WebDAV sync is disabled (webdav.enabled = false in the settings file)"
        ));
    }
    print_info("Building sync plan...");
    let report = runtime
        .sync_application()?
        .sync(
            runtime.settings.webdav.clone(),
            runtime.settings.configs_dir.clone(),
        )
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    if report.total_actions == 0 {
        print_info("No sync actions needed; local and remote already match");
        return Ok(());
    }
    println!(
        "sync: {} planned, {} succeeded, {} failed",
        report.total_actions, report.success_count, report.failed_count
    );
    Ok(())
}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SyncSummary {
    total: usize,
    succeeded: usize,
    failed: usize,
}

#[cfg(test)]
pub(crate) fn render_summary(summary: &SyncSummary) -> String {
    format!(
        "sync: {} planned, {} succeeded, {} failed",
        summary.total, summary.succeeded, summary.failed
    )
}

#[cfg(test)]
mod tests {
    use infiltrator_core::settings_io::{
        clear_webdav_password, load_settings, save_settings, save_webdav_password,
    };
    use infiltrator_domain::settings::{AppSettings, WebDavConfig};
    use infiltrator_ports::secure_store::SecureStore;

    use super::{SyncSummary, dav_config, dav_password, render_summary};

    fn settings_with(config: WebDavConfig) -> AppSettings {
        AppSettings {
            webdav: config,
            ..AppSettings::default()
        }
    }

    #[test]
    fn dav_config_rejects_an_empty_url() {
        let settings = settings_with(WebDavConfig::default());
        assert!(dav_config(&settings).is_err());

        let settings = settings_with(WebDavConfig {
            url: "   ".to_string(),
            ..WebDavConfig::default()
        });
        assert!(dav_config(&settings).is_err());
    }

    #[test]
    fn dav_config_returns_url_and_username() {
        let settings = settings_with(WebDavConfig {
            url: "https://dav.example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            ..WebDavConfig::default()
        });
        let (url, username) = dav_config(&settings).unwrap();
        assert_eq!(url, "https://dav.example.com");
        assert_eq!(username, "user");
    }

    #[test]
    fn summary_renders_all_counts() {
        let summary = SyncSummary {
            total: 3,
            succeeded: 2,
            failed: 1,
        };
        let text = render_summary(&summary);
        assert!(text.contains("3 planned"), "{text}");
        assert!(text.contains("2 succeeded"), "{text}");
        assert!(text.contains("1 failed"), "{text}");
    }

    /// 内存凭据存储（仿 core settings.rs 的 MemoryStore 先例），避免测试
    /// 触碰真实 OS keyring。
    struct MemoryStore {
        entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl Default for MemoryStore {
        fn default() -> Self {
            Self {
                entries: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl MemoryStore {
        fn peek(&self, service: &str, key: &str) -> Option<String> {
            self.entries
                .lock()
                .expect("store lock")
                .get(&format!("{service}/{key}"))
                .cloned()
        }
    }

    #[async_trait::async_trait]
    impl SecureStore for MemoryStore {
        async fn get(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<Option<String>, infiltrator_ports::error::PortError> {
            Ok(self.peek(service, key))
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(
            &self,
            service: &str,
            key: &str,
        ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .remove(&format!("{service}/{key}"));
            Ok(())
        }
    }

    /// 密码经 helper 往返（keyring 写入/读回），settings 文件全程无明文：
    /// 即便内存镜像里填了密码，save_settings 的 TOML 输出也不含它，重新
    /// 加载得到的镜像为空串（CLI 的密码来源只有 keyring 一条路）。
    #[tokio::test]
    async fn webdav_password_roundtrips_via_helper_and_settings_file_stays_clean() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("settings.toml");

        let settings = settings_with(WebDavConfig {
            url: "https://dav.example.com".to_string(),
            username: "user".to_string(),
            password: "s3cret".to_string(),
            ..WebDavConfig::default()
        });
        save_settings(&file, &settings).await.unwrap();
        let raw = std::fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("password"), "plaintext leaked: {raw}");
        assert!(!raw.contains("s3cret"), "plaintext leaked: {raw}");

        let store = MemoryStore::default();
        save_webdav_password(&store, "s3cret").await.unwrap();
        assert_eq!(dav_password(&store).await, "s3cret");

        // settings 文件重新加载后密码镜像仍为空串：CLI 取密码只能走 keyring。
        let loaded = load_settings(&file).await.unwrap();
        assert_eq!(loaded.webdav.password, "");
        assert_eq!(loaded.webdav.url, "https://dav.example.com");

        clear_webdav_password(&store).await;
        assert_eq!(dav_password(&store).await, "");
    }
}
