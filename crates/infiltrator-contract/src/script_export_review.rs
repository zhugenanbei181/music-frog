//! Frozen export content is reviewed before a host write is admitted.
use crate::error::{ErrorCode, Failure};
use crate::script_export::{ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot};
use crate::script_run::MAX_SCRIPT_INPUT_BYTES;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportDraft {
    pub kind: ScriptExportKind,
    pub profile: Option<String>,
    pub base_yaml: String,
    pub mixin_yaml: String,
    pub script_code: String,
    pub preset: Option<String>,
}
impl ScriptExportDraft {
    pub fn validate(&self) -> Result<(), Failure> {
        let bytes = self
            .base_yaml
            .len()
            .saturating_add(self.mixin_yaml.len())
            .saturating_add(self.script_code.len());
        if bytes > MAX_SCRIPT_INPUT_BYTES
            || self
                .profile
                .as_ref()
                .is_some_and(|value| value.len() > 4096)
            || self.preset.as_ref().is_some_and(|value| value.len() > 128)
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Oversized export draft",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportIdentity {
    pub owner: u64,
    pub sequence: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportReview {
    pub identity: ScriptExportIdentity,
    pub draft: ScriptExportDraft,
    pub snapshot: ScriptExportSnapshot,
}
impl ScriptExportReview {
    pub fn validate(&self) -> Result<(), Failure> {
        self.draft.validate()?;
        if self.identity.owner == 0
            || self.identity.sequence == 0
            || self.identity.sha256.len() != 64
            || !self
                .identity
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || self.snapshot.file_name.trim().is_empty()
            || self.snapshot.content.len() > MAX_SCRIPT_INPUT_BYTES
            || !matches!(self.snapshot.outcome, ScriptExportOutcome::Prepared)
            || self.snapshot.kind != self.draft.kind
            || self.snapshot.profile != self.draft.profile
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Invalid export review",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportSaved {
    pub identity: ScriptExportIdentity,
    pub snapshot: ScriptExportSnapshot,
}
impl ScriptExportSaved {
    pub fn validate(&self, identity: &ScriptExportIdentity) -> Result<(), Failure> {
        if self.identity != *identity
            || !matches!(&self.snapshot.outcome,
            ScriptExportOutcome::Saved { path, bytes_written }
                if !path.trim().is_empty() && *bytes_written == self.snapshot.content.len())
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Invalid export write receipt",
                false,
            ));
        }
        Ok(())
    }
}
