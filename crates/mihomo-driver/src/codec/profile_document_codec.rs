//! Pure Mihomo YAML document inspection.
//!
//! The driver owns the single neutral implementation of `scope`, `inspect` and
//! the editor snippet splice. It stays product-neutral: it has no snippet
//! catalogue of its own and no product type. The product adapter injects its
//! one snippet catalogue through [`SnippetBodyLookup`], so snippet bodies have
//! exactly one owner.

use crate::codec::snippet::splice_at_caret;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    ProfileDocumentSnippetInsertion, ProfileDocumentSnippetOutcome,
};
use std::str::from_utf8;
use yaml_rust2::YamlLoader;

pub const PROFILE_FORMAT: &str = "mihomo-yaml";
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// Resolves one editor snippet id to its exact YAML body.
///
/// The catalogue itself is a product/UI asset; the neutral driver only owns
/// the splice mechanics. A codec built without a lookup fails snippet
/// insertion closed.
pub type SnippetBodyLookup = fn(&str) -> Option<&'static str>;

#[derive(Clone, Copy, Debug, Default)]
pub struct MihomoProfileDocumentCodec {
    snippet_body: Option<SnippetBodyLookup>,
}

impl MihomoProfileDocumentCodec {
    /// Codec without a snippet catalogue: inspection works, snippet insertion
    /// fails closed with typed `Unsupported`.
    pub const fn new() -> Self {
        Self { snippet_body: None }
    }

    /// Codec that splices snippet bodies resolved by `snippet_body`.
    pub const fn with_snippets(snippet_body: SnippetBodyLookup) -> Self {
        Self {
            snippet_body: Some(snippet_body),
        }
    }
}

impl ProfileDocumentCodecPort for MihomoProfileDocumentCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        ProfileDocumentCodecScope {
            engine_id: EngineId::mihomo(),
            format_id: ProfileFormatId::new(PROFILE_FORMAT)
                .expect("static Mihomo format ID is valid"),
            schema_version: ProfileSchemaVersion::new(PROFILE_SCHEMA_VERSION)
                .expect("Mihomo schema version is positive"),
        }
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        let content = from_utf8(raw_bytes)
            .map_err(|error| PortError::Failed(format!("Mihomo YAML is not UTF-8: {error}")))?;
        match inspect_mihomo_yaml(content) {
            Ok(()) => Ok(DocumentValidation::Valid),
            Err(diagnostic) => Ok(DocumentValidation::Invalid {
                diagnostics: vec![DocumentDiagnosticSnapshot {
                    line: Some(diagnostic.line),
                    column: Some(diagnostic.column),
                    message: diagnostic.message,
                }],
            }),
        }
    }

    fn insert_snippet(
        &self,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        let Some(snippet_body) = self.snippet_body else {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "Mihomo editor snippets",
            ));
        };
        let previous = self.inspect(content.as_bytes())?;
        let Some(body) = snippet_body(snippet_id) else {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                format!("unknown Mihomo editor snippet: {snippet_id}"),
                false,
            )));
        };
        let spliced = splice_at_caret(content, snippet_id, body, caret);
        let validation = self.inspect(spliced.content.as_bytes())?;
        if matches!(&previous, DocumentValidation::Valid)
            && matches!(&validation, DocumentValidation::Invalid { .. })
        {
            return Ok(ProfileDocumentSnippetOutcome::Rejected { validation });
        }
        Ok(ProfileDocumentSnippetOutcome::Inserted(
            ProfileDocumentSnippetInsertion {
                snippet_id: spliced.snippet_id,
                content: spliced.content,
                cursor_line: spliced.cursor_line,
                cursor_column: spliced.cursor_column,
                validation,
            },
        ))
    }
}

struct MihomoYamlDiagnostic {
    line: usize,
    column: usize,
    message: String,
}

fn inspect_mihomo_yaml(content: &str) -> Result<(), MihomoYamlDiagnostic> {
    if content.trim().is_empty() {
        return Ok(());
    }
    match YamlLoader::load_from_str(content) {
        Ok(_) => Ok(()),
        Err(error) => Err(MihomoYamlDiagnostic {
            line: error.marker().line(),
            column: error.marker().col(),
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
#[path = "profile_document_codec_tests.rs"]
mod tests;
