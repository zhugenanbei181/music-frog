//! Explicit in-memory profile transactions shared by behavior tests and captures.
use crate::rule_source_identity::rule_workspace;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_domain::rules::source_identity::identify_rules_document;
use infiltrator_domain::rules::{RuleEntry, apply_rules_to_yaml};
use infiltrator_domain::yaml_edit::rule_location::replace_rule_at_location;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub const TRACE_PROFILE: &str = "trace.yaml";
pub const TRACE_DOCUMENT: &str = "mixed-port: 7890\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules:\n  - SRC-IP-CIDR,10.0.0.0/8,DIRECT\n  - DOMAIN,special.com,DIRECT\n  - DOMAIN-SUFFIX,google.com,PROXY\n  - MATCH,REJECT\n";
pub const NAMED_TRACE_DOCUMENT: &str = "# retained\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules:\n  - SUB-RULE,(DOMAIN-SUFFIX,google.com),media\n  - MATCH,DIRECT\nsub-rules:\n  media:\n    - SUB-RULE,(DST-PORT,443),secure\n    - MATCH,REJECT\n  secure:\n    - 'DOMAIN-SUFFIX,google.com,PROXY'  # preserve this\n    - MATCH,REJECT\n";
struct Files {
    current: String,
    documents: BTreeMap<String, String>,
}
pub struct RuleTraceStore {
    files: Mutex<Files>,
    pub deny_read: AtomicBool,
    pub deny_write: AtomicBool,
    pub writes: AtomicUsize,
    pub rule_loads: AtomicUsize,
}
impl Default for RuleTraceStore {
    fn default() -> Self {
        Self {
            files: Mutex::new(Files {
                current: TRACE_PROFILE.into(),
                documents: BTreeMap::from([(TRACE_PROFILE.into(), TRACE_DOCUMENT.into())]),
            }),
            deny_read: AtomicBool::new(false),
            deny_write: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
            rule_loads: AtomicUsize::new(0),
        }
    }
}
impl RuleTraceStore {
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
        let content = files
            .documents
            .get(&files.current)
            .ok_or_else(|| PortError::NotFound(files.current.clone()))?;
        rule_workspace(files.current.clone(), content).map_err(PortError::Rejected)
    }
    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        self.check_read()?;
        let mut files = self.files.lock().expect("isolated rule files");
        let content = files
            .documents
            .get(&files.current)
            .ok_or_else(|| PortError::NotFound(files.current.clone()))?;
        if identify_rules_document(files.current.clone(), content) != expected.source {
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
        let updated = apply_rules_to_yaml(content, entries).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                error.to_string(),
                false,
            ))
        })?;
        let current = files.current.clone();
        files.documents.insert(current, updated);
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
        let content = files
            .documents
            .get(&files.current)
            .ok_or_else(|| PortError::NotFound(files.current.clone()))?;
        if identify_rules_document(files.current.clone(), content) != expected.source {
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
        let updated = replace_rule_at_location(content, location, expected_rule, replacement)
            .map_err(|error| PortError::Rejected(Failure::unsupported(error.to_string())))?;
        let current = files.current.clone();
        files.documents.insert(current, updated);
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
    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        let mut files = self.files.lock().expect("isolated rule files");
        if !files.documents.contains_key(profile) {
            return Err(PortError::NotFound(profile.into()));
        }
        files.current = profile.into();
        Ok(())
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
    async fn save(&self, _: &str, _: &str) -> Result<(), PortError> {
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
    async fn load_options(&self, _: &str) -> Result<ProfileOptions, PortError> {
        Ok(ProfileOptions::default())
    }
    async fn save_options(&self, _: &str, _: &ProfileOptions) -> Result<(), PortError> {
        Err(outside())
    }
    async fn delete_options(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn clear_backup(&self, _: &str) -> Result<(), PortError> {
        Err(outside())
    }
    async fn restore_backup(&self, _: &str) -> Result<bool, PortError> {
        Err(outside())
    }
}
