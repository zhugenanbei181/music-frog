//! Profile-options handlers: the mixin overlay editor (Editor page second
//! pane) and the per-profile subscription filter editor.
//!
//! DUAL-09-14: both writes go through the *shared* application use-case
//! [`ProfileOptionsApplication`] — the same call the Bevy editor panes make.
//! The Mixin save strips the outgoing mixin's prepend/append rule lines first
//! so repeated edits stay idempotent; the filter save re-runs the pipeline in
//! place (the next subscription update recomposes from the raw source anyway).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::profile_edit::{ProfileEditReply, ProfileOptionsReadReply};
use crate::update::profile::editor::ViewportSync;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_application::failure_projection::failure_message;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;
use infiltrator_domain::mixin_studio::{MixinPreflightReport, preflight_mixin, set_toggle};

impl AppState {
    pub(super) fn update_options(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetEditorPane(pane) => {
                self.editor.editor_pane = pane;
                match pane {
                    EditorPane::Mixin => self.ensure_mixin_loaded(),
                    EditorPane::Filter => self.ensure_filter_loaded(),
                    EditorPane::Profile | EditorPane::Script => Task::none(),
                }
            }
            Message::MixinEditorAction(action) => {
                if !self.editor.mixin_session.can_edit() {
                    return Task::none();
                }
                let scroll_delta = match &action {
                    text_editor::Action::Scroll { lines } => Some(*lines),
                    _ => None,
                };
                self.editor.mixin_content.perform(action);
                // DUAL-09-02/13: the Mixin overlay is windowed the same way the
                // profile document is, from the same shared model.
                match scroll_delta {
                    Some(delta) => self
                        .sync_document_viewport(EditorPane::Mixin, ViewportSync::Scrolled(delta)),
                    None => self.sync_document_viewport(EditorPane::Mixin, ViewportSync::Caret),
                }
                let text = self.editor.mixin_content.text();
                // DUAL-10-10: the shared syntax + merge + validation preflight
                // gates the pane (the same rule the application re-runs).
                let report = self.mixin_preflight(&text);
                self.editor.syntax_error = report.error;
                self.editor.syntax_error_line = None;
                Task::none()
            }
            Message::ToggleMixinPreset(id, enabled) => {
                if !self.editor.mixin_session.can_edit() {
                    return Task::none();
                }
                let text = self.editor.mixin_content.text();
                match set_toggle(&text, &id, enabled) {
                    Ok(updated) => {
                        self.editor.mixin_content = text_editor::Content::with_text(&updated);
                        self.reset_document_viewport(EditorPane::Mixin);
                        let report = self.mixin_preflight(&updated);
                        self.editor.syntax_error = report.error;
                        self.editor.syntax_error_line = None;
                    }
                    Err(error) => {
                        self.set_error(InfiltratorError::Config(error));
                    }
                }
                Task::none()
            }
            Message::MixinLoaded(reply) => {
                if self.editor.mixin_load.as_ref() != Some(&(reply.ticket, reply.profile.clone()))
                    || self.editor_profile_name().as_deref() != Some(&reply.profile)
                {
                    return Task::none();
                }
                self.editor.mixin_load = None;
                match reply.result {
                    Ok(snapshot) => {
                        if self.editor.mixin_session.observe(
                            &snapshot.source,
                            &snapshot.mixin_yaml,
                            &self.editor.mixin_content.text(),
                        ) {
                            self.editor.mixin_content =
                                text_editor::Content::with_text(&snapshot.mixin_yaml);
                            self.reset_document_viewport(EditorPane::Mixin);
                        }
                        self.editor.mixin_session.read_completed(&snapshot.source);
                    }
                    Err(failure) => self.editor.mixin_session.read_failed(failure),
                }
                Task::none()
            }
            Message::DiscardMixinDraft => {
                if let Some(content) = self.editor.mixin_session.discard() {
                    self.editor.mixin_content = text_editor::Content::with_text(&content);
                    self.reset_document_viewport(EditorPane::Mixin);
                }
                Task::none()
            }
            Message::SaveMixin => self.save_mixin(),
            Message::MixinSaved(reply) => {
                if !self.editor.mixin_session.finish(
                    reply.pending.operation,
                    &reply.pending.intent,
                    reply.result.clone(),
                ) {
                    return Task::none();
                }
                self.editor.is_saving_mixin = false;
                if let Ok(CommandOutput::ProfileMixinSaved(saved)) = reply.result {
                    self.editor.mixin_content =
                        text_editor::Content::with_text(&saved.options.mixin_yaml);
                    if let Some(source) = saved.document.source.as_ref()
                        && self.editor.document_session.observe(
                            source,
                            &saved.document.content,
                            &self.editor.editor_content.text(),
                        )
                    {
                        self.editor.editor_content =
                            text_editor::Content::with_text(&saved.document.content);
                        self.reset_document_viewport(EditorPane::Profile);
                    }
                    if let Some((_, document)) = &mut self.editor.document_latest {
                        *document = saved.document;
                    }
                    self.invalidate_rules_dns_views();
                    Task::done(Message::LoadProfileSnapshots)
                } else {
                    Task::none()
                }
            }
            Message::LoadProfileFilter => self.read_profile_filter(true),
            Message::ProfileFilterLoaded {
                token,
                profile,
                result,
            } => {
                if self.editor.filter_load.as_ref() != Some(&(token, profile.clone()))
                    || self.editor_profile_name().as_deref() != Some(&profile)
                {
                    return Task::none();
                }
                self.editor.filter_load = None;
                self.editor.filter_editor.observe_profile(&profile, result);
                Task::none()
            }
            Message::UpdateFilterInclude(value) => {
                self.editor.filter_editor.edit(FilterField::Include, value);
                Task::none()
            }
            Message::UpdateFilterExclude(value) => {
                self.editor.filter_editor.edit(FilterField::Exclude, value);
                Task::none()
            }
            Message::UpdateFilterExcludeTypes(value) => {
                self.editor
                    .filter_editor
                    .edit(FilterField::Protocols, value);
                Task::none()
            }
            Message::UpdateFilterRenames(value) => {
                self.editor.filter_editor.edit(FilterField::Renames, value);
                Task::none()
            }
            Message::UpdateFilterAdvancedPolicy(value) => {
                self.editor.filter_editor.edit(FilterField::Advanced, value);
                Task::none()
            }
            Message::UpdateFilterDedup(index) => {
                if let Some(mode) = SubscriptionFilterDedup::from_index(index) {
                    self.editor.filter_editor.pick(mode);
                }
                Task::none()
            }
            Message::DiscardProfileFilter => {
                self.editor.filter_editor.cancel();
                Task::none()
            }
            Message::SaveProfileFilter => self.save_profile_filter(),
            Message::ProfileFilterSaved { token, result } => {
                let applied = result.is_ok();
                if self.editor.filter_editor.finish(token, result)
                    && applied
                    && self.editor.filter_editor.applied
                {
                    self.invalidate_rules_dns_views();
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Lazily load the profile's mixin overlay the first time the Mixin pane
    /// is opened for it. DUAL-09-14: the read is the shared sidecar use-case,
    /// so the same snapshot lands in the surface projection the Bevy panes
    /// consume.
    pub(super) fn ensure_mixin_loaded(&mut self) -> Task<Message> {
        let Some(profile) = self.editor_profile_name() else {
            return Task::none();
        };
        if self.editor.mixin_loaded_for.as_deref() == Some(profile.as_str()) {
            return Task::none();
        }
        self.editor.mixin_loaded_for = Some(profile.clone());
        self.editor.next_editor_read = self
            .editor
            .next_editor_read
            .checked_add(1)
            .expect("editor read identity exhausted");
        let ticket = self.editor.next_editor_read;
        self.editor.mixin_load = Some((ticket, profile.clone()));
        let commands = self.commands.clone();
        Task::perform(
            async move {
                let result = match commands {
                    Some(commands) => commands
                        .execute(CommandIntent::LoadProfileOptions {
                            profile: Some(profile.clone()),
                        })
                        .await
                        .into_output()
                        .and_then(CommandOutput::into_profile_options),
                    None => Err(Failure::new(
                        ErrorCode::NotReady,
                        "Profile editor command service is unavailable",
                        true,
                    )),
                };
                ProfileOptionsReadReply {
                    ticket,
                    profile,
                    result,
                }
            },
            Message::MixinLoaded,
        )
    }

    /// Lazily load the per-profile subscription filter the first time the
    /// Filter pane is opened for the profile currently open in the editor.
    pub(super) fn ensure_filter_loaded(&mut self) -> Task<Message> {
        self.read_profile_filter(false)
    }
    fn read_profile_filter(&mut self, refresh: bool) -> Task<Message> {
        let Some(profile) = self.editor_profile_name() else {
            return Task::none();
        };
        self.editor.filter_editor.bind_profile(Some(&profile));
        if !refresh
            && self.editor.filter_editor.source_profile() == Some(&profile)
            && self.editor.filter_editor.current()
        {
            return Task::none();
        }
        if self
            .editor
            .filter_load
            .as_ref()
            .is_some_and(|(_, source)| source == &profile)
        {
            return Task::none();
        }
        self.editor.next_filter_load += 1;
        let token = self.editor.next_filter_load;
        self.editor.filter_load = Some((token, profile.clone()));
        let reply_profile = profile.clone();
        let commands = self.commands.clone();
        Task::perform(
            async move {
                let commands = commands.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "Filter command service is unavailable",
                        true,
                    )
                })?;
                commands
                    .execute(CommandIntent::LoadProfileOptions {
                        profile: Some(profile),
                    })
                    .await
                    .into_output()?
                    .into_profile_options()
                    .map(|snapshot| FilterObservation {
                        source: snapshot.source,
                        filter: snapshot.filter,
                    })
            },
            move |result| Message::ProfileFilterLoaded {
                token,
                profile: reply_profile,
                result,
            },
        )
    }

    /// The profile the editor has open, derived from the edited document path.
    pub(crate) fn editor_profile_name(&self) -> Option<String> {
        self.editor
            .editor_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|name| name.to_str())
            .map(str::to_string)
    }

    /// DUAL-10-10: run the shared Mixin preflight against the open profile
    /// document as the base. Both the live pane verdict and the save gate call it.
    fn mixin_preflight(&self, mixin_yaml: &str) -> MixinPreflightReport {
        let base = self.editor.editor_content.text();
        preflight_mixin(&base, mixin_yaml)
    }

    fn save_mixin(&mut self) -> Task<Message> {
        if self.editor.is_saving_mixin {
            return Task::none();
        }
        // DUAL-10-10 validation gate: a malformed or unmergeable overlay is
        // rejected by the shared preflight before any state flips or task
        // spawns, so the editor keeps its content for fixing.
        let text = self.editor.mixin_content.text();
        let report = self.mixin_preflight(&text);
        if let Some(error) = report.error {
            let failure = Failure::new(ErrorCode::InvalidInput, error.clone(), false)
                .with_reason(FailureReason::MixinYaml);
            let message = failure_message(&failure, &self.shell.lang);
            self.set_error(&message);
            self.editor.mixin_session.failure = Some(failure);
            self.editor.syntax_error = Some(error);
            self.editor.syntax_error_line = None;
            return Task::done(Message::ShowToast(message, ToastStatus::Error));
        }
        self.editor.syntax_error = None;
        self.editor.syntax_error_line = None;
        let pending = match self.editor.mixin_session.begin_mixin(text) {
            Ok(pending) => pending,
            Err(failure) => {
                self.editor.mixin_session.failure = Some(failure);
                return Task::none();
            }
        };
        self.editor.is_saving_mixin = true;
        let commands = self.commands.clone();
        Task::perform(
            async move {
                let result = match commands {
                    Some(commands) => commands.execute(pending.intent.clone()).await.into_output(),
                    None => Err(Failure::new(
                        ErrorCode::NotReady,
                        "Profile editor command service is unavailable",
                        true,
                    )),
                };
                ProfileEditReply { pending, result }
            },
            Message::MixinSaved,
        )
    }

    fn save_profile_filter(&mut self) -> Task<Message> {
        let Some(profile) = self.editor_profile_name() else {
            return Task::none();
        };
        if self.editor.filter_editor.source_profile() != Some(&profile) {
            return Task::none();
        }
        let pending = match self.editor.filter_editor.begin() {
            Ok(pending) => pending,
            Err(_) => return Task::none(),
        };
        let token = pending.token;
        let commands = self.commands.clone();
        Task::perform(
            async move {
                let commands = commands.ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "Filter command service is unavailable",
                        true,
                    )
                })?;
                commands
                    .execute(CommandIntent::SaveSubscriptionFilter {
                        source: pending.source.source,
                        filter: pending.draft,
                    })
                    .await
                    .into_output()?
                    .into_subscription_filter()
            },
            move |result| Message::ProfileFilterSaved { token, result },
        )
    }
}
