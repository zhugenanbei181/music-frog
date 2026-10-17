//! Product-level typed configuration extension for the Mihomo YAML codec.
//!
//! The neutral inspection implementation (`scope`/`inspect`/`insert_snippet`)
//! is owned by `mihomo_driver::codec::profile_document_codec`. This newtype
//! only exists because the product `ProfileConfigurationCodecPort` extension is
//! a foreign trait: the orphan rule requires a product-local type. It delegates
//! every neutral operation to that single driver implementation and only adds
//! the product-owned typed workspace, mixin and filter operations.
//!
//! The shared editor snippet catalogue is a product/UI asset, so this adapter
//! injects it into the neutral driver codec as data; the driver owns the splice
//! mechanics but no snippet body.

use crate::{profile_configuration_codec, profile_options_codec};
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::yaml_snippet;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationCodecPort, ProfileConfigurationMutation, ProfileConfigurationQuery,
    ProfileConfigurationReadout, ProfileConfigurationUpdate, ProfileFilterMutation,
    ProfileMixinMutation, ProfileOptionsCodecScope, ProfileOptionsProjection,
    ProfileRuleWorkspaceMutation, ProfileRuleWorkspaceProjection, ProfileRuleWorkspaceUpdate,
};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileWorkspaceIdentity,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    ProfileDocumentSnippetOutcome,
};
use mihomo_driver::codec::profile_document_codec::MihomoProfileDocumentCodec;

#[derive(Clone, Copy, Debug, Default)]
pub struct MihomoProfileConfigurationCodec;

impl MihomoProfileConfigurationCodec {
    pub const fn new() -> Self {
        Self
    }

    /// The driver codec with the shared snippet catalogue injected.
    const fn document_codec() -> MihomoProfileDocumentCodec {
        MihomoProfileDocumentCodec::with_snippets(snippet_body)
    }
}

fn snippet_body(snippet_id: &str) -> Option<&'static str> {
    yaml_snippet(snippet_id).map(|snippet| snippet.body)
}

impl ProfileDocumentCodecPort for MihomoProfileConfigurationCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        Self::document_codec().scope()
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        Self::document_codec().inspect(raw_bytes)
    }

    fn insert_snippet(
        &self,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, PortError> {
        Self::document_codec().insert_snippet(content, snippet_id, caret)
    }
}

impl ProfileConfigurationCodecPort for MihomoProfileConfigurationCodec {
    fn read_configuration(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        query: ProfileConfigurationQuery,
    ) -> Result<ProfileConfigurationReadout, PortError> {
        profile_configuration_codec::read(identity, raw_bytes, query)
    }

    fn apply_configuration(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        update: ProfileConfigurationUpdate,
    ) -> Result<ProfileConfigurationMutation, PortError> {
        profile_configuration_codec::apply(identity, raw_bytes, update)
    }

    fn read_rule_workspace(
        &self,
        workspace: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
    ) -> Result<ProfileRuleWorkspaceProjection, PortError> {
        profile_configuration_codec::read_rule_workspace(workspace, raw_bytes)
    }

    fn apply_rule_workspace(
        &self,
        workspace: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
        update: ProfileRuleWorkspaceUpdate,
    ) -> Result<ProfileRuleWorkspaceMutation, PortError> {
        profile_configuration_codec::apply_rule_workspace(workspace, raw_bytes, update)
    }

    fn options_scope(&self) -> Result<ProfileOptionsCodecScope, PortError> {
        Ok(profile_options_codec::scope(self.scope()))
    }

    fn inspect_options(
        &self,
        identity: &ProfileDocumentIdentity,
        sidecar_bytes: Option<&[u8]>,
    ) -> Result<ProfileOptionsProjection, PortError> {
        profile_options_codec::inspect(identity, sidecar_bytes)
    }

    fn apply_mixin(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinMutation, PortError> {
        profile_options_codec::apply_mixin(identity, document_bytes, sidecar_bytes, mixin_yaml)
    }

    fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, PortError> {
        profile_options_codec::inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, PortError> {
        profile_options_codec::set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, PortError> {
        profile_options_codec::inspect_mixin_studio(identity, document_bytes, mixin_yaml)
    }

    fn apply_filter(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        draft: &SubscriptionFilterDraft,
    ) -> Result<ProfileFilterMutation, PortError> {
        profile_options_codec::apply_filter(identity, document_bytes, sidecar_bytes, draft)
    }
}
