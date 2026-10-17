//! Engine/format/schema-scoped, pure inspection of opaque profile documents.
//!
//! This neutral trait only covers operations that need no product type: scope
//! identity, syntax/format inspection and a format-neutral editor snippet
//! splice. Typed configuration, workspace, mixin and filter operations are
//! product-owned and live in `infiltrator-ports`.

use crate::error::PortError;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};

/// Immutable engine/format/schema identity of one document codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDocumentCodecScope {
    pub engine_id: EngineId,
    pub format_id: ProfileFormatId,
    pub schema_version: ProfileSchemaVersion,
}

/// Character-based caret used by format adapters that support editor snippets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileDocumentCaret {
    /// One-based line number.
    pub line: usize,
    /// Zero-based character column.
    pub column: usize,
}

/// Format-neutral result of a successful editor snippet splice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDocumentSnippetInsertion {
    pub snippet_id: String,
    pub content: String,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub validation: DocumentValidation,
}

/// A splice may be applied to an already-invalid document, but a splice that
/// makes a valid document invalid is rejected with neutral diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileDocumentSnippetOutcome {
    Inserted(ProfileDocumentSnippetInsertion),
    Rejected { validation: DocumentValidation },
}

/// Pure format inspection for exact document bytes. Implementations must not
/// read or write the profile store or contact a running core.
pub trait ProfileDocumentCodecPort: Send + Sync {
    /// Return immutable adapter identity without performing I/O.
    fn scope(&self) -> ProfileDocumentCodecScope;

    /// Inspect the supplied bytes without mutating them or invoking runtime work.
    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError>;

    /// Insert one adapter-owned editor snippet without touching profile
    /// storage or runtime state. Formats without snippet support fail closed.
    fn insert_snippet(
        &self,
        _content: &str,
        _snippet_id: &str,
        _caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile editor snippets",
        ))
    }
}
