//! Replay independent editor facts without navigating or replacing a dirty buffer.
use crate::state::AppState;
use iced::widget::text_editor;
use infiltrator_contract::profile_editor_read::ProfileEditorSnapshot;
impl AppState {
    pub(crate) fn observe_editor_surface(&mut self, snapshot: &ProfileEditorSnapshot) {
        if let Some(document) = &snapshot.document
            && let Some(source) = &document.source
            && self
                .editor
                .document_session
                .source()
                .is_some_and(|old| old.profile == source.profile)
            && self.editor.document_session.observe(
                source,
                &document.content,
                &self.editor.editor_content.text(),
            )
        {
            self.editor.editor_content = text_editor::Content::with_text(&document.content);
            if let Some((_, prior)) = &mut self.editor.document_latest {
                *prior = document.clone();
            }
        }
        if let Some(document) = &snapshot.document
            && document.source.as_ref() == self.editor.document_session.source()
            && let Some((_, prior)) = &mut self.editor.document_latest
        {
            if prior.write_protection != document.write_protection {
                self.editor.profile_protection_override = false;
            }
            prior.write_protection = document.write_protection;
        }
        if let Some(options) = &snapshot.options
            && self
                .editor
                .mixin_session
                .source()
                .is_some_and(|old| old.profile == options.source.profile)
            && self.editor.mixin_session.observe(
                &options.source,
                &options.mixin_yaml,
                &self.editor.mixin_content.text(),
            )
        {
            self.editor.mixin_content = text_editor::Content::with_text(&options.mixin_yaml);
        }
        self.editor
            .document_session
            .observe_read_status(&snapshot.read, false);
        self.editor
            .mixin_session
            .observe_read_status(&snapshot.read, true);
    }
}
