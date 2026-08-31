use anyhow::anyhow;
use dav_client::DavClient as _;
use dav_client::client::WebDavClient;
use infiltrator_core::settings::AppSettings;
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

pub(crate) fn dav_config(settings: &AppSettings) -> anyhow::Result<(String, String, String)> {
    let config = &settings.webdav;
    if config.url.trim().is_empty() {
        return Err(anyhow!(
            "WebDAV URL is empty; configure [webdav] in the settings file first"
        ));
    }
    Ok((
        config.url.clone(),
        config.username.clone(),
        config.password.clone(),
    ))
}

fn dav_client(runtime: &Runtime) -> anyhow::Result<WebDavClient> {
    let (url, username, password) = dav_config(&runtime.settings)?;
    WebDavClient::new(&url, &username, &password)
}

/// Connectivity check: PROPFIND against the server root.
async fn test(runtime: &Runtime) -> anyhow::Result<()> {
    let client = dav_client(runtime)?;
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
    let client = dav_client(runtime)?;

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
    use infiltrator_core::settings::{AppSettings, WebDavConfig};

    use super::{dav_config, render_summary, SyncSummary};

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
    fn dav_config_returns_url_and_credentials() {
        let settings = settings_with(WebDavConfig {
            url: "https://dav.example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            ..WebDavConfig::default()
        });
        let (url, username, password) = dav_config(&settings).unwrap();
        assert_eq!(url, "https://dav.example.com");
        assert_eq!(username, "user");
        assert_eq!(password, "pass");
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
}
