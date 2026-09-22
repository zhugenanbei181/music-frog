//! DUAL-07-01: headless tests for the shared multi-channel import dispatch.

use super::*;
use async_trait::async_trait;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
struct FakeStore {
    saved: Mutex<BTreeMap<String, String>>,
    metadata: Mutex<BTreeMap<String, ProfileMetadata>>,
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        Ok(self
            .saved
            .lock()
            .expect("saved")
            .keys()
            .map(|name| ProfileInfo {
                name: name.clone(),
                path: format!("/fake/configs/{name}.yaml"),
                ..ProfileInfo::default()
            })
            .collect())
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(String::new())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.saved
            .lock()
            .expect("saved")
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.saved
            .lock()
            .expect("saved")
            .insert(profile.to_string(), content.to_string());
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(self
            .metadata
            .lock()
            .expect("metadata")
            .get(profile)
            .cloned()
            .unwrap_or_default())
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.metadata
            .lock()
            .expect("metadata")
            .insert(profile.to_string(), metadata.clone());
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load_options(&self, _profile: &str) -> Result<ProfileOptions, PortError> {
        Ok(ProfileOptions::default())
    }

    async fn save_options(
        &self,
        _profile: &str,
        _options: &ProfileOptions,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete_options(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

struct FakeSource {
    content: String,
    fetches: Mutex<usize>,
}

#[async_trait]
impl SubscriptionSource for FakeSource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        *self.fetches.lock().expect("fetches") += 1;
        Ok(SubscriptionDocument {
            content: self.content.clone(),
            userinfo: None,
        })
    }

    async fn fetch_conditional(
        &self,
        profile: &str,
        url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
        _headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        let document = self.fetch(profile, url).await?;
        Ok(ConditionalDocumentResult::Modified {
            document,
            etag: None,
            last_modified: None,
        })
    }
}

struct FakeImportPort {
    file: Result<String, PortError>,
    clipboard: Result<String, PortError>,
}

#[async_trait]
impl SubscriptionImportPort for FakeImportPort {
    async fn read_local_file(&self, _path: &str) -> Result<String, PortError> {
        self.file.clone()
    }

    async fn read_clipboard(&self) -> Result<String, PortError> {
        self.clipboard.clone()
    }
}

const CLASH_DOC: &str =
    "proxies:\n  - name: node1\n    type: ss\n  - name: node2\n    type: vmess\n";

fn application(port: FakeImportPort) -> SubscriptionImportApplication {
    SubscriptionImportApplication::new(
        ProfileApplication::new(Arc::new(FakeStore::default())),
        Arc::new(port),
    )
}

#[tokio::test]
async fn local_file_channel_reads_through_the_host_port() {
    let app = application(FakeImportPort {
        file: Ok(CLASH_DOC.to_string()),
        clipboard: Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "no clipboard",
        )),
    });
    let source = FakeSource {
        content: String::new(),
        fetches: Mutex::new(0),
    };
    let report = app
        .import(
            &source,
            "main",
            SubscriptionImportChannel::LocalFile,
            "/tmp/main.yaml",
        )
        .await
        .expect("import");
    assert_eq!(report.channel, SubscriptionImportChannel::LocalFile);
    assert_eq!(
        report.format,
        infiltrator_contract::subscription_import::SubscriptionFormat::ClashYaml
    );
    assert_eq!(report.node_count, 2);
}

#[tokio::test]
async fn clipboard_url_is_fetched_through_the_shared_source() {
    let app = application(FakeImportPort {
        file: Ok(String::new()),
        clipboard: Ok("https://example.com/sub.yaml".to_string()),
    });
    let source = FakeSource {
        content: CLASH_DOC.to_string(),
        fetches: Mutex::new(0),
    };
    let report = app
        .import(
            &source,
            "from-clip",
            SubscriptionImportChannel::Clipboard,
            "",
        )
        .await
        .expect("import");
    assert_eq!(report.channel, SubscriptionImportChannel::Url);
    assert_eq!(report.node_count, 2);
}

#[tokio::test]
async fn unsupported_clipboard_is_a_typed_failure_not_a_fake_success() {
    let app = application(FakeImportPort {
        file: Ok(String::new()),
        clipboard: Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "this host has no clipboard",
        )),
    });
    let source = FakeSource {
        content: String::new(),
        fetches: Mutex::new(0),
    };
    let failure = app
        .import(&source, "x", SubscriptionImportChannel::Clipboard, "")
        .await
        .expect_err("unsupported clipboard must fail");
    assert_eq!(
        failure.code,
        infiltrator_contract::error::ErrorCode::Unsupported
    );
}
