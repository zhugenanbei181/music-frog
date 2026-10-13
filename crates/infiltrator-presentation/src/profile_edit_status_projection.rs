//! Localized copy for typed profile-editor session facts.
use crate::failure_projection::failure_message;
use infiltrator_contract::profile_editor_read::ProfileEditStatusFact;
use infiltrator_shared::i18n_interpolator::localize;

pub fn status(fact: &ProfileEditStatusFact, locale: &str) -> String {
    match fact {
        ProfileEditStatusFact::SavePending => localize(locale, "editor_save_pending", &[]),
        ProfileEditStatusFact::SaveFailed(failure) => localize(
            locale,
            "editor_save_failed",
            &[("reason", failure_message(failure, locale))],
        ),
        ProfileEditStatusFact::ReadFailed(failure) => localize(
            locale,
            "editor_read_failed",
            &[("reason", failure_message(failure, locale))],
        ),
        ProfileEditStatusFact::ReadLoading => localize(locale, "editor_read_loading", &[]),
        ProfileEditStatusFact::VerificationFailed(failure) => localize(
            locale,
            "editor_source_verification_failed",
            &[("reason", failure_message(failure, locale))],
        ),
        ProfileEditStatusFact::SourceChanged => localize(locale, "editor_save_source_changed", &[]),
        ProfileEditStatusFact::SourceUnobserved => localize(locale, "editor_save_unobserved", &[]),
        ProfileEditStatusFact::UnsavedChanges => localize(locale, "editor_unsaved", &[]),
        ProfileEditStatusFact::Saved => localize(locale, "editor_save_committed", &[]),
        ProfileEditStatusFact::Ready => String::new(),
    }
}

#[cfg(test)]
#[path = "profile_edit_status_projection_tests.rs"]
mod tests;
