//! Shared source and terminal-result state for the two native YAML editors.
use crate::failure_projection::failure_message;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Baseline {
    source: ProfileWorkspaceIdentity,
    content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEditPending {
    pub operation: u64,
    pub intent: CommandIntent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProfileEditOperationPhase {
    #[default]
    Ready,
    Pending(ProfileEditPending),
    Failed(Failure),
    Saved,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum ProfileEditVerification {
    #[default]
    Unchecked,
    Current,
    Stale,
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileEditObservation {
    DraftProtected,
    Unchanged,
    Updated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileEditFinish {
    Stale,
    Applied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileEditReadTarget {
    Document,
    Options,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditSession {
    baseline: Option<Baseline>,
    latest: Option<Baseline>,
    pub operation: ProfileEditOperationPhase,
    read_status: ProfileReadStatus,
    verification: ProfileEditVerification,
    next_operation: u64,
}

impl ProfileEditSession {
    pub fn source(&self) -> Option<&ProfileWorkspaceIdentity> {
        self.baseline.as_ref().map(|baseline| &baseline.source)
    }

    pub fn stale(&self) -> bool {
        self.baseline.as_ref().map(|baseline| &baseline.source)
            != self.latest.as_ref().map(|latest| &latest.source)
    }

    pub fn can_edit(&self) -> bool {
        self.baseline.is_some() && !matches!(&self.operation, ProfileEditOperationPhase::Pending(_))
    }

    pub fn can_save(&self) -> bool {
        self.can_edit()
            && !self.stale()
            && match (&self.read_status, &self.verification) {
                (
                    ProfileReadStatus::Unobserved,
                    ProfileEditVerification::Unchecked | ProfileEditVerification::Current,
                )
                | (ProfileReadStatus::Ready, ProfileEditVerification::Current) => true,
                (
                    ProfileReadStatus::Unobserved,
                    ProfileEditVerification::Stale | ProfileEditVerification::Failed(_),
                )
                | (
                    ProfileReadStatus::Ready,
                    ProfileEditVerification::Unchecked
                    | ProfileEditVerification::Stale
                    | ProfileEditVerification::Failed(_),
                )
                | (ProfileReadStatus::Loading | ProfileReadStatus::Failed(_), _) => false,
            }
    }

    pub fn dirty(&self, draft: &str) -> bool {
        self.baseline
            .as_ref()
            .is_some_and(|baseline| baseline.content != draft)
    }

    /// Keep incoming source facts while protecting a pending operation or an edited draft.
    pub fn observe(
        &mut self,
        source: &ProfileWorkspaceIdentity,
        content: &str,
        draft: &str,
    ) -> ProfileEditObservation {
        let latest = Baseline {
            source: source.clone(),
            content: content.into(),
        };
        self.latest = Some(latest.clone());
        if matches!(&self.operation, ProfileEditOperationPhase::Pending(_)) || self.dirty(draft) {
            return ProfileEditObservation::DraftProtected;
        }
        let changed = self.baseline.as_ref() != Some(&latest);
        if changed {
            if matches!(&self.operation, ProfileEditOperationPhase::Saved) {
                self.operation = ProfileEditOperationPhase::Ready;
            }
            self.baseline = Some(latest);
        }
        match changed {
            true => ProfileEditObservation::Updated,
            false => ProfileEditObservation::Unchanged,
        }
    }

    pub fn read_failed(&mut self, failure: Failure) {
        self.read_status = ProfileReadStatus::Failed(failure.clone());
        self.verification = ProfileEditVerification::Failed(failure);
    }

    pub fn read_completed(&mut self, source: &ProfileWorkspaceIdentity) {
        self.read_status = ProfileReadStatus::Ready;
        self.verification = match self.source() == Some(source) {
            true => ProfileEditVerification::Current,
            false => ProfileEditVerification::Stale,
        };
    }

    pub fn observe_read_status(
        &mut self,
        snapshot: &ProfileEditorReadSnapshot,
        target: ProfileEditReadTarget,
    ) {
        if snapshot.profile.as_deref()
            != self
                .source()
                .map(|source| source.document.profile_id.as_str())
        {
            return;
        }
        self.read_status = match target {
            ProfileEditReadTarget::Document => snapshot.document.clone(),
            ProfileEditReadTarget::Options => snapshot.options.clone(),
        };
        self.verification = match &snapshot.verification_failure {
            Some(failure) => ProfileEditVerification::Failed(failure.clone()),
            None if snapshot.source_current()
                && snapshot.verified_workspace_identity.as_ref() == self.source() =>
            {
                ProfileEditVerification::Current
            }
            None => ProfileEditVerification::Stale,
        };
    }

    pub fn discard(&mut self) -> Option<String> {
        if matches!(&self.operation, ProfileEditOperationPhase::Pending(_)) {
            return None;
        }
        self.baseline = self.latest.clone();
        self.operation = ProfileEditOperationPhase::Ready;
        self.baseline
            .as_ref()
            .map(|baseline| baseline.content.clone())
    }

    pub fn begin_document(
        &mut self,
        content: String,
        authorization: ProfileWriteAuthorization,
    ) -> Result<ProfileEditPending, Failure> {
        let workspace_identity = self.ready_source()?;
        self.begin(CommandIntent::SaveProfileDocument {
            workspace_identity,
            content,
            authorization,
        })
    }

    pub fn begin_mixin(&mut self, mixin_yaml: String) -> Result<ProfileEditPending, Failure> {
        let workspace_identity = self.ready_source()?;
        self.begin(CommandIntent::SaveMixinOverlay {
            workspace_identity,
            mixin_yaml,
        })
    }

    pub fn reject(&mut self, failure: Failure) {
        self.operation = ProfileEditOperationPhase::Failed(failure);
    }

    fn ready_source(&self) -> Result<ProfileWorkspaceIdentity, Failure> {
        if matches!(&self.operation, ProfileEditOperationPhase::Pending(_)) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "editor operation is already pending",
                true,
            ));
        }
        let baseline = self.baseline.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Inspect the current source before saving",
                true,
            )
        })?;
        if self.latest.as_ref().map(|latest| &latest.source) != Some(&baseline.source) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The profile source changed; inspect the current source before saving",
                true,
            ));
        }
        match (&self.read_status, &self.verification) {
            (
                ProfileReadStatus::Unobserved,
                ProfileEditVerification::Unchecked | ProfileEditVerification::Current,
            )
            | (ProfileReadStatus::Ready, ProfileEditVerification::Current) => {}
            (
                ProfileReadStatus::Unobserved,
                ProfileEditVerification::Stale | ProfileEditVerification::Failed(_),
            )
            | (
                ProfileReadStatus::Ready,
                ProfileEditVerification::Unchecked
                | ProfileEditVerification::Stale
                | ProfileEditVerification::Failed(_),
            )
            | (ProfileReadStatus::Loading | ProfileReadStatus::Failed(_), _) => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "Verify the current source before saving",
                    true,
                ));
            }
        }
        Ok(baseline.source.clone())
    }

    fn begin(&mut self, intent: CommandIntent) -> Result<ProfileEditPending, Failure> {
        if matches!(&self.operation, ProfileEditOperationPhase::Pending(_)) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "editor operation is already pending",
                true,
            ));
        }
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
        self.operation = ProfileEditOperationPhase::Pending(pending.clone());
        Ok(pending)
    }

    pub fn finish(
        &mut self,
        operation: u64,
        intent: &CommandIntent,
        result: Result<CommandOutput, Failure>,
    ) -> ProfileEditFinish {
        match &self.operation {
            ProfileEditOperationPhase::Pending(pending)
                if pending.operation == operation && pending.intent == *intent => {}
            ProfileEditOperationPhase::Ready
            | ProfileEditOperationPhase::Pending(_)
            | ProfileEditOperationPhase::Failed(_)
            | ProfileEditOperationPhase::Saved => return ProfileEditFinish::Stale,
        };
        let output = match result.and_then(|output| output.validate_for(intent).map(|()| output)) {
            Ok(output) => output,
            Err(failure) => {
                self.operation = ProfileEditOperationPhase::Failed(failure);
                return ProfileEditFinish::Applied;
            }
        };
        let baseline = match output {
            CommandOutput::ProfileDocumentSaved(saved) => Baseline {
                source: saved.document.workspace_identity,
                content: saved.document.content,
            },
            CommandOutput::ProfileMixinSaved(saved) => Baseline {
                source: saved.options.workspace_identity,
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
        self.read_status = ProfileReadStatus::Unobserved;
        self.verification = ProfileEditVerification::Unchecked;
        self.operation = ProfileEditOperationPhase::Saved;
        ProfileEditFinish::Applied
    }

    pub fn status(&self, draft: &str, locale: &str) -> String {
        match &self.operation {
            ProfileEditOperationPhase::Pending(_) => {
                return localize(locale, "editor_save_pending", &[]);
            }
            ProfileEditOperationPhase::Failed(failure) => {
                return localize(
                    locale,
                    "editor_save_failed",
                    &[("reason", failure_message(failure, locale))],
                );
            }
            ProfileEditOperationPhase::Ready | ProfileEditOperationPhase::Saved => {}
        }
        match &self.read_status {
            ProfileReadStatus::Failed(failure) => {
                return localize(
                    locale,
                    "editor_read_failed",
                    &[("reason", failure_message(failure, locale))],
                );
            }
            ProfileReadStatus::Loading => {
                return localize(locale, "editor_read_loading", &[]);
            }
            ProfileReadStatus::Unobserved | ProfileReadStatus::Ready => {}
        }
        match &self.verification {
            ProfileEditVerification::Failed(failure) => {
                return localize(
                    locale,
                    "editor_source_verification_failed",
                    &[("reason", failure_message(failure, locale))],
                );
            }
            ProfileEditVerification::Stale => {
                return localize(locale, "editor_save_source_changed", &[]);
            }
            ProfileEditVerification::Unchecked | ProfileEditVerification::Current => {}
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
        if matches!(&self.operation, ProfileEditOperationPhase::Saved) {
            return localize(locale, "editor_save_committed", &[]);
        }
        String::new()
    }
}
