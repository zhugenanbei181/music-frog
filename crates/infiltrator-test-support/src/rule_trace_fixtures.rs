//! Explicit in-memory profile transactions shared by behavior tests and captures.
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use crate::profile_workspace_test_support::workspace_with_raw_sidecar;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::{OutboundDetailsSnapshot, OutboundDetailsState};
use infiltrator_contract::outbound::identity::SnapshotRevision;
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOwnerBinding, RuleTraceRequest, ScopedRuleTraceRequest,
};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileRuleWorkspaceUpdate,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use lurker_core_contract::session::SessionToken;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub const TRACE_PROFILE: &str = "trace.yaml";
pub const TRACE_DOCUMENT: &str = "mixed-port: 7890\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules:\n  - SRC-IP-CIDR,10.0.0.0/8,DIRECT\n  - DOMAIN,special.com,DIRECT\n  - DOMAIN-SUFFIX,google.com,PROXY\n  - MATCH,REJECT\n";
pub const NAMED_TRACE_DOCUMENT: &str = "# retained\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules:\n  - SUB-RULE,(DOMAIN-SUFFIX,google.com),media\n  - MATCH,DIRECT\nsub-rules:\n  media:\n    - SUB-RULE,(DST-PORT,443),secure\n    - MATCH,REJECT\n  secure:\n    - 'DOMAIN-SUFFIX,google.com,PROXY'  # preserve this\n    - MATCH,REJECT\n";

pub fn rule_trace_test_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("fixture engine ID"),
            CoreInstanceId::new("rule-trace-test").expect("fixture instance ID"),
            Some(CoreVersion::new("test-version").expect("fixture version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    }
}

pub fn scoped_rule_trace_request(
    request: RuleTraceRequest,
    workspace: ProfileWorkspaceIdentity,
) -> (CoreScope, ProfileWorkspaceIdentity, ScopedRuleTraceRequest) {
    let scope = rule_trace_test_scope();
    let binding = RuleTraceOwnerBinding::new(scope.clone(), workspace.clone())
        .expect("fixture owner engine matches workspace");
    let request =
        ScopedRuleTraceRequest::new(binding, request).expect("fixture rule trace request is valid");
    (scope, workspace, request)
}

/// Typed local-unknown outbound facts for rule-trace tests and capture harnesses.
/// Live commands must supply an adapter-authored observation instead.
pub fn unknown_rule_trace_observation(scope: CoreScope) -> OutboundObservationSnapshot {
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundInventoryState::Unknown,
    )
    .expect("unknown inventory is a valid observation");
    let details = OutboundDetailsSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundDetailsState::Unknown,
    )
    .expect("unknown details are a valid observation");
    OutboundObservationSnapshot::new(inventory, details)
        .expect("unknown inventory and details share one scope")
}

struct Files {
    current: String,
    documents: BTreeMap<String, String>,
}
pub struct RuleTraceStore {
    files: Mutex<Files>,
    document_codec: Arc<dyn ProfileDocumentCodecPort>,
    pub deny_read: AtomicBool,
    pub deny_write: AtomicBool,
    pub writes: AtomicUsize,
    pub rule_loads: AtomicUsize,
}
impl Default for RuleTraceStore {
    fn default() -> Self {
        Self::with_document_codec(Arc::new(TestProfileDocumentCodec::mihomo()))
    }
}

impl RuleTraceStore {
    pub fn with_document_codec(document_codec: Arc<dyn ProfileDocumentCodecPort>) -> Self {
        Self {
            files: Mutex::new(Files {
                current: TRACE_PROFILE.into(),
                documents: BTreeMap::from([(TRACE_PROFILE.into(), TRACE_DOCUMENT.into())]),
            }),
            document_codec,
            deny_read: AtomicBool::new(false),
            deny_write: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
            rule_loads: AtomicUsize::new(0),
        }
    }

    fn codec(&self) -> &dyn ProfileDocumentCodecPort {
        self.document_codec.as_ref()
    }

    fn workspace(&self, profile: &str, content: &str) -> Result<ProfileWorkspace, PortError> {
        workspace_with_raw_sidecar(profile, content.as_bytes(), None, None)
    }

    fn read_rule_workspace(
        &self,
        profile: &str,
        content: &str,
    ) -> Result<RuleWorkspace, PortError> {
        let workspace = self.workspace(profile, content)?;
        let projection = self
            .codec()
            .read_rule_workspace(&workspace.identity(), &workspace.document.raw_bytes)?;
        Ok(RuleWorkspace {
            source: projection.source,
            profile_identity: workspace.identity(),
            rules: projection.rules,
            targets: projection.targets,
            sub_rules: projection.sub_rules,
        })
    }

    pub fn content(&self) -> String {
        self.files.lock().expect("isolated rule files").documents[TRACE_PROFILE].clone()
    }
    pub fn content_for(&self, profile: &str) -> Option<String> {
        self.files
            .lock()
            .expect("isolated rule files")
            .documents
            .get(profile)
            .cloned()
    }
    pub fn replace(&self, content: &str) {
        self.files
            .lock()
            .expect("isolated rule files")
            .documents
            .insert(TRACE_PROFILE.into(), content.into());
    }
    pub fn select_profile(&self, profile: &str, content: &str) {
        let mut files = self.files.lock().expect("isolated rule files");
        files
            .documents
            .insert(profile.to_owned(), content.to_owned());
        files.current = profile.to_owned();
    }
    fn check_read(&self) -> Result<(), PortError> {
        if self.deny_read.load(Ordering::SeqCst) {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile read access, then retry",
                true,
            )))
        } else {
            Ok(())
        }
    }
}
#[async_trait]
impl RuleOverridePort for RuleTraceStore {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        self.rule_loads.fetch_add(1, Ordering::SeqCst);
        self.check_read()?;
        let files = self.files.lock().expect("isolated rule files");
        let profile = files.current.clone();
        let content = files
            .documents
            .get(&profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.clone()))?;
        drop(files);
        self.read_rule_workspace(&profile, &content)
    }
    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        self.check_read()?;
        let mut files = self.files.lock().expect("isolated rule files");
        let profile = files.current.clone();
        let content = files
            .documents
            .get(&profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.clone()))?;
        let observed = self.read_rule_workspace(&profile, &content)?;
        if observed.profile_identity != expected.profile_identity
            || observed.source != expected.source
            || observed.rules != expected.rules
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The source document changed",
                true,
            )));
        }
        if self.deny_write.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile write access, then retry",
                true,
            )));
        }
        let workspace = self.workspace(&profile, &content)?;
        let mutation = self.codec().apply_rule_workspace(
            &workspace.identity(),
            &workspace.document.raw_bytes,
            ProfileRuleWorkspaceUpdate::ReplaceRules(entries.to_vec()),
        )?;
        let updated = String::from_utf8(mutation.document_bytes)
            .map_err(|error| PortError::Failed(error.to_string()))?;
        files.documents.insert(profile, updated);
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn compare_and_apply_named_rule(
        &self,
        expected: &RuleWorkspace,
        location: &RuleLocation,
        expected_rule: &str,
        replacement: &str,
    ) -> Result<(), PortError> {
        self.check_read()?;
        let mut files = self.files.lock().expect("isolated rule files");
        let profile = files.current.clone();
        let content = files
            .documents
            .get(&profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.clone()))?;
        let observed = self.read_rule_workspace(&profile, &content)?;
        if observed.profile_identity != expected.profile_identity
            || observed.source != expected.source
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The source document changed",
                true,
            )));
        }
        if self.deny_write.load(Ordering::SeqCst) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Permission,
                "Allow profile write access, then retry",
                true,
            )));
        }
        let workspace = self.workspace(&profile, &content)?;
        let mutation = self.codec().apply_rule_workspace(
            &workspace.identity(),
            &workspace.document.raw_bytes,
            ProfileRuleWorkspaceUpdate::ReplaceNamedRule {
                location: location.clone(),
                expected_rule: expected_rule.to_owned(),
                replacement: replacement.to_owned(),
            },
        )?;
        let updated = String::from_utf8(mutation.document_bytes)
            .map_err(|error| PortError::Failed(error.to_string()))?;
        files.documents.insert(profile, updated);
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn outside() -> PortError {
    PortError::unsupported(
        Capability::Profiles,
        "Operation is outside the isolated rule trace fixture",
    )
}
#[async_trait]
impl ProfileStore for RuleTraceStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/isolated-rule-trace")
    }
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        self.check_read()?;
        let files = self.files.lock().expect("isolated rule files");
        Ok(files
            .documents
            .keys()
            .map(|name| ProfileInfo {
                name: name.clone(),
                active: *name == files.current,
                ..ProfileInfo::default()
            })
            .collect())
    }
    async fn get_current(&self) -> Result<String, PortError> {
        self.check_read()?;
        Ok(self
            .files
            .lock()
            .expect("isolated rule files")
            .current
            .clone())
    }
    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.check_read()?;
        self.files
            .lock()
            .expect("isolated rule files")
            .documents
            .get(profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.into()))
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        self.check_read()?;
        let files = self.files.lock().expect("isolated rule files");
        let profile = files.current.clone();
        let content = files
            .documents
            .get(&profile)
            .cloned()
            .ok_or_else(|| PortError::NotFound(profile.clone()))?;
        drop(files);
        self.workspace(&profile, &content)
    }
    async fn save_inactive_profile(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_profile(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn get_profile_metadata(&self, _: &str) -> Result<ProfileMetadata, PortError> {
        Ok(ProfileMetadata::default())
    }
    async fn update_profile_metadata(&self, _: &str, _: &ProfileMetadata) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_subscription_credential(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn restore_inactive_profile_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(outside())
    }
}
