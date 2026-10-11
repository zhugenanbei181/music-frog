//! Headless tests for the shared rule-edit command path (DUAL-11-09/10/11/12)
//! and the DUAL-07-09/14 subscription settings commands.
//!
//! A minimal in-memory `ProfileStore` proves the application applies the same
//! list arithmetic as `infiltrator_domain::rules::edit` and persists the
//! result, without a real profile directory.

use super::*;
use crate::command_application::CommandApplication;
use crate::command_application::handler::ValidatedCoreCommandScope;
use crate::configuration_application::ConfigurationApplication;
use crate::core_capability_test_support::{core_scope, with_test_capabilities};
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use crate::profile_workspace_test_support::{workspace_with_origin, workspace_with_raw_sidecar};
use async_trait::async_trait;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::provider_cache::{ProviderCachePurge, RuleProviderCacheSnapshot};
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_contract::subscription_import::SubscriptionScheduleDraft;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::rules::edit::build_custom_rule;
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;
use infiltrator_domain::rules::{game_routing_presets, load_rules_from_yaml};
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::rule_provider_cache::{
    ProviderCacheEntry, ProviderFileFact, RuleProviderCachePort,
};
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct FakeStore {
    profile: Mutex<String>,
    content: Mutex<String>,
    metadata: Mutex<ProfileMetadata>,
    workspace_reads: AtomicUsize,
    workspace_writes: AtomicUsize,
}

impl FakeStore {
    fn with_profile(content: &str) -> Self {
        Self {
            profile: Mutex::new("main".to_owned()),
            content: Mutex::new(content.to_owned()),
            metadata: Mutex::new(ProfileMetadata::default()),
            ..Self::default()
        }
    }

    fn content(&self) -> String {
        self.content.lock().expect("content lock").clone()
    }

    fn metadata(&self) -> ProfileMetadata {
        self.metadata.lock().expect("metadata lock").clone()
    }
}

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        let name = self.profile.lock().expect("profile lock").clone();
        Ok(vec![ProfileInfo {
            name,
            active: true,
            path: "/fake/configs/main.yaml".to_owned(),
            ..ProfileInfo::default()
        }])
    }

    async fn get_current(&self) -> Result<String, PortError> {
        Ok(self.profile.lock().expect("profile lock").clone())
    }

    async fn set_current(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn load(&self, _profile: &str) -> Result<String, PortError> {
        Ok(self.content())
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        let content = self.content();
        let mut workspace =
            workspace_with_origin(profile.as_str(), content.as_bytes(), None, None)?;
        workspace.write_protection = ProfileWriteProtection::Editable;
        Ok(workspace)
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = self.profile.lock().expect("profile lock").clone();
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.load_workspace(&profile).await
    }

    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.workspace_writes.fetch_add(1, Ordering::SeqCst);
        let mut content = self.content.lock().unwrap();
        let metadata = self.metadata.lock().unwrap();
        let current = workspace_with_origin(
            expected.document.profile_id.as_str(),
            content.as_bytes(),
            None,
            None,
        )?;
        if current.identity() != *expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "source changed",
                true,
            )));
        }
        let protection = ProfileWriteProtection::from_subscription_url(
            metadata.subscription_url.as_deref().unwrap_or_default(),
        );
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                authorization: ProfileWriteAuthorization::RequireUnlock
            }
        ) && protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "protected edit",
                false,
            )));
        }
        assert!(
            update.sidecar_bytes.is_none(),
            "this fixture has no sidecar"
        );
        *content = String::from_utf8(update.document_bytes.clone())
            .map_err(|error| PortError::Io(error.to_string()))?;
        let mut committed = workspace_with_raw_sidecar(
            expected.document.profile_id.as_str(),
            update.document_bytes.as_slice(),
            update.sidecar_bytes.clone(),
            None,
        )?;
        committed.write_protection = protection;
        Ok(committed)
    }

    async fn compare_and_save_active_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        if self.profile.lock().expect("profile lock").as_str()
            != expected.document.profile_id.as_str()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile changed",
                true,
            )));
        }
        self.compare_and_save_workspace(expected, update).await
    }

    async fn save(&self, _profile: &str, content: &str) -> Result<(), PortError> {
        *self.content.lock().expect("content lock") = content.to_owned();
        Ok(())
    }

    async fn delete_profile(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn get_profile_metadata(&self, _profile: &str) -> Result<ProfileMetadata, PortError> {
        Ok(self.metadata())
    }

    async fn update_profile_metadata(
        &self,
        _profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        *self.metadata.lock().expect("metadata lock") = metadata.clone();
        Ok(())
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn restore_backup(&self, _profile: &str) -> Result<bool, PortError> {
        Ok(false)
    }
}

const THREE_RULES: &str =
    "rules:\n  - DOMAIN,a.com,DIRECT\n  - DOMAIN,b.com,PROXY\n  - MATCH,DIRECT\n";

/// Runtime fixture records the operations under test; unrelated port methods
/// supply only the minimal isolated observations needed by those use cases.
#[derive(Default)]
pub(super) struct RecordingGeoGateway {
    geo_upgrades: AtomicUsize,
    probe_calls: Mutex<Vec<(String, String, u32)>>,
    switches: AtomicUsize,
    deny_probe: AtomicBool,
    denied_probe_names: Mutex<Vec<String>>,
    close_all_calls: AtomicUsize,
    deny_close_all: AtomicBool,
}

impl RecordingGeoGateway {
    pub(super) fn geo_upgrades(&self) -> usize {
        self.geo_upgrades.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RuntimeGateway for RecordingGeoGateway {
    async fn switch_proxy(&self, _group: &str, _proxy: &str) -> Result<(), PortError> {
        self.switches.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn test_delay(&self, proxy: &str, url: &str, timeout_ms: u32) -> Result<u32, PortError> {
        self.probe_calls
            .lock()
            .unwrap()
            .push((proxy.into(), url.into(), timeout_ms));
        if self.deny_probe.load(Ordering::SeqCst)
            || self
                .denied_probe_names
                .lock()
                .unwrap()
                .iter()
                .any(|name| name == proxy)
        {
            Err(PortError::PermissionDenied(
                "grant proxy probe permission".into(),
            ))
        } else {
            Ok(42)
        }
    }

    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        Ok(Vec::new())
    }

    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        Ok(Vec::new())
    }

    async fn update_proxy_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn update_rule_provider(&self, _name: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn upgrade_geo(&self) -> Result<(), PortError> {
        self.geo_upgrades.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn get_connections(&self) -> Result<ConnectionSnapshot, PortError> {
        Ok(ConnectionSnapshot::default())
    }

    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        Ok(MemoryData::default())
    }

    async fn close_connection(&self, _id: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn close_all_connections(&self) -> Result<(), PortError> {
        self.close_all_calls.fetch_add(1, Ordering::SeqCst);
        if self.deny_close_all.load(Ordering::SeqCst) {
            Err(PortError::PermissionDenied(
                "grant connection control permission".into(),
            ))
        } else {
            Ok(())
        }
    }

    async fn stream_logs(
        &self,
        _level: Option<String>,
    ) -> Result<RuntimeStream<String>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }

    async fn stream_traffic(&self) -> Result<RuntimeStream<TrafficData>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }

    async fn stream_connections(&self) -> Result<RuntimeStream<ConnectionSnapshot>, PortError> {
        Err(PortError::Failed("not implemented".into()))
    }
}

fn application(store: &Arc<FakeStore>) -> CommandApplication {
    let store = Arc::clone(store) as Arc<dyn ProfileStore>;
    let profiles = ProfileApplication::new(Arc::clone(&store))
        .with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()));
    let configuration = ConfigurationApplication::from_profile_application(profiles.clone());
    with_test_capabilities(
        CommandApplication::new()
            .with_profile(profiles)
            .with_configuration(configuration),
    )
}

#[path = "command_application_tests/rule_edit_cases.rs"]
mod rule_edit_cases;

#[derive(Default)]
struct RecordingProviderCache {
    purges: Mutex<usize>,
}

impl RecordingProviderCache {
    fn purges(&self) -> usize {
        *self.purges.lock().expect("purge lock")
    }
}

#[async_trait]
impl RuleProviderCachePort for RecordingProviderCache {
    async fn read_provider(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError> {
        Ok(None)
    }

    async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
        *self.purges.lock().expect("purge lock") += 1;
        Ok(ProviderCachePurge {
            directory: Some("/fake/configs/rules".to_owned()),
            files_removed: 2,
            bytes_freed: 64,
        })
    }

    async fn fingerprint(
        &self,
        _declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderFileFact>, PortError> {
        Ok(None)
    }

    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
        Ok(RuleProviderCacheSnapshot::ready(
            "/fake/configs/rules",
            0,
            0,
        ))
    }
}

#[path = "command_probe_tests.rs"]
mod probe_tests;

#[path = "command_connections_tests.rs"]
mod connections_tests;

#[path = "command_profile_document_tests.rs"]
mod profile_documents;
