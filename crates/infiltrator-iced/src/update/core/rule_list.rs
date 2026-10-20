//! Iced adapter for complete, source-bound draft facts and atomic commits.
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use iced::Task;
use iced::widget::text_editor::Content;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};
use infiltrator_contract::surface_snapshot::RulesPageSnapshot;
use infiltrator_presentation::rule_form_binding::default_form_target;
use infiltrator_presentation::rules_page_model::RulesOperation;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;

impl AppState {
    pub(crate) fn observe_rule_list_page(&mut self, page: &PageData<RulesPageSnapshot>) {
        if self.editor.rule_list.observe_page(page) {
            self.editor.rules_loaded_once = true;
            self.rebuild_rules_render_cache();
            self.apply_rules_filter();
        }
        if let Some(snapshot) = page.data.as_ref() {
            for document in &snapshot.json_documents {
                self.adopt_rules_document(document.section, document);
            }
        }
        if self
            .editor
            .rule_form_binding
            .observe(&self.editor.rule_list)
        {
            self.editor.new_rule_target = default_form_target(&self.editor.rule_list);
        }
    }
    pub(super) fn update_rule_list(&mut self, action: RuleListAction) -> Task<Message> {
        match action {
            RuleListAction::Settings => {
                if self.editor.rule_list.can_guide() {
                    return self.update(Message::Navigate(Route::Settings));
                }
            }
            RuleListAction::Discard => {
                if self.editor.rule_list.discard() {
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
            }
            RuleListAction::DiscardForm => {
                if !self.editor.rule_list.is_pending()
                    && !self.editor.rule_list.is_awaiting_readback()
                {
                    self.editor.new_rule_type = "DOMAIN-SUFFIX".into();
                    self.editor.new_rule_payload.clear();
                    self.editor.new_rule_target = default_form_target(&self.editor.rule_list);
                    self.editor.rule_form_binding.reset(&self.editor.rule_list);
                }
            }
            RuleListAction::Finished { operation, result } => {
                if self.editor.rule_list.finish(operation, result) {
                    self.editor
                        .rules_model
                        .operation_mut(RulesOperation::Save)
                        .reset();
                }
            }
        }
        Task::none()
    }
    pub(super) fn commit_rule_list(&mut self) -> Task<Message> {
        if !self.editor.rule_list.can_save() {
            return Task::none();
        }
        let Some(commands) = self.command_ingress() else {
            self.editor.rule_list.record_failure(Failure::new(
                ErrorCode::NotReady,
                "The rule command service is unavailable",
                true,
            ));
            return Task::none();
        };
        let (operation, request) = match self.editor.rule_list.begin() {
            Ok(pending) => pending,
            Err(_) => return Task::none(),
        };
        self.editor
            .rules_model
            .operation_mut(RulesOperation::Save)
            .begin();
        Task::perform(
            async move {
                match (commands
                    .execute(CommandIntent::CommitRuleList { request })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::RuleList(RuleListAction::Finished { operation, result }),
        )
    }
}

impl AppState {
    /// Adopt one JSON document the owner published.
    ///
    /// The committed text lives in the shared page model, next to the draft
    /// it belongs to; the surface keeps no second copy of it.
    fn adopt_rules_document(
        &mut self,
        section: RulesJsonSection,
        document: &RulesJsonDocumentSnapshot,
    ) {
        let modified = self.editor.rules_model.json_modified(section);
        if modified && self.editor.rules_json_content(section).text() != document.json.as_str() {
            return;
        }
        self.editor
            .rules_model
            .observe_json_document(section, document.json.clone());
        *self.editor.rules_json_identity_mut(section) = Some(document.identity.clone());
        if self.editor.rules_json_editor_state(section) == EditorLazyState::Loaded {
            let text = document.json.clone();
            *self.editor.rules_json_content_mut(section) = Content::with_text(&text);
        }
    }
}
