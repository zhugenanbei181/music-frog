//! Preparation never calls the host; confirmation writes the exact reviewed bytes.
use super::{ExportState, ScriptExportApplication};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportRequest, ScriptExportSnapshot,
};
use infiltrator_contract::script_export_review::{
    ScriptExportDraft, ScriptExportIdentity, ScriptExportReview, ScriptExportSaved,
};
use infiltrator_domain::script_export::{
    compose_directive_dsl_export, compose_extension_package_export, compose_mixin_overlay_export,
};
use infiltrator_domain::script_export_review::{content_sha256, draft_package};
use std::sync::Mutex;

struct SavingGuard<'a>(&'a Mutex<ExportState>);
impl Drop for SavingGuard<'_> {
    fn drop(&mut self) {
        self.0.lock().expect("script export state").saving = false;
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, false)
}
impl ScriptExportApplication {
    pub fn prepare(&self, draft: &ScriptExportDraft) -> Result<ScriptExportReview, Failure> {
        draft.validate()?;
        let artifact = match draft.kind {
            ScriptExportKind::MixinOverlayYaml => compose_mixin_overlay_export(
                draft
                    .profile
                    .as_deref()
                    .ok_or_else(|| invalid("No source profile for overlay export"))?,
                &draft.base_yaml,
                &draft.mixin_yaml,
            ),
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
        let mut state = self.state.lock().expect("script export state");
        if state.saving {
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
        state.review = Some(review.clone());
        state.saved = None;
        Ok(review)
    }

    pub fn cancel(&self, identity: &ScriptExportIdentity) -> Result<(), Failure> {
        let mut state = self.state.lock().expect("script export state");
        if state.saving {
            return Err(invalid("Export write is pending"));
        }
        if state
            .review
            .as_ref()
            .is_none_or(|review| review.identity != *identity)
        {
            return Err(invalid("Export review expired"));
        }
        state.review = None;
        state.saved = None;
        state.latest = None;
        Ok(())
    }

    pub fn confirm(&self, identity: &ScriptExportIdentity) -> Result<ScriptExportSaved, Failure> {
        let review = {
            let mut state = self.state.lock().expect("script export state");
            if state.saving {
                return Err(invalid("Export write is pending"));
            }
            let review = state
                .review
                .as_ref()
                .filter(|review| review.identity == *identity)
                .ok_or_else(|| invalid("Export review expired"))?
                .clone();
            review.validate()?;
            if content_sha256(&review.snapshot.content) != identity.sha256 {
                return Err(invalid("Reviewed export content changed"));
            }
            if let Some(saved) = &state.saved {
                return Ok(saved.clone());
            }
            state.saving = true;
            review
        };
        let _saving = SavingGuard(&self.state);
        let port = self
            .port
            .as_ref()
            .ok_or_else(|| Failure::unsupported("Script export host is not composed"))?;
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
        state.saved = Some(saved.clone());
        Ok(saved)
    }
}
