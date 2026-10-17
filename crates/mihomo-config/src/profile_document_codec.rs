//! Mihomo YAML document inspection adapter.

use crate::{profile_configuration_codec, profile_options_codec};
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::{SnippetCaret, insert_at_caret};
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationCodecPort, ProfileConfigurationMutation, ProfileConfigurationQuery,
    ProfileConfigurationReadout, ProfileConfigurationUpdate, ProfileFilterMutation,
    ProfileMixinMutation, ProfileOptionsCodecScope, ProfileOptionsProjection,
    ProfileRuleWorkspaceMutation, ProfileRuleWorkspaceProjection, ProfileRuleWorkspaceUpdate,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentCodecScope,
    ProfileDocumentSnippetInsertion, ProfileDocumentSnippetOutcome,
};
use std::str::from_utf8;
use yaml_rust2::YamlLoader;

pub(crate) const PROFILE_FORMAT: &str = "mihomo-yaml";
pub(crate) const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct MihomoProfileDocumentCodec;

impl ProfileDocumentCodecPort for MihomoProfileDocumentCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        ProfileDocumentCodecScope {
            engine_id: crate::engine_id(),
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
        let previous = self.inspect(content.as_bytes())?;
        let insertion = insert_at_caret(
            content,
            snippet_id,
            SnippetCaret {
                line: caret.line,
                column: caret.column,
            },
        )
        .map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
        let validation = self.inspect(insertion.content.as_bytes())?;
        if matches!(&previous, DocumentValidation::Valid)
            && matches!(&validation, DocumentValidation::Invalid { .. })
        {
            return Ok(ProfileDocumentSnippetOutcome::Rejected { validation });
        }
        Ok(ProfileDocumentSnippetOutcome::Inserted(
            ProfileDocumentSnippetInsertion {
                snippet_id: insertion.snippet_id.to_owned(),
                content: insertion.content,
                cursor_line: insertion.cursor_line,
                cursor_column: insertion.cursor_column,
                validation,
            },
        ))
    }
}

impl ProfileConfigurationCodecPort for MihomoProfileDocumentCodec {
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
