use super::ProfileApplication;
use infiltrator_contract::profile_document::{
    DocumentDiagnosticSnapshot, DocumentInspection, DocumentInspectionUnavailableReason,
    DocumentInspectionUnsupportedReason, DocumentValidation,
};
use infiltrator_contract::profile_document_identity::{
    ProfileDocument, ProfileDocumentIdentity, ProfileWorkspaceIdentity,
};
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationQuery, ProfileConfigurationReadout,
    ProfileConfigurationUpdate, ProfileDocumentCaret, ProfileDocumentCodecPort,
    ProfileDocumentCodecScope, ProfileDocumentSnippetOutcome, ProfileFilterMutation,
    ProfileMixinMutation, ProfileOptionsCodecScope, ProfileOptionsProjection,
    ProfileRuleWorkspaceMutation, ProfileRuleWorkspaceProjection, ProfileRuleWorkspaceUpdate,
};
use infiltrator_ports::profile_workspace::ProfileWorkspace;
use infiltrator_ports::rule_tracer::RuleWorkspace;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::sync::Arc;

impl ProfileApplication {
    pub fn with_document_codec(mut self, codec: Arc<dyn ProfileDocumentCodecPort>) -> Self {
        self.document_codec = Some(codec);
        self
    }

    pub fn inspect_document(&self, document: &ProfileDocument) -> DocumentInspection {
        self.inspect_document_buffer(&document.identity, &document.raw_bytes)
    }

    /// Inspect an editor buffer with the codec selected by its immutable
    /// engine/format/schema identity. This is pure and performs no profile or
    /// runtime I/O, so surfaces can request a live preflight as text changes.
    pub fn inspect_document_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
    ) -> DocumentInspection {
        let store_engine = self.store.engine_id();
        if identity.engine_id != store_engine {
            return DocumentInspection::Unsupported(
                DocumentInspectionUnsupportedReason::EngineMismatch {
                    document: identity.engine_id.clone(),
                    expected: store_engine,
                },
            );
        }
        let Some(codec) = &self.document_codec else {
            return DocumentInspection::Unavailable(
                DocumentInspectionUnavailableReason::CodecMissing,
            );
        };
        let scope = codec.scope();
        if let Some(reason) = scope_mismatch(identity, &scope) {
            return DocumentInspection::Unsupported(reason);
        }
        match codec.inspect(raw_bytes) {
            Ok(validation) => DocumentInspection::Inspected(validation),
            Err(error) => DocumentInspection::Unavailable(
                DocumentInspectionUnavailableReason::AdapterUnavailable {
                    detail: error.to_string(),
                },
            ),
        }
    }

    /// Apply a snippet through the scoped codec. The adapter owns snippet
    /// syntax and its local validation; the application only checks scope and
    /// transports neutral diagnostics.
    pub fn insert_document_snippet(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        caret: ProfileDocumentCaret,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        let store_engine = self.store.engine_id();
        if identity.engine_id != store_engine {
            return Err(inspection_unsupported_failure(
                DocumentInspectionUnsupportedReason::EngineMismatch {
                    document: identity.engine_id.clone(),
                    expected: store_engine,
                },
            ));
        }
        let Some(codec) = &self.document_codec else {
            return Err(inspection_unavailable_failure(
                DocumentInspectionUnavailableReason::CodecMissing,
            ));
        };
        let scope = codec.scope();
        if let Some(reason) = scope_mismatch(identity, &scope) {
            return Err(inspection_unsupported_failure(reason));
        }
        codec
            .insert_snippet(content, snippet_id, caret)
            .map_err(Failure::from)
    }

    pub fn inspect_profile_options(
        &self,
        identity: &ProfileDocumentIdentity,
        sidecar_bytes: Option<&[u8]>,
    ) -> Result<ProfileOptionsProjection, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .inspect_options(identity, sidecar_bytes)
            .map_err(Failure::from)
    }

    pub fn apply_profile_mixin(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinMutation, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .apply_mixin(identity, document_bytes, sidecar_bytes, mixin_yaml)
            .map_err(Failure::from)
    }

    pub fn inspect_profile_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
            .map_err(Failure::from)
    }

    pub fn set_profile_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
            .map_err(Failure::from)
    }

    pub fn inspect_profile_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .inspect_mixin_studio(identity, document_bytes, mixin_yaml)
            .map_err(Failure::from)
    }

    pub fn apply_profile_filter(
        &self,
        identity: &ProfileDocumentIdentity,
        document_bytes: &[u8],
        sidecar_bytes: Option<&[u8]>,
        draft: &SubscriptionFilterDraft,
    ) -> Result<ProfileFilterMutation, Failure> {
        let codec = self.options_codec_for(identity)?;
        codec
            .apply_filter(identity, document_bytes, sidecar_bytes, draft)
            .map_err(Failure::from)
    }

    /// Read an engine-authored typed configuration view from an exact profile
    /// document identity. The application validates scope before calling the
    /// adapter, and the adapter owns the document format grammar.
    pub fn read_profile_configuration(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        query: ProfileConfigurationQuery,
    ) -> Result<ProfileConfigurationReadout, Failure> {
        let codec = self.configuration_codec_for(identity)?;
        codec
            .read_configuration(identity, raw_bytes, query)
            .map_err(Failure::from)
    }

    /// Transform typed configuration input without storage or runtime side
    /// effects. The returned readback was decoded from the returned bytes.
    pub fn apply_profile_configuration(
        &self,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        update: ProfileConfigurationUpdate,
    ) -> Result<ProfileConfigurationMutation, Failure> {
        let codec = self.configuration_codec_for(identity)?;
        codec
            .apply_configuration(identity, raw_bytes, update)
            .map_err(Failure::from)
    }

    /// Project the Mihomo-owned rules/sub-rules schema from one coherent
    /// document/sidecar workspace, preserving its exact source identity.
    pub fn read_profile_rule_workspace(
        &self,
        workspace: &ProfileWorkspace,
    ) -> Result<RuleWorkspace, Failure> {
        let identity = workspace.identity();
        let codec = self.configuration_codec_for(&workspace.document.identity)?;
        let ProfileRuleWorkspaceProjection {
            source,
            rules,
            targets,
            sub_rules,
        } = codec
            .read_rule_workspace(&identity, &workspace.document.raw_bytes)
            .map_err(Failure::from)?;
        Ok(RuleWorkspace {
            source,
            profile_identity: identity,
            rules,
            targets,
            sub_rules,
        })
    }

    /// Apply a typed rule workspace edit through the selected format adapter.
    /// No document parsing or profile/runtime side effect occurs here.
    pub fn apply_profile_rule_workspace(
        &self,
        identity: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
        update: ProfileRuleWorkspaceUpdate,
    ) -> Result<ProfileRuleWorkspaceMutation, Failure> {
        let codec = self.configuration_codec_for(&identity.document)?;
        codec
            .apply_rule_workspace(identity, raw_bytes, update)
            .map_err(Failure::from)
    }

    /// Validate the selected options codec scope without reading profile
    /// storage or decoding sidecar bytes.
    pub fn validate_profile_options_scope(
        &self,
        identity: &ProfileDocumentIdentity,
    ) -> Result<(), Failure> {
        self.options_codec_for(identity).map(|_| ())
    }

    fn options_codec_for(
        &self,
        identity: &ProfileDocumentIdentity,
    ) -> Result<Arc<dyn ProfileDocumentCodecPort>, Failure> {
        let store_engine = self.store.engine_id();
        if identity.engine_id != store_engine {
            return Err(inspection_unsupported_failure(
                DocumentInspectionUnsupportedReason::EngineMismatch {
                    document: identity.engine_id.clone(),
                    expected: store_engine,
                },
            ));
        }
        let Some(codec) = &self.document_codec else {
            return Err(inspection_unavailable_failure(
                DocumentInspectionUnavailableReason::CodecMissing,
            ));
        };
        let document_scope = codec.scope();
        if let Some(reason) = scope_mismatch(identity, &document_scope) {
            return Err(inspection_unsupported_failure(reason));
        }
        let options_scope: ProfileOptionsCodecScope =
            codec.options_scope().map_err(Failure::from)?;
        if options_scope.document != document_scope {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "profile codec reported inconsistent document and sidecar scopes",
                false,
            ));
        }
        if let Some(reason) = scope_mismatch(identity, &options_scope.document) {
            return Err(inspection_unsupported_failure(reason));
        }
        Ok(codec.clone())
    }

    fn configuration_codec_for(
        &self,
        identity: &ProfileDocumentIdentity,
    ) -> Result<Arc<dyn ProfileDocumentCodecPort>, Failure> {
        let store_engine = self.store.engine_id();
        if identity.engine_id != store_engine {
            return Err(inspection_unsupported_failure(
                DocumentInspectionUnsupportedReason::EngineMismatch {
                    document: identity.engine_id.clone(),
                    expected: store_engine,
                },
            ));
        }
        let Some(codec) = &self.document_codec else {
            return Err(inspection_unavailable_failure(
                DocumentInspectionUnavailableReason::CodecMissing,
            ));
        };
        let scope = codec.scope();
        if let Some(reason) = scope_mismatch(identity, &scope) {
            return Err(inspection_unsupported_failure(reason));
        }
        Ok(codec.clone())
    }

    pub fn validate_document_write(
        &self,
        expected: &ProfileWorkspaceIdentity,
        raw_bytes: &[u8],
    ) -> Result<(), Failure> {
        self.validate_expected_engine(expected)?;
        let Some(codec) = &self.document_codec else {
            return Err(inspection_unavailable_failure(
                DocumentInspectionUnavailableReason::CodecMissing,
            ));
        };
        let scope = codec.scope();
        if let Some(reason) = scope_mismatch(&expected.document, &scope) {
            return Err(inspection_unsupported_failure(reason));
        }
        let validation = codec.inspect(raw_bytes).map_err(|error| {
            inspection_unavailable_failure(
                DocumentInspectionUnavailableReason::AdapterUnavailable {
                    detail: error.to_string(),
                },
            )
        })?;
        match validation {
            DocumentValidation::Valid => Ok(()),
            DocumentValidation::Invalid { diagnostics } => Err(Failure::new(
                ErrorCode::Configuration,
                validation_message(&diagnostics),
                false,
            )),
        }
    }
}

fn scope_mismatch(
    identity: &ProfileDocumentIdentity,
    scope: &ProfileDocumentCodecScope,
) -> Option<DocumentInspectionUnsupportedReason> {
    if identity.engine_id != scope.engine_id {
        return Some(DocumentInspectionUnsupportedReason::EngineMismatch {
            document: identity.engine_id.clone(),
            expected: scope.engine_id.clone(),
        });
    }
    if identity.format_id != scope.format_id {
        return Some(DocumentInspectionUnsupportedReason::FormatMismatch {
            document: identity.format_id.clone(),
            codec: scope.format_id.clone(),
        });
    }
    if identity.schema_version != scope.schema_version {
        return Some(DocumentInspectionUnsupportedReason::SchemaMismatch {
            document: identity.schema_version,
            codec: scope.schema_version,
        });
    }
    None
}

fn inspection_unavailable_failure(reason: DocumentInspectionUnavailableReason) -> Failure {
    let code = match &reason {
        DocumentInspectionUnavailableReason::AdapterUnavailable { .. } => ErrorCode::NotReady,
        DocumentInspectionUnavailableReason::CodecMissing => ErrorCode::Unsupported,
    };
    Failure::new(
        code,
        format!("profile document validation is unavailable: {reason:?}"),
        false,
    )
}

fn inspection_unsupported_failure(reason: DocumentInspectionUnsupportedReason) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!("profile document codec does not support this identity: {reason:?}"),
        false,
    )
}

fn validation_message(diagnostics: &[DocumentDiagnosticSnapshot]) -> String {
    if diagnostics.is_empty() {
        return "profile document validation failed".to_owned();
    }
    diagnostics
        .iter()
        .map(|diagnostic| match (diagnostic.line, diagnostic.column) {
            (Some(line), Some(column)) => {
                format!("line {line}, column {column}: {}", diagnostic.message)
            }
            _ => diagnostic.message.clone(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}
