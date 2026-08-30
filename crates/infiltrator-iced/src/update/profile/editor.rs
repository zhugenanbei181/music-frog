//! Profile YAML editor handlers: load profile content into the editor,
//! editor actions and saving back to disk.

use crate::state::AppState;
use crate::types::{InfiltratorError, Message, ToastStatus};
use iced::Task;
use iced::widget::text_editor;

impl AppState {
    pub(super) fn update_editor(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditProfile(path) => {
                let p = path.clone();
                Task::perform(
                    async move {
                        let content = tokio::fs::read_to_string(&p)
                            .await
                            .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                        Ok((p, content))
                    },
                    Message::ProfileContentLoaded,
                )
            }
            Message::ProfileContentLoaded(result) => match result {
                Ok((path, content)) => {
                    self.editor.editor_path = Some(path);
                    self.editor.editor_content = text_editor::Content::with_text(&content);
                    Task::done(Message::Navigate(crate::types::Route::Editor))
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::EditorAction(action) => {
                self.editor.editor_content.perform(action);
                Task::none()
            }
            Message::SaveProfile => {
                if let Some(path) = self.editor.editor_path.clone() {
                    let content = self.editor.editor_content.text();
                    Task::perform(
                        async move {
                            tokio::fs::write(&path, content)
                                .await
                                .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                            Ok(())
                        },
                        Message::ProfileSaved,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProfileSaved(result) => match result {
                Ok(_) => {
                    self.invalidate_rules_dns_views();
                    Task::done(Message::ShowToast(
                        "Profile saved".to_string(),
                        ToastStatus::Success,
                    ))
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            _ => Task::none(),
        }
    }
}
