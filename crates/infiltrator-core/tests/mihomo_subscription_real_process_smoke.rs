//! Real-process subscription application evidence for locked Mihomo binaries.
//!
//! The Mihomo binary is selected and SHA-verified by the explicit smoke
//! script. Subscription HTTP is restricted to a mock server bound to loopback;
//! this test never contacts a public subscription service.

use async_trait::async_trait;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::subscription_import_application::SubscriptionImportApplication;
use infiltrator_application::subscription_refresh_application::SubscriptionRefreshApplication;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::subscription_import::{
    SubscriptionContentEffectOutcome, SubscriptionImportChannel, SubscriptionUpdateOutcome,
};
use infiltrator_core::subscription_io::HttpSubscriptionSource;
use infiltrator_domain::subscription_scheduler_policy::RetryBackoffPolicy;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_ports::subscription_import::SubscriptionImportPort;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use mockito::Matcher;
use std::collections::HashMap;
use std::env;
use std::future::ready;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const PROFILE_NAME: &str = "CORE-SMOKE-SUB";

struct SmokeSecureStore(Mutex<HashMap<(String, String), String>>);

impl Default for SmokeSecureStore {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[async_trait]
impl SecureStore for SmokeSecureStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .0
            .lock()
            .await
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .await
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .await
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

struct LoopbackOnlyImportHost;

#[async_trait]
impl SubscriptionImportPort for LoopbackOnlyImportHost {
    async fn read_local_file(&self, _path: &str) -> Result<String, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this smoke only authorizes URL import through its loopback fixture",
        ))
    }

    async fn read_clipboard(&self) -> Result<String, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this smoke has no clipboard integration",
        ))
    }
}

struct ImmediateApplicationRuntime;

impl ApplicationRuntime for ImmediateApplicationRuntime {
    fn block_on(&self, _future: ApplicationFuture) {
        panic!("the subscription evidence path does not schedule background work");
    }

    fn sleep(&self, _duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(ready(()))
    }
}

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

fn subscription_document(revision: &str, log_level: &str) -> String {
    format!(
        "port: 0\nsocks-port: 0\nredir-port: 0\ntproxy-port: 0\nmixed-port: 0\nallow-lan: false\nmode: rule\nlog-level: {log_level}\nipv6: false\nproxies:\n  - name: CORE-SMOKE-SOCKS\n    type: socks5\n    server: 127.0.0.1\n    port: 1\nproxy-groups: []\nrules:\n  - DOMAIN,subscription-{revision}.smoke.test,DIRECT\n  - MATCH,DIRECT\n"
    )
}

fn native_check_candidate(binary: &Path, configs_root: &Path, candidate_name: &str, bytes: &[u8]) {
    let candidate_path = configs_root.join(candidate_name);
    std::fs::write(&candidate_path, bytes)
        .expect("write isolated subscription candidate for native validation");
    let native_check = Command::new(binary)
        .arg("-t")
        .arg("-d")
        .arg(configs_root)
        .arg("-f")
        .arg(&candidate_path)
        .output()
        .expect("run the exact SHA-locked Mihomo native config check");
    let _ = std::fs::remove_file(&candidate_path);
    assert!(
        native_check.status.success(),
        "Mihomo native check rejected {candidate_name}: stdout={} stderr={}",
        String::from_utf8_lossy(&native_check.stdout),
        String::from_utf8_lossy(&native_check.stderr),
    );
}

async fn inspect_and_check_workspace(
    profiles: &ProfileApplication,
    codec: &MihomoProfileDocumentCodec,
    binary: &Path,
    configs_root: &Path,
    candidate_name: &str,
    expected_rule: &str,
) -> Vec<u8> {
    let workspace = profiles
        .load_workspace(&ProfileId::new(PROFILE_NAME).expect("valid profile ID"))
        .await
        .expect("load the exact subscription workspace from the shared engine store");
    assert_eq!(workspace.document.identity.engine_id.as_str(), "mihomo");
    assert_eq!(
        workspace.document.identity.profile_id.as_str(),
        PROFILE_NAME
    );
    assert_eq!(
        codec
            .inspect(&workspace.document.raw_bytes)
            .expect("inspect Mihomo YAML"),
        DocumentValidation::Valid,
        "the typed Mihomo codec accepts the persisted workspace"
    );
    let content = std::str::from_utf8(&workspace.document.raw_bytes).expect("UTF-8 profile");
    assert!(content.contains(expected_rule));
    native_check_candidate(
        binary,
        configs_root,
        candidate_name,
        &workspace.document.raw_bytes,
    );
    workspace.document.raw_bytes
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn subscription_import_update_and_refresh_share_the_mihomo_profile_store() {
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let binary = PathBuf::from(required_environment("MIHOMO_REAL_BINARY"));
    let configs_root = PathBuf::from(required_environment("MIHOMO_REAL_CONFIGS_ROOT"));
    let binary_version = Command::new(&binary)
        .arg("-v")
        .output()
        .expect("read the SHA-locked Mihomo binary version");
    assert!(
        binary_version.status.success(),
        "Mihomo -v exited unsuccessfully"
    );
    let binary_version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&binary_version.stdout),
        String::from_utf8_lossy(&binary_version.stderr)
    );
    assert!(
        binary_version_text.contains(&expected_version),
        "locked binary version output did not contain {expected_version}: {binary_version_text}"
    );

    let mut server = mockito::Server::new_async().await;
    let url = format!("{}/sub.yaml", server.url());
    let parsed_url = url::Url::parse(&url).expect("fixture URL parses");
    assert!(
        parsed_url
            .host_str()
            .and_then(|host| host.parse::<std::net::IpAddr>().ok())
            .is_some_and(|address| address.is_loopback())
    );

    let import_body = subscription_document("v1", "error");
    let update_body = subscription_document("v2", "warning");
    let refresh_body = subscription_document("v3", "info");
    let import_mock = server
        .mock("GET", "/sub.yaml")
        .match_header("if-none-match", Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/yaml")
        .with_header("etag", "\"subscription-v1\"")
        .with_body(import_body)
        .expect(1)
        .create_async()
        .await;
    let update_mock = server
        .mock("GET", "/sub.yaml")
        .match_header("if-none-match", Matcher::Missing)
        .with_status(200)
        .with_header("content-type", "application/yaml")
        .with_header("etag", "\"subscription-v2\"")
        .with_body(update_body)
        .expect(1)
        .create_async()
        .await;
    let refresh_mock = server
        .mock("GET", "/sub.yaml")
        .match_header("if-none-match", "\"subscription-v2\"")
        .with_status(200)
        .with_header("content-type", "application/yaml")
        .with_header("etag", "\"subscription-v3\"")
        .with_body(refresh_body)
        .expect(1)
        .create_async()
        .await;

    let client = infiltrator_http::reqwest::Client::builder()
        .user_agent("MusicFrog-Despicable-Infiltrator")
        .no_proxy()
        .build()
        .expect("build a no-proxy subscription client");
    let raw_client = infiltrator_http::reqwest::Client::builder()
        .user_agent("MusicFrog-Despicable-Infiltrator")
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .build()
        .expect("build a no-proxy raw subscription client");
    let source = HttpSubscriptionSource::new(&client, &raw_client);

    let home = tempfile::tempdir().expect("isolated subscription settings home");
    let configs_root_text = configs_root
        .to_str()
        .expect("smoke configs root is a UTF-8 path");
    let config = ConfigManager::with_home_configs_dir_and_store(
        home.path().to_path_buf(),
        Some(configs_root_text),
        SmokeSecureStore::default(),
    )
    .expect("construct the Mihomo engine-scoped profile store");
    let store: Arc<dyn ProfileStore> = config.shared_engine_profile_store();
    let codec = MihomoProfileDocumentCodec;
    let profiles = ProfileApplication::new(store).with_document_codec(Arc::new(codec));
    assert_eq!(profiles.engine_id().as_str(), "mihomo");
    assert_eq!(profiles.config_dir(), config.config_dir());

    let import =
        SubscriptionImportApplication::new(profiles.clone(), Arc::new(LoopbackOnlyImportHost));
    let imported = import
        .import(&source, PROFILE_NAME, SubscriptionImportChannel::Url, &url)
        .await
        .expect("ImportSubscription uses production source and shared profile application");
    assert_eq!(imported.profile_name, PROFILE_NAME);
    assert_eq!(imported.channel, SubscriptionImportChannel::Url);
    assert_eq!(
        imported.format,
        infiltrator_contract::subscription_import::SubscriptionFormat::ClashYaml
    );
    assert_eq!(imported.node_count, 1);
    assert!(imported.content_bytes > 0);
    assert_eq!(
        profiles
            .load_profile_info(PROFILE_NAME)
            .await
            .expect("imported profile info")
            .subscription_url,
        Some(url.clone())
    );
    let imported_bytes = inspect_and_check_workspace(
        &profiles,
        &codec,
        &binary,
        &configs_root,
        "subscription-import-candidate.yaml",
        "subscription-v1.smoke.test",
    )
    .await;
    assert_eq!(
        imported_bytes,
        subscription_document("v1", "error").into_bytes()
    );

    let refresh = SubscriptionRefreshApplication::new(
        profiles.clone(),
        Arc::new(ImmediateApplicationRuntime),
        RetryBackoffPolicy::test_immediate(),
    );
    let updated = refresh
        .refresh_profile(&source, PROFILE_NAME)
        .await
        .expect("UpdateProfile uses the shared refresh application");
    assert_eq!(updated.profile_name, PROFILE_NAME);
    assert!(matches!(
        updated.outcome,
        SubscriptionUpdateOutcome::Updated { .. }
    ));
    assert_eq!(updated.etag.as_deref(), Some("\"subscription-v2\""));
    assert_eq!(
        updated.content_effect,
        SubscriptionContentEffectOutcome::NotActive
    );
    let updated_bytes = inspect_and_check_workspace(
        &profiles,
        &codec,
        &binary,
        &configs_root,
        "subscription-update-candidate.yaml",
        "subscription-v2.smoke.test",
    )
    .await;
    assert_ne!(updated_bytes, imported_bytes);

    let refreshed = refresh
        .refresh_all(&source, 1)
        .await
        .expect("UpdateAllSubscriptions uses the shared batch refresh application");
    assert!(refreshed.total >= 1);
    assert_eq!(refreshed.total - refreshed.skipped, 1);
    assert_eq!(refreshed.updated, 1);
    assert_eq!(refreshed.not_modified, 0);
    assert_eq!(refreshed.failed, 0);
    assert_eq!(refreshed.outcomes.len(), 1);
    assert_eq!(refreshed.outcomes[0].profile_name, PROFILE_NAME);
    assert!(matches!(
        refreshed.outcomes[0].outcome,
        SubscriptionUpdateOutcome::Updated { .. }
    ));
    assert_eq!(
        refreshed.outcomes[0].etag.as_deref(),
        Some("\"subscription-v3\"")
    );
    let refreshed_bytes = inspect_and_check_workspace(
        &profiles,
        &codec,
        &binary,
        &configs_root,
        "subscription-refresh-candidate.yaml",
        "subscription-v3.smoke.test",
    )
    .await;
    assert_ne!(refreshed_bytes, updated_bytes);

    import_mock.assert_async().await;
    update_mock.assert_async().await;
    refresh_mock.assert_async().await;
    assert_eq!(
        profiles
            .load_profile_info(PROFILE_NAME)
            .await
            .expect("refreshed profile metadata")
            .subscription_url,
        Some(url)
    );
}
