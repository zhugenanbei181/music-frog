//! Replay independent editor facts without navigating or replacing a dirty buffer.
use crate::state::AppState;
use iced::widget::text_editor;
use infiltrator_application::profile_edit_session::{
    ProfileEditObservation, ProfileEditReadTarget,
};
use infiltrator_contract::profile_editor_read::ProfileEditorSnapshot;
impl AppState {
    pub(crate) fn observe_editor_surface(&mut self, snapshot: &ProfileEditorSnapshot) {
        if let Some(document) = &snapshot.document
            && self.editor.document_session.source().is_some_and(|old| {
                old.document.profile_id == document.workspace_identity.document.profile_id
            })
            && matches!(
                self.editor.document_session.observe(
                    &document.workspace_identity,
                    &document.content,
                    &self.editor.editor_content.text(),
                ),
                ProfileEditObservation::Updated
            )
        {
            self.editor.editor_content = text_editor::Content::with_text(&document.content);
            if let Some((_, prior)) = &mut self.editor.document_latest {
                *prior = document.clone();
            }
        }
        if let Some(document) = &snapshot.document
            && Some(&document.workspace_identity) == self.editor.document_session.source()
            && let Some((_, prior)) = &mut self.editor.document_latest
        {
            if prior.write_protection != document.write_protection {
                self.editor.profile_write_authorization = Default::default();
            }
            prior.write_protection = document.write_protection;
        }
        if let Some(options) = &snapshot.options
            && self.editor.mixin_session.source().is_some_and(|old| {
                old.document.profile_id == options.workspace_identity.document.profile_id
            })
            && matches!(
                self.editor.mixin_session.observe(
                    &options.workspace_identity,
                    &options.mixin_yaml,
                    &self.editor.mixin_content.text(),
                ),
                ProfileEditObservation::Updated
            )
        {
            self.editor.mixin_content = text_editor::Content::with_text(&options.mixin_yaml);
        }
        self.editor
            .document_session
            .observe_read_status(&snapshot.read, ProfileEditReadTarget::Document);
        self.editor
            .mixin_session
            .observe_read_status(&snapshot.read, ProfileEditReadTarget::Options);
    }
}
