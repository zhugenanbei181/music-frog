//! TEA replays the shared inspector; cleanup only edits the shared draft.
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::rule_statistics_workbench::StatisticsAction;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};

impl AppState {
    pub(super) fn update_rule_statistics(&mut self, action: StatisticsAction) -> Task<Message> {
        match action {
            StatisticsAction::Tab(tab) => self.editor.rule_hit_audit.select_tab(tab),
            StatisticsAction::PreviousPage => self.editor.rule_hit_audit.previous_page(),
            StatisticsAction::NextPage => self.editor.rule_hit_audit.next_page(),
            StatisticsAction::Inspect => {
                self.editor.rule_hit_audit.inspect(&self.editor.rule_list);
            }
            StatisticsAction::PrepareCleanup => {
                if self
                    .editor
                    .rule_hit_audit
                    .prepare_cleanup(&self.editor.rule_list)
                {
                    self.shell.confirmation = Some(ConfirmAction::RuleStatisticsCleanup);
                }
            }
            StatisticsAction::CancelCleanup => {
                self.editor.rule_hit_audit.cancel_cleanup();
                if self.shell.confirmation == Some(ConfirmAction::RuleStatisticsCleanup) {
                    self.shell.confirmation = None;
                }
            }
            StatisticsAction::ConfirmCleanup => {
                if self
                    .editor
                    .rule_hit_audit
                    .confirm_cleanup(&mut self.editor.rule_list)
                    .is_some()
                {
                    self.shell.confirmation = None;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
            }
            StatisticsAction::DismissFailure => self.editor.rule_hit_audit.clear_failure = None,
            StatisticsAction::Reset => {
                let Some(request) = self.editor.rule_hit_audit.begin_reset() else {
                    return Task::none();
                };
                let Some(commands) = self.commands.clone() else {
                    self.editor.rule_hit_audit.finish_reset(
                        &request,
                        Err(Failure::new(
                            ErrorCode::NotReady,
                            "Rule statistics command service is unavailable",
                            true,
                        )),
                    );
                    return Task::none();
                };
                let completion = request.clone();
                return Task::perform(
                    async move {
                        commands
                            .execute(CommandIntent::ResetRuleHitCounters {
                                expected_source: request.source,
                            })
                            .await
                            .into_output()
                            .and_then(CommandOutput::into_statistics_reset)
                    },
                    move |result| {
                        Message::RuleStatistics(StatisticsAction::ResetFinished {
                            request: completion,
                            result,
                        })
                    },
                );
            }
            StatisticsAction::ResetFinished { request, result } => {
                self.editor.rule_hit_audit.finish_reset(&request, result);
            }
        }
        Task::none()
    }
}
