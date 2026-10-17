//! Shared profile-document read model with engine-authored inspection facts.

use crate::profile_document_identity::{
    ProfileFormatId, ProfileSchemaVersion, ProfileWorkspaceIdentity,
};
use crate::profile_options::ProfileOptionsSnapshot;
use crate::profile_protection::ProfileWriteProtection;
use lurker_core_contract::identity::EngineId;
use serde::{Deserialize, Serialize};

/// Format-neutral diagnostic emitted by a document codec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnosticSnapshot {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentValidation {
    Valid,
    Invalid {
        diagnostics: Vec<DocumentDiagnosticSnapshot>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentInspectionUnavailableReason {
    CodecMissing,
    AdapterUnavailable { detail: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentInspectionUnsupportedReason {
    EngineMismatch {
        document: EngineId,
        expected: EngineId,
    },
    FormatMismatch {
        document: ProfileFormatId,
        codec: ProfileFormatId,
    },
    SchemaMismatch {
        document: ProfileSchemaVersion,
        codec: ProfileSchemaVersion,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentInspection {
    Inspected(DocumentValidation),
    Unavailable(DocumentInspectionUnavailableReason),
    Unsupported(DocumentInspectionUnsupportedReason),
}

/// The stored document of one profile plus its engine-scoped inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocumentSnapshot {
    pub workspace_identity: ProfileWorkspaceIdentity,
    pub content: String,
    /// The same write classification the save guard enforces (DUAL-09-12).
    pub write_protection: ProfileWriteProtection,
    /// A codec-authored result. Missing or mismatched codecs are never treated
    /// as successful validation.
    pub inspection: DocumentInspection,
    pub line_count: usize,
}

impl ProfileDocumentSnapshot {
    pub fn new(
        workspace_identity: ProfileWorkspaceIdentity,
        content: impl Into<String>,
        write_protection: ProfileWriteProtection,
    ) -> Self {
        let content = content.into();
        let line_count = content.lines().count();
        Self {
            workspace_identity,
            content,
            write_protection,
            inspection: DocumentInspection::Unavailable(
                DocumentInspectionUnavailableReason::CodecMissing,
            ),
            line_count,
        }
    }

    pub fn with_inspection(mut self, inspection: DocumentInspection) -> Self {
        self.inspection = inspection;
        self
    }

    /// Line numbers rendered in the editor gutter.
    pub fn line_numbers(&self) -> impl Iterator<Item = usize> + '_ {
        1..=self.line_count.max(1)
    }
}

/// Both facets returned by one actual document/sidecar transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileWorkspaceReceipt {
    pub previous: ProfileWorkspaceIdentity,
    pub submitted_mixin_yaml: String,
    pub document: ProfileDocumentSnapshot,
    pub options: ProfileOptionsSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocumentSaved {
    pub previous: ProfileWorkspaceIdentity,
    pub document: ProfileDocumentSnapshot,
}
