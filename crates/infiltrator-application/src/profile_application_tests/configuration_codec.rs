//! Engine-scoped configuration codec behavior and source-bound writes.
//! test-intent: behavior

use super::*;
use crate::configuration_application::ConfigurationApplication;
use crate::profile_application::ProfileRuntimeAccess;
use crate::protocol_codec_application::ProtocolCodecApplication;
use async_trait::async_trait;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_configuration::{
    ProfileConfigurationRequest, ProfileConfigurationUpdate as ContractConfigurationUpdate,
};
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentRevision, ProfileFormatId, ProfileSchemaVersion,
};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::protocol_fidelity::{CodecAudit, NodeCodecFormat, ProtocolDraft};
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::runtime::{MemoryData, ProxyProvider, RuleProvider};
use infiltrator_domain::tun::TunConfigPatch;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationQuery, ProfileConfigurationReadout,
    ProfileProtocolNodeMutationReadback, ProfileRuleWorkspaceUpdate,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct WorkspaceApplyRuntime {
    calls: AtomicUsize,
    update: Mutex<Option<ProfileWorkspaceUpdate>>,
}

impl WorkspaceApplyRuntime {
    fn unexpected(&self) -> ! {
        panic!("unexpected controller operation during profile workspace apply")
    }
}

#[async_trait]
impl RuntimeGateway for WorkspaceApplyRuntime {
    async fn switch_proxy(&self, _: &str, _: &str) -> Result<(), PortError> {
        self.unexpected()
    }
    async fn test_delay(&self, _: &str, _: &str, _: u32) -> Result<u32, PortError> {
        self.unexpected()
    }
    async fn get_proxy_providers(&self) -> Result<Vec<ProxyProvider>, PortError> {
        self.unexpected()
    }
    async fn get_rule_providers(&self) -> Result<Vec<RuleProvider>, PortError> {
        self.unexpected()
    }
    async fn update_proxy_provider(&self, _: &str) -> Result<(), PortError> {
        self.unexpected()
    }
    async fn update_rule_provider(&self, _: &str) -> Result<(), PortError> {
        self.unexpected()
    }
    async fn get_memory(&self) -> Result<MemoryData, PortError> {
        self.unexpected()
    }
}

#[async_trait]
impl ManagedRuntime for WorkspaceApplyRuntime {
    fn engine_id(&self) -> Option<infiltrator_contract::core_identity::EngineId> {
        infiltrator_contract::core_identity::EngineId::new("mihomo").ok()
    }
    fn generation(&self) -> u64 {
        7
    }
    fn core_lifecycle(&self) -> infiltrator_contract::snapshot::CoreLifecycle {
        infiltrator_contract::snapshot::CoreLifecycle::Running
    }
    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<
        infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization,
        PortError,
    > {
        Ok(infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: infiltrator_contract::core_identity::CoreIdentity::new(
                infiltrator_contract::core_identity::EngineId::new("mihomo")
                    .expect("test engine"),
                infiltrator_contract::core_identity::CoreInstanceId::new("workspace-test-core")
                    .expect("test owner instance"),
                None,
            ),
            generation: infiltrator_contract::core_identity::CoreGeneration::new(7),
        })
    }
    async fn is_running(&self) -> bool {
        true
    }
    async fn restart(&self) -> Result<u64, PortError> {
        self.unexpected()
    }
    async fn http_proxy_endpoint(&self) -> Result<Option<String>, PortError> {
        self.unexpected()
    }
    async fn shutdown(&self) -> Result<(), PortError> {
        self.unexpected()
    }
    async fn apply_profile_workspace(
        &self,
        _: &infiltrator_contract::active_profile_reference::ActiveProfileReference,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        _: infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization,
        _: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.update.lock().unwrap() = Some(update.clone());
        let mut receipt = crate::profile_workspace_test_support::workspace_with_raw_sidecar(
            expected.document.profile_id.as_str(),
            &update.document_bytes,
            update.sidecar_bytes.clone(),
            None,
        )?;
        receipt.document.identity.source = expected.document.source.clone();
        Ok(receipt)
    }
}

fn configuration_application(
    store: Arc<FakeStore>,
    codec: Arc<TestProfileDocumentCodec>,
) -> ConfigurationApplication {
    ConfigurationApplication::from_profile_application(
        ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
            .with_document_codec(codec),
    )
}

#[tokio::test]
async fn foreign_format_is_rejected_before_the_codec_is_called() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let workspace = profile.load_active_workspace().await.unwrap();
    let mut identity = workspace.document.identity;
    identity.format_id = ProfileFormatId::new("mihomo-json").unwrap();
    identity.schema_version = ProfileSchemaVersion::new(1).unwrap();

    let failure = profile
        .read_profile_configuration(&identity, b"mode: rule\n", ProfileConfigurationQuery::Dns)
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Unsupported);
    let endpoint_failure = profile
        .read_profile_configuration(
            &identity,
            b"not: [valid YAML",
            ProfileConfigurationQuery::ProxyEndpoint,
        )
        .unwrap_err();
    assert_eq!(endpoint_failure.code, ErrorCode::Unsupported);
    assert_eq!(codec.configuration_read_count(), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn protocol_and_ca_queries_use_queued_typed_readouts_without_yaml_fallback() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let workspace = profiles.load_active_workspace().await.unwrap();
    let identity = workspace.document.identity;
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "typed-readback".to_owned();
    draft.server = "node.example.com".to_owned();
    draft.port = 443;
    codec.queue_configuration_readout(
        ProfileConfigurationQuery::ProtocolNodes,
        ProfileConfigurationReadout::ProtocolNodes(vec![draft.clone()]),
    );
    codec.queue_configuration_readout(
        ProfileConfigurationQuery::TrustAnchors,
        ProfileConfigurationReadout::TrustAnchors(TlsTrustParams {
            ca_path: "/etc/ssl/custom-ca.pem".to_owned(),
            ..TlsTrustParams::default()
        }),
    );

    let protocol = profiles
        .read_profile_configuration(
            &identity,
            b"opaque protocol bytes",
            ProfileConfigurationQuery::ProtocolNodes,
        )
        .expect("typed protocol readout");
    let ProfileConfigurationReadout::ProtocolNodes(nodes) = protocol else {
        panic!("expected typed protocol nodes");
    };
    assert_eq!(nodes, vec![draft]);

    let trust = profiles
        .read_profile_configuration(
            &identity,
            b"opaque CA bytes",
            ProfileConfigurationQuery::TrustAnchors,
        )
        .expect("typed CA readout");
    let ProfileConfigurationReadout::TrustAnchors(params) = trust else {
        panic!("expected typed CA trust anchors");
    };
    assert_eq!(params.ca_path, "/etc/ssl/custom-ca.pem");
    assert_eq!(codec.configuration_read_count(), 2);
    assert_eq!(codec.configuration_update_count(), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_codec_update_is_rejected_before_profile_write() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "tun:\n  enable: false\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    codec.queue_configuration_mutation(Err(PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        "fake codec rejected an invalid TUN update",
        false,
    ))));
    let application = configuration_application(store.clone(), codec.clone());

    let failure = application
        .save_tun_config(TunConfigPatch {
            mtu: Some(0),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(codec.configuration_update_count(), 1);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.load("main").await.unwrap(), "tun:\n  enable: false\n");
}

#[tokio::test]
async fn stale_source_rejected_after_pure_codec_transform_before_write() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "tun:\n  enable: false\n",
        true,
    ));
    *store.workspace_write_error.lock().unwrap() = Some(PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        "fake store observed a stale source revision",
        true,
    )));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let application = configuration_application(store.clone(), codec.clone());

    let failure = application
        .save_tun_config(TunConfigPatch {
            enable: Some(true),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(codec.configuration_update_count(), 1);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.load("main").await.unwrap(), "tun:\n  enable: false\n");
}

#[tokio::test]
async fn committed_configuration_returns_codec_readback_for_exact_written_bytes() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "tun:\n  enable: false\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    codec.queue_configuration_mutation(Ok(ProfileConfigurationMutation {
        document_bytes: b"tun:\n  enable: true\n  mtu: 9000\n".to_vec(),
        readback: ProfileConfigurationReadout::Tun(infiltrator_domain::tun::TunConfig {
            enable: Some(true),
            mtu: Some(9000),
            ..Default::default()
        }),
    }));
    let application = configuration_application(store.clone(), codec);

    let readback = application
        .save_tun_config(TunConfigPatch {
            enable: Some(true),
            mtu: Some(9000),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(readback.enable, Some(true));
    assert_eq!(readback.mtu, Some(9000));
    assert_eq!(
        store.load("main").await.unwrap(),
        "tun:\n  enable: true\n  mtu: 9000\n"
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn desktop_managed_runtime_receives_scoped_dns_mutation_and_returns_readback() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "dns:\n  enable: false\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let application = configuration_application(store, codec);
    let runtime = Arc::new(WorkspaceApplyRuntime::default());

    let readback = application
        .apply_dns_settings_with_runtime(
            ProfileRuntimeAccess::Managed(runtime.clone()),
            DnsSettingsPatch {
                expected_profile: Some("main".into()),
                nameserver: Some(vec!["https://dns.example/query".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        readback.nameserver,
        Some(vec!["https://dns.example/query".into()])
    );
    let committed = runtime.update.lock().unwrap().clone().unwrap();
    let document = String::from_utf8(committed.document_bytes).unwrap();
    assert!(document.contains("https://dns.example/query"));
}

#[tokio::test]
async fn typed_profile_update_rejects_stale_document_identity_before_codec_or_write() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "rule-providers: {}\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let observed = profile.load_active_workspace().await.unwrap();
    let mut stale_identity = observed.document.identity;
    stale_identity.revision = ProfileDocumentRevision::new("stale-revision").unwrap();
    let application = ConfigurationApplication::from_profile_application(
        profile.with_document_codec(codec.clone()),
    );

    let failure = application
        .apply_profile_configuration_request(ProfileConfigurationRequest {
            identity: stale_identity,
            update: ContractConfigurationUpdate::RuleProviders(BTreeMap::new()),
        })
        .await
        .unwrap_err();

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(codec.configuration_update_count(), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.load("main").await.unwrap(), "rule-providers: {}\n");
}

#[tokio::test]
async fn typed_profile_update_commits_against_observed_revision_and_returns_readback() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "rule-providers: {}\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profile = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let observed = profile.load_active_workspace().await.unwrap();
    let mut providers = BTreeMap::new();
    providers.insert(
        "ads".to_owned(),
        serde_json::json!({"type": "inline", "behavior": "domain"}),
    );
    codec.queue_configuration_mutation(Ok(ProfileConfigurationMutation {
        document_bytes: b"rule-providers:\n  ads:\n    type: inline\n    behavior: domain\n"
            .to_vec(),
        readback: ProfileConfigurationReadout::RuleProviders(providers.clone()),
    }));
    let application = ConfigurationApplication::from_profile_application(
        profile.with_document_codec(codec.clone()),
    );

    let readback = application
        .apply_profile_configuration_request(ProfileConfigurationRequest {
            identity: observed.document.identity,
            update: ContractConfigurationUpdate::RuleProviders(providers.clone()),
        })
        .await
        .unwrap();

    assert!(
        matches!(readback, ProfileConfigurationReadout::RuleProviders(value) if value == providers)
    );
    assert_eq!(codec.configuration_update_count(), 1);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store.load("main").await.unwrap(),
        "rule-providers:\n  ads:\n    type: inline\n    behavior: domain\n"
    );
}

#[tokio::test]
async fn android_proxy_endpoint_read_goes_through_the_scoped_profile_codec() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "mixed-port: '7893'\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let application = configuration_application(store.clone(), codec.clone());

    assert_eq!(
        application.load_proxy_endpoint().await.unwrap(),
        Some("socks5://127.0.0.1:7893".to_owned())
    );
    assert_eq!(codec.configuration_read_count(), 1);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn rules_editor_read_projections_share_the_exact_document_revision() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "rule-providers:\n  ads:\n    type: inline\nproxy-providers:\n  edge:\n    type: http\nsniffer:\n  enable: true\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let application = configuration_application(store.clone(), codec.clone());

    let documents = application.load_rules_json_documents().await.unwrap();

    assert_eq!(documents.len(), 3);
    assert!(
        documents
            .iter()
            .all(|document| document.identity == documents[0].identity)
    );
    assert_eq!(codec.configuration_read_count(), 3);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    assert!(
        documents
            .iter()
            .any(|document| document.json.contains("ads"))
    );
    assert!(
        documents
            .iter()
            .any(|document| document.json.contains("edge"))
    );
    assert!(
        documents
            .iter()
            .any(|document| document.json.contains("enable"))
    );
}

#[tokio::test]
async fn rule_workspace_read_and_edit_dispatch_through_codec_without_storage_side_effects() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "rules:\n  - MATCH,DIRECT\nsub-rules:\n  regional:\n    - DOMAIN,example.com,DIRECT\n",
        true,
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let workspace = application.load_active_workspace().await.unwrap();

    let projected = application
        .read_profile_rule_workspace(&workspace)
        .expect("scoped rule projection");
    assert_eq!(projected.profile_identity, workspace.identity());
    assert_eq!(projected.rules.len(), 1);
    assert_eq!(
        projected.sub_rules["regional"][0].rule,
        "DOMAIN,example.com,DIRECT"
    );

    let mutation = application
        .apply_profile_rule_workspace(
            &workspace.identity(),
            &workspace.document.raw_bytes,
            ProfileRuleWorkspaceUpdate::ReplaceRules(vec![infiltrator_domain::rules::RuleEntry {
                rule: "MATCH,REJECT".to_owned(),
                enabled: true,
            }]),
        )
        .expect("codec-scoped rule mutation");
    assert_eq!(mutation.readback.rules[0].rule, "MATCH,REJECT");
    assert!(
        String::from_utf8(mutation.document_bytes)
            .unwrap()
            .contains("MATCH,REJECT")
    );
    assert_eq!(codec.configuration_read_count(), 2);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

fn protocol_save_draft() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "new-node".to_owned();
    draft.server = "node.example.com".to_owned();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    draft.params.tls_trust.ca_path = "/etc/ssl/custom-ca.pem".to_owned();
    draft
}

fn queue_protocol_save(
    codec: &TestProfileDocumentCodec,
    draft: &ProtocolDraft,
    output_bytes: &[u8],
) {
    codec.queue_configuration_mutation(Ok(ProfileConfigurationMutation {
        document_bytes: b"mode: rule\ntls:\n  custom-certifactes:\n    - /etc/ssl/custom-ca.pem\n"
            .to_vec(),
        readback: ProfileConfigurationReadout::TrustAnchors(TlsTrustParams {
            ca_path: draft.params.tls_trust.ca_path.clone(),
            ..TlsTrustParams::default()
        }),
    }));
    codec.queue_configuration_mutation(Ok(ProfileConfigurationMutation {
        document_bytes: output_bytes.to_vec(),
        readback: ProfileConfigurationReadout::ProtocolNodeMutation(
            ProfileProtocolNodeMutationReadback {
                nodes: vec![draft.clone()],
                audit: CodecAudit {
                    source_format: NodeCodecFormat::Uri,
                    target_format: NodeCodecFormat::ClashYaml,
                    node_count: 1,
                    unknown_fields: vec!["future-field".to_owned()],
                    structure_preserved: true,
                    lossless: true,
                    detail: "fake exact protocol mutation".to_owned(),
                },
                replaced_existing: false,
            },
        ),
    }));
}

#[tokio::test]
async fn protocol_and_ca_codecs_commit_exact_typed_readback_bytes() {
    let original = "mode: rule\nproxies: []\n";
    let output = b"mode: rule\ntls:\n  custom-certifactes:\n    - /etc/ssl/custom-ca.pem\nproxies:\n  - name: new-node\n    type: vless\n";
    let store = Arc::new(FakeStore::with_profile("main", original, true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let draft = protocol_save_draft();
    queue_protocol_save(&codec, &draft, output);
    let runtime = Arc::new(WorkspaceApplyRuntime::default());

    let commit = ProtocolCodecApplication::save_draft_to_active_profile(
        &profiles,
        runtime.clone(),
        &draft,
        ProfileWriteAuthorization::RequireUnlock,
    )
    .await
    .expect("typed protocol transaction");

    assert_eq!(commit.profile_yaml.as_bytes(), output);
    assert_eq!(commit.draft, draft);
    assert_eq!(commit.node_count, 1);
    assert!(commit.audit.lossless);
    assert_eq!(commit.audit.unknown_fields, vec!["future-field"]);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime
            .update
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .document_bytes,
        output
    );
    assert_eq!(codec.configuration_update_count(), 2);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn foreign_protocol_codec_scope_fails_before_dispatch_or_transaction() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::for_scope(
        "mihomo",
        "other-yaml",
        1,
    ));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let runtime = Arc::new(WorkspaceApplyRuntime::default());

    let failure = ProtocolCodecApplication::save_draft_to_active_profile(
        &profiles,
        runtime.clone(),
        &protocol_save_draft(),
        ProfileWriteAuthorization::RequireUnlock,
    )
    .await
    .unwrap_err();

    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(codec.configuration_update_count(), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_protocol_codec_candidate_has_no_workspace_or_runtime_write() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let draft = protocol_save_draft();
    codec.queue_configuration_mutation(Ok(ProfileConfigurationMutation {
        document_bytes: b"mode: rule\n".to_vec(),
        readback: ProfileConfigurationReadout::TrustAnchors(TlsTrustParams::default()),
    }));
    codec.queue_configuration_mutation(Err(PortError::Rejected(Failure::new(
        ErrorCode::InvalidInput,
        "fake Mihomo codec rejected an invalid protocol candidate",
        false,
    ))));
    let runtime = Arc::new(WorkspaceApplyRuntime::default());

    let failure = ProtocolCodecApplication::save_draft_to_active_profile(
        &profiles,
        runtime.clone(),
        &draft,
        ProfileWriteAuthorization::RequireUnlock,
    )
    .await
    .unwrap_err();

    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(codec.configuration_update_count(), 2);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn stale_profile_revision_after_codec_transform_has_no_runtime_or_store_write() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    store
        .stale_after_next_workspace_read
        .store(true, Ordering::SeqCst);
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let draft = protocol_save_draft();
    queue_protocol_save(&codec, &draft, b"mode: rule\nproxies: []\n");
    let runtime = Arc::new(WorkspaceApplyRuntime::default());

    let failure = ProtocolCodecApplication::save_draft_to_active_profile(
        &profiles,
        runtime.clone(),
        &draft,
        ProfileWriteAuthorization::RequireUnlock,
    )
    .await
    .unwrap_err();

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(codec.configuration_update_count(), 2);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
}
