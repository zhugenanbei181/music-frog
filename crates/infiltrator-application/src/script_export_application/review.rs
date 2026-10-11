//! Preparation never calls the host; confirmation writes the exact reviewed bytes.
use super::{ExportReviewState, ExportState, ScriptExportApplication};
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportRequest, ScriptExportSnapshot,
};
use infiltrator_contract::script_export_review::{
    ScriptExportDraft, ScriptExportIdentity, ScriptExportReview, ScriptExportSaved,
    ScriptExportValidationScope,
};
use infiltrator_domain::script_export::{
    ExportArtifact, compose_directive_dsl_export, compose_extension_package_export,
    compose_mixin_overlay_export,
};
use infiltrator_domain::script_export_review::{content_sha256, draft_package};
use std::sync::Mutex;

struct SavingGuard<'a> {
    state: &'a Mutex<ExportState>,
    identity: ScriptExportIdentity,
}
impl Drop for SavingGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.state.lock().expect("script export state");
        let review = match &state.review_state {
            ExportReviewState::Saving(review) if review.identity == self.identity => review.clone(),
            _ => return,
        };
        state.review_state = ExportReviewState::Prepared(review);
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, false)
}
impl ScriptExportApplication {
    pub fn prepare(&self, draft: &ScriptExportDraft) -> Result<ScriptExportReview, Failure> {
        draft.validate()?;
        let artifact = match draft.kind {
            ScriptExportKind::MixinOverlayYaml => {
                return Err(Failure::unsupported(
                    "Mixin overlay export requires its profile options codec",
                ));
            }
            ScriptExportKind::DirectiveDslScript => compose_directive_dsl_export(
                draft.preset.as_deref(),
                &draft.script_code,
                draft.preset.as_deref(),
            ),
            ScriptExportKind::ExtensionPackageJson => {
                compose_extension_package_export(&draft_package(
                    draft
                        .preset
                        .as_deref()
                        .or(draft.profile.as_deref())
                        .unwrap_or("music-frog-extension"),
                    draft.profile.as_deref(),
                    &draft.script_code,
                    Some(&draft.mixin_yaml),
                    draft.preset.as_deref(),
                ))
            }
        }
        .map_err(|reason| Failure::new(ErrorCode::Configuration, reason, false))?;
        self.prepare_artifact(draft, artifact)
    }

    /// Prepare a Mixin overlay only after the engine-scoped profile options
    /// codec validates this exact identity and base/overlay buffer pair.
    pub fn prepare_mixin_overlay(
        &self,
        draft: &ScriptExportDraft,
        options: &ProfileOptionsApplication,
    ) -> Result<ScriptExportReview, Failure> {
        draft.validate()?;
        if draft.kind != ScriptExportKind::MixinOverlayYaml {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Draft is not a Mixin overlay export",
                false,
            ));
        }
        let identity = match &draft.validation_scope {
            ScriptExportValidationScope::ProfileDocument(identity) => identity,
            ScriptExportValidationScope::Unscoped => {
                return Err(Failure::unsupported(
                    "Mixin overlay export requires a profile document identity",
                ));
            }
        };
        options.inspect_mixin_draft(identity, &draft.base_yaml, &draft.mixin_yaml)?;
        let profile = draft
            .profile
            .as_deref()
            .ok_or_else(|| invalid("No source profile for overlay export"))?;
        let artifact = compose_mixin_overlay_export(profile, &draft.mixin_yaml)
            .map_err(|reason| Failure::new(ErrorCode::Configuration, reason, false))?;
        self.prepare_artifact(draft, artifact)
    }

    fn prepare_artifact(
        &self,
        draft: &ScriptExportDraft,
        artifact: ExportArtifact,
    ) -> Result<ScriptExportReview, Failure> {
        let mut state = self.state.lock().expect("script export state");
        if matches!(&state.review_state, ExportReviewState::Saving(_)) {
            return Err(invalid("Export write is pending"));
        }
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Export identity exhausted"))?;
        let review = ScriptExportReview {
            draft: draft.clone(),
            identity: ScriptExportIdentity {
                owner: self.owner,
                sequence,
                sha256: content_sha256(&artifact.content),
            },
            snapshot: ScriptExportSnapshot {
                kind: artifact.kind,
                profile: draft.profile.clone(),
                file_name: artifact.file_name,
                media_type: artifact.media_type,
                content: artifact.content,
                checksum: artifact.checksum,
                honest_note: artifact.honest_note,
                outcome: ScriptExportOutcome::Prepared,
            },
        };
        review.validate()?;
        state.sequence = sequence;
        state.latest = Some(review.snapshot.clone());
        state.review_state = ExportReviewState::Prepared(review.clone());
        Ok(review)
    }

    /// Cancel an unsaved review or dismiss a saved receipt from the shared
    /// projection. Dismissing a receipt never removes the host-written file.
    pub fn cancel(&self, identity: &ScriptExportIdentity) -> Result<(), Failure> {
        let mut state = self.state.lock().expect("script export state");
        match &state.review_state {
            ExportReviewState::Saving(_) => return Err(invalid("Export write is pending")),
            ExportReviewState::Prepared(review) if review.identity == *identity => {}
            ExportReviewState::Saved(saved) if saved.identity == *identity => {}
            _ => return Err(invalid("Export review expired")),
        }
        state.review_state = ExportReviewState::Empty;
        state.latest = None;
        Ok(())
    }

    pub fn confirm(&self, identity: &ScriptExportIdentity) -> Result<ScriptExportSaved, Failure> {
        let port = self
            .port
            .as_ref()
            .ok_or_else(|| Failure::unsupported("Script export host is not composed"))?;
        let review = {
            let mut state = self.state.lock().expect("script export state");
            let review = match &state.review_state {
                ExportReviewState::Saving(_) => return Err(invalid("Export write is pending")),
                ExportReviewState::Prepared(review) if review.identity == *identity => {
                    review.clone()
                }
                ExportReviewState::Saved(saved) if saved.identity == *identity => {
                    return Ok(saved.clone());
                }
                _ => return Err(invalid("Export review expired")),
            };
            review.validate()?;
            if content_sha256(&review.snapshot.content) != identity.sha256 {
                return Err(invalid("Reviewed export content changed"));
            }
            state.review_state = ExportReviewState::Saving(review.clone());
            review
        };
        let _saving = SavingGuard {
            state: &self.state,
            identity: identity.clone(),
        };
        let snapshot = review.snapshot;
        let request = ScriptExportRequest {
            kind: snapshot.kind,
            file_name: snapshot.file_name.clone(),
            media_type: snapshot.media_type.clone(),
            content: snapshot.content.clone(),
        };
        let receipt = port.save_export(&request).map_err(Failure::from)?;
        let saved = ScriptExportSaved {
            identity: identity.clone(),
            snapshot: ScriptExportSnapshot {
                outcome: ScriptExportOutcome::Saved {
                    path: receipt.path,
                    bytes_written: receipt.bytes_written,
                },
                ..snapshot
            },
        };
        saved.validate(identity)?;
        let mut state = self.state.lock().expect("script export state");
        state.latest = Some(saved.snapshot.clone());
        state.review_state = ExportReviewState::Saved(saved.clone());
        Ok(saved)
    }
}
