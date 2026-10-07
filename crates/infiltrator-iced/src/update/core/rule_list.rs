//! Iced adapter for complete, source-bound draft facts and atomic commits.
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use iced::Task;
use infiltrator_application::rule_form_binding::default_form_target;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::{PageData, RulesPageSnapshot};

impl AppState {
    pub(crate) fn observe_rule_list_page(&mut self, page: &PageData<RulesPageSnapshot>) {
        if self.editor.rule_list.observe_page(page) {
            self.editor.rules_loaded_once = true;
            self.rebuild_rules_render_cache();
            self.apply_rules_filter();
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
                if self.editor.rule_list.pending.is_none() && !self.editor.rule_list.awaiting_read {
                    self.editor.new_rule_type = "DOMAIN-SUFFIX".into();
                    self.editor.new_rule_payload.clear();
                    self.editor.new_rule_target = default_form_target(&self.editor.rule_list);
                    self.editor.rule_form_binding.reset(&self.editor.rule_list);
                }
            }
            RuleListAction::Finished { operation, result } => {
                if self.editor.rule_list.finish(operation, result) {
                    self.editor.is_saving_rules = false;
                }
            }
        }
        Task::none()
    }
    pub(super) fn commit_rule_list(&mut self) -> Task<Message> {
        let Some(commands) = self.commands.clone() else {
            self.editor.rule_list.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "The rule command service is unavailable",
                true,
            ));
            return Task::none();
        };
        let (operation, request) = match self.editor.rule_list.begin() {
            Ok(pending) => pending,
            Err(failure) => {
                self.editor.rule_list.failure = Some(failure);
                return Task::none();
            }
        };
        self.editor.is_saving_rules = true;
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
