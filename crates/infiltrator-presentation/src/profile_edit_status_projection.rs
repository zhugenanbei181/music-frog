//! Localized copy for typed profile-editor session facts.
use infiltrator_contract::profile_editor_read::ProfileEditStatusFact;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::failure_projection::failure_message;

pub fn status(fact: &ProfileEditStatusFact, locale: &str) -> String {
    match fact {
        ProfileEditStatusFact::SavePending => localize(locale, "editor_save_pending", &[]),
        ProfileEditStatusFact::SaveFailed(failure) => localize(
            locale,
            "editor_save_failed",
            &[("reason", failure_message(failure, &Lang(locale)))],
        ),
        ProfileEditStatusFact::ReadFailed(failure) => localize(
            locale,
            "editor_read_failed",
            &[("reason", failure_message(failure, &Lang(locale)))],
        ),
        ProfileEditStatusFact::ReadLoading => localize(locale, "editor_read_loading", &[]),
        ProfileEditStatusFact::VerificationFailed(failure) => localize(
            locale,
            "editor_source_verification_failed",
            &[("reason", failure_message(failure, &Lang(locale)))],
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
