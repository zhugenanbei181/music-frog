//! Product-level typed configuration extension for the sing-box profile codec.
//!
//! The neutral inspection implementation (`scope`/`inspect`/`insert_snippet`)
//! is owned by `singbox_driver::codec::SingBoxProfileDocumentCodec`. This
//! newtype only exists because the product `ProfileConfigurationCodecPort`
//! extension is a foreign trait: the orphan rule requires a product-local type.
//! It delegates every neutral operation to the single driver implementation and
//! leaves every typed operation on the trait's fail-closed default, because
//! sing-box has no product-level typed configuration extension.

use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    ProfileDocumentSnippetOutcome,
};
use singbox_driver::codec::profile_document_codec::SingBoxProfileDocumentCodec;

#[derive(Clone, Copy, Debug, Default)]
pub struct SingBoxProfileConfigurationCodec(SingBoxProfileDocumentCodec);

impl SingBoxProfileConfigurationCodec {
    pub const fn new() -> Self {
        Self(SingBoxProfileDocumentCodec)
    }
}

impl ProfileDocumentCodecPort for SingBoxProfileConfigurationCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.scope()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        self.0.inspect(raw_bytes)
    }

    fn insert_snippet(
        &self,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        self.0.insert_snippet(content, snippet_id, caret)
    }
}

/// sing-box does not implement any typed configuration extension; every
/// product-level operation fails closed through the trait defaults.
impl ProfileConfigurationCodecPort for SingBoxProfileConfigurationCodec {}

#[cfg(test)]
#[path = "profile_document_codec_tests.rs"]
mod tests;
