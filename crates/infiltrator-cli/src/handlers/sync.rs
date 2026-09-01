use anyhow::anyhow;
use dav_client::DavClient as _;
use dav_client::client::WebDavClient;
use infiltrator_core::settings::{AppSettings, load_webdav_password};
use mihomo_platform::traits::{CredentialStore, DefaultCredentialStore};
use state_store::StateStore;
use sync_engine::SyncPlanner;
use sync_engine::executor::SyncExecutor;

use crate::commands::SyncAction;
use crate::context::Runtime;
use crate::output::{self, print_info, print_success};

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
pub(crate) fn dav_config(settings: &AppSettings) -> anyhow::Result<(String, String)> {
    let config = &settings.webdav;
    if config.url.trim().is_empty() {
        return Err(anyhow!(
            "WebDAV URL is empty; configure [webdav] in the settings file first"
        ));
    }
    Ok((config.url.clone(), config.username.clone()))
}

/// WebDAV 密码经 core helper 从凭据存储取回（`webdav:password`）。读取
/// 失败归一为空串，由后续服务器认证显式报错，而不是在 CLI 里掩盖配置
/// 问题。
pub(crate) async fn dav_password<S: CredentialStore>(store: &S) -> String {
    load_webdav_password(store).await.unwrap_or_default()
}

async fn dav_client(runtime: &Runtime) -> anyhow::Result<WebDavClient> {
    let (url, username) = dav_config(&runtime.settings)?;
    let password = dav_password(&DefaultCredentialStore::default()).await;
    WebDavClient::new(&url, &username, &password)
}

/// Connectivity check: PROPFIND against the server root.
async fn test(runtime: &Runtime) -> anyhow::Result<()> {
    let client = dav_client(runtime).await?;
    let entries = client.list("/").await?;
    print_success(&format!(
        "WebDAV server '{}' is reachable ({} top-level entries)",
        runtime.settings.webdav.url,
        entries.len(),
    ));
    Ok(())
}

/// One full sync round over the configs directory, replicating the admin
/// scheduler's minimal path: DavClient -> SyncPlanner -> SyncExecutor ->
/// StateStore.
async fn now(runtime: &Runtime) -> anyhow::Result<()> {
    if !runtime.settings.webdav.enabled {
        return Err(anyhow!(
            "WebDAV sync is disabled (webdav.enabled = false in the settings file)"
        ));
    }
    let client = dav_client(runtime).await?;

    let local_root = runtime.configs_dir()?;
    tokio::fs::create_dir_all(&local_root).await?;
    let db_path = runtime.home().join("sync_state.db");
    let store = StateStore::new(&db_path.to_string_lossy()).await?;

    print_info("Building sync plan...");
    let planner = SyncPlanner::new(local_root, "/".to_string(), &client, &store);
    let actions = planner.build_plan().await?;
    let total = actions.len();
    if total == 0 {
        print_info("No sync actions needed; local and remote already match");
        return Ok(());
    }

    let executor = SyncExecutor::new(&client, &store);
    let mut summary = SyncSummary {
        total,
        ..SyncSummary::default()
    };
    for action in actions {
        match executor.execute(action).await {
            Ok(()) => summary.succeeded += 1,
            Err(err) => {
                summary.failed += 1;
                output::print_error(&format!("sync action failed: {err:#}"));
            }
        }
    }
    println!("{}", render_summary(&summary));
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SyncSummary {
    total: usize,
    succeeded: usize,
    failed: usize,
}

pub(crate) fn render_summary(summary: &SyncSummary) -> String {
    format!(
        "sync: {} planned, {} succeeded, {} failed",
        summary.total, summary.succeeded, summary.failed
    )
}

#[cfg(test)]
mod tests {
    use infiltrator_core::settings::{
        AppSettings, WebDavConfig, clear_webdav_password, load_settings, save_settings,
        save_webdav_password,
    };
    use mihomo_platform::traits::CredentialStore;

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
    impl CredentialStore for MemoryStore {
        async fn get(
            &self,
            service: &str,
            key: &str,
        ) -> mihomo_api::error::Result<Option<String>> {
            Ok(self.peek(service, key))
        }

        async fn set(&self, service: &str, key: &str, value: &str) -> mihomo_api::error::Result<()> {
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(&self, service: &str, key: &str) -> mihomo_api::error::Result<()> {
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
