use super::*;
use infiltrator_contract::profile_document::DocumentInspection;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use lurker_core_ports::profile_document_codec::ProfileDocumentSnippetOutcome;

impl CoreApplication {
    /// Inspect an editor-owned buffer through the configured engine codec.
    /// This synchronous path is reserved for pure format operations and does
    /// not enter the Core command queue or contact a running engine.
    pub fn inspect_profile_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.editor_command_handler()?
            .inspect_profile_buffer(identity, content)
    }

    /// Insert a codec-owned snippet into an editor buffer without mutating
    /// storage or runtime state.
    pub fn insert_profile_snippet(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.editor_command_handler()?
            .insert_profile_snippet(identity, content, snippet_id, line, column)
    }

    /// Run the composed document and Mixin codec preflight without applying
    /// the returned mutation to storage or a running engine.
    pub fn inspect_mixin_draft(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.editor_command_handler()?
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }

    pub fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.editor_command_handler()?
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    pub fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.editor_command_handler()?
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    pub fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, Failure> {
        self.editor_command_handler()?
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }

    fn editor_command_handler(&self) -> Result<Arc<dyn CommandHandler>, Failure> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "application is closed",
                false,
            ));
        }
        self.inner
            .command_handler
            .read()
            .expect("command handler lock")
            .clone()
            .ok_or_else(|| Failure::unsupported("profile editor codec is not composed"))
    }
}
