//! Localized copy for typed profile-editor session facts.
use crate::text::localize;
use infiltrator_contract::profile_editor_read::ProfileEditStatusFact;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::failure_projection::failure_message;

pub fn status(fact: &ProfileEditStatusFact, catalog: &dyn TextCatalog) -> String {
    match fact {
        ProfileEditStatusFact::SavePending => localize(catalog, "editor_save_pending", &[]),
        ProfileEditStatusFact::SaveFailed(failure) => localize(
            catalog,
            "editor_save_failed",
            &[("reason", failure_message(failure, catalog))],
        ),
        ProfileEditStatusFact::ReadFailed(failure) => localize(
            catalog,
            "editor_read_failed",
            &[("reason", failure_message(failure, catalog))],
        ),
        ProfileEditStatusFact::ReadLoading => localize(catalog, "editor_read_loading", &[]),
        ProfileEditStatusFact::VerificationFailed(failure) => localize(
            catalog,
            "editor_source_verification_failed",
            &[("reason", failure_message(failure, catalog))],
        ),
        ProfileEditStatusFact::SourceChanged => {
            localize(catalog, "editor_save_source_changed", &[])
        }
        ProfileEditStatusFact::SourceUnobserved => localize(catalog, "editor_save_unobserved", &[]),
        ProfileEditStatusFact::UnsavedChanges => localize(catalog, "editor_unsaved", &[]),
        ProfileEditStatusFact::Saved => localize(catalog, "editor_save_committed", &[]),
        ProfileEditStatusFact::Ready => String::new(),
    }
}

#[cfg(test)]
#[path = "profile_edit_status_projection_tests.rs"]
mod tests;
