//! Isolated profile-document codec fixture.
//!
//! The fixture really parses the Mihomo YAML document and really splices the
//! shared snippet catalogue. It never fabricates a valid document: a document
//! with a syntax error is reported as `Invalid` with the parser's own
//! position, and a codec without a snippet catalogue refuses snippet
//! insertion with a typed `Unsupported` failure.

use infiltrator_contract::yaml_snippets::yaml_snippet;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    ProfileDocumentSnippetOutcome,
};
use mihomo_driver::codec::profile_document_codec::MihomoProfileDocumentCodec;

/// Resolves one shared snippet id to the exact YAML body it inserts.
pub fn capture_snippet_body(snippet_id: &str) -> Option<&'static str> {
    yaml_snippet(snippet_id).map(|snippet| snippet.body)
}

/// An isolated profile-document codec over the real Mihomo YAML format.
pub struct ProfileDocumentCodecCapture {
    codec: MihomoProfileDocumentCodec,
}

impl Default for ProfileDocumentCodecCapture {
    fn default() -> Self {
        Self::with_snippets()
    }
}

impl ProfileDocumentCodecCapture {
    /// A codec that splices the shared snippet catalogue.
    pub fn with_snippets() -> Self {
        Self {
            codec: MihomoProfileDocumentCodec::with_snippets(capture_snippet_body),
        }
    }

    /// A codec without any snippet catalogue; insertion fails closed.
    pub fn without_snippets() -> Self {
        Self {
            codec: MihomoProfileDocumentCodec::new(),
        }
    }
}

impl ProfileDocumentCodecPort for ProfileDocumentCodecCapture {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.codec.scope()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        self.codec.inspect(raw_bytes)
    }

    fn insert_snippet(
        &self,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        self.codec.insert_snippet(content, snippet_id, caret)
    }
}
