//! Shared source and terminal-result state for the two native YAML editors.
use crate::failure_projection::failure_message;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Baseline {
    source: ProfileSourceIdentity,
    content: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEditPending {
    pub operation: u64,
    pub intent: CommandIntent,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditSession {
    baseline: Option<Baseline>,
    latest: Option<Baseline>,
    pub pending: Option<ProfileEditPending>,
    pub failure: Option<Failure>,
    pub saved: bool,
    read_status: Option<ProfileReadStatus>,
    verification_current: bool,
    verification_failure: Option<Failure>,
    next_operation: u64,
}
impl ProfileEditSession {
    pub fn source(&self) -> Option<&ProfileSourceIdentity> {
        self.baseline.as_ref().map(|baseline| &baseline.source)
    }
    pub fn stale(&self) -> bool {
        self.baseline.as_ref().map(|baseline| &baseline.source)
            != self.latest.as_ref().map(|latest| &latest.source)
    }
    pub fn can_edit(&self) -> bool {
        self.baseline.is_some() && self.pending.is_none()
    }
    pub fn can_save(&self) -> bool {
        self.can_edit()
            && !self.stale()
            && self
                .read_status
                .as_ref()
                .is_none_or(|status| matches!(status, ProfileReadStatus::Ready))
            && (self.read_status.is_none() || self.verification_current)
    }
    pub fn dirty(&self, draft: &str) -> bool {
        self.baseline
            .as_ref()
            .is_some_and(|baseline| baseline.content != draft)
    }
    /// Return whether the native buffer may adopt this read without losing a draft.
    pub fn observe(&mut self, source: &ProfileSourceIdentity, content: &str, draft: &str) -> bool {
        let latest = Baseline {
            source: source.clone(),
            content: content.into(),
        };
        self.latest = Some(latest.clone());
        if self.pending.is_some() || self.dirty(draft) {
            return false;
        }
        let changed = self.baseline.as_ref() != Some(&latest);
        if changed {
            self.saved = false;
        }
        self.baseline = Some(latest);
        changed
    }
    pub fn read_failed(&mut self, failure: Failure) {
        self.read_status = Some(ProfileReadStatus::Failed(failure));
        self.verification_current = false;
    }
    pub fn read_completed(&mut self, source: &ProfileSourceIdentity) {
        self.read_status = Some(ProfileReadStatus::Ready);
        self.verification_current = self.source() == Some(source);
        self.verification_failure = None;
    }
    pub fn observe_read_status(&mut self, snapshot: &ProfileEditorReadSnapshot, options: bool) {
        if snapshot.profile.as_ref() != self.source().map(|source| &source.profile) {
            return;
        }
        self.read_status = Some(if options {
            snapshot.options.clone()
        } else {
            snapshot.document.clone()
        });
        self.verification_current =
            snapshot.source_current() && snapshot.verified_source.as_ref() == self.source();
        self.verification_failure = snapshot.verification_failure.clone();
    }
    pub fn discard(&mut self) -> Option<String> {
        if self.pending.is_some() {
            return None;
        }
        self.baseline = self.latest.clone();
        self.failure = None;
        self.saved = false;
        self.baseline
            .as_ref()
            .map(|baseline| baseline.content.clone())
    }
    pub fn begin_document(
        &mut self,
        content: String,
        allow_protected: bool,
    ) -> Result<ProfileEditPending, Failure> {
        let source = self.ready_source()?;
        self.begin(CommandIntent::SaveProfileDocument {
            source,
            content,
            allow_protected,
        })
    }
    pub fn begin_mixin(&mut self, mixin_yaml: String) -> Result<ProfileEditPending, Failure> {
        let source = self.ready_source()?;
        self.begin(CommandIntent::SaveMixinOverlay { source, mixin_yaml })
    }
    fn ready_source(&self) -> Result<ProfileSourceIdentity, Failure> {
        if !self.can_save() {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Inspect the current source before saving",
                true,
            ));
        }
        Ok(self
            .source()
            .expect("save requires an observed source")
            .clone())
    }
    fn begin(&mut self, intent: CommandIntent) -> Result<ProfileEditPending, Failure> {
        self.next_operation = self.next_operation.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::Internal,
                "Editor operation identity exhausted",
                false,
            )
        })?;
        let pending = ProfileEditPending {
            operation: self.next_operation,
            intent,
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        self.saved = false;
        Ok(pending)
    }
    pub fn finish(
        &mut self,
        operation: u64,
        intent: &CommandIntent,
        result: Result<CommandOutput, Failure>,
    ) -> bool {
        if self
            .pending
            .as_ref()
            .is_none_or(|pending| pending.operation != operation || pending.intent != *intent)
        {
            return false;
        }
        self.pending = None;
        let output = match result.and_then(|output| output.validate_for(intent).map(|()| output)) {
            Ok(output) => output,
            Err(failure) => {
                self.failure = Some(failure);
                return true;
            }
        };
        let baseline = match output {
            CommandOutput::ProfileDocumentSaved(saved) => Baseline {
                source: saved.document.source.expect("validated document source"),
                content: saved.document.content,
            },
            CommandOutput::ProfileMixinSaved(saved) => Baseline {
                source: saved.options.source,
                content: saved.options.mixin_yaml,
            },
            _ => unreachable!("only editor save intents enter this state"),
        };
        // A newer independent observation is retained and keeps this receipt visibly stale.
        let old_source = self.baseline.as_ref().map(|old| &old.source);
        if self.latest.as_ref().map(|latest| &latest.source) == old_source {
            self.latest = Some(baseline.clone());
        }
        self.baseline = Some(baseline);
        self.read_status = None;
        self.verification_current = false;
        self.verification_failure = None;
        self.failure = None;
        self.saved = true;
        true
    }
    pub fn status(&self, draft: &str, locale: &str) -> String {
        if self.pending.is_some() {
            return localize(locale, "editor_save_pending", &[]);
        }
        if let Some(failure) = &self.failure {
            return localize(
                locale,
                "editor_save_failed",
                &[("reason", failure_message(failure, locale))],
            );
        }
        if let Some(ProfileReadStatus::Failed(failure)) = &self.read_status {
            return localize(
                locale,
                "editor_read_failed",
                &[("reason", failure_message(failure, locale))],
            );
        }
        if matches!(self.read_status, Some(ProfileReadStatus::Loading)) {
            return localize(locale, "editor_read_loading", &[]);
        }
        if let Some(failure) = &self.verification_failure {
            return localize(
                locale,
                "editor_source_verification_failed",
                &[("reason", failure_message(failure, locale))],
            );
        }
        if self.read_status.is_some() && !self.verification_current {
            return localize(locale, "editor_save_source_changed", &[]);
        }
        if self.stale() {
            return localize(locale, "editor_save_source_changed", &[]);
        }
        if self.baseline.is_none() {
            return localize(locale, "editor_save_unobserved", &[]);
        }
        if self.dirty(draft) {
            return localize(locale, "editor_unsaved", &[]);
        }
        if self.saved {
            return localize(locale, "editor_save_committed", &[]);
        }
        String::new()
    }
}
