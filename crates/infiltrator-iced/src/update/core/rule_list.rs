//! Iced adapter for complete, source-bound draft facts and atomic commits.
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rules_workspace::RulesJsonSection;
use infiltrator_contract::surface_snapshot::{PageData, RulesPageSnapshot};
use infiltrator_presentation::rule_form_binding::default_form_target;

impl AppState {
    pub(crate) fn observe_rule_list_page(&mut self, page: &PageData<RulesPageSnapshot>) {
        if self.editor.rule_list.observe_page(page) {
            self.editor.rules_loaded_once = true;
            self.rebuild_rules_render_cache();
            self.apply_rules_filter();
        }
        if let Some(snapshot) = page.data.as_ref() {
            for document in &snapshot.json_documents {
                match document.section {
                    RulesJsonSection::RuleProviders => adopt_rules_document(
                        &mut self.editor.rule_providers_json_cache,
                        &mut self.editor.rule_providers_json_identity,
                        &mut self.editor.rule_providers_json_content,
                        self.editor.rule_providers_editor_state,
                        &mut self.editor.rule_providers_json_dirty,
                        document,
                    ),
                    RulesJsonSection::ProxyProviders => adopt_rules_document(
                        &mut self.editor.proxy_providers_json_cache,
                        &mut self.editor.proxy_providers_json_identity,
                        &mut self.editor.proxy_providers_json_content,
                        self.editor.proxy_providers_editor_state,
                        &mut self.editor.proxy_providers_json_dirty,
                        document,
                    ),
                    RulesJsonSection::Sniffer => adopt_rules_document(
                        &mut self.editor.sniffer_json_cache,
                        &mut self.editor.sniffer_json_identity,
                        &mut self.editor.sniffer_json_content,
                        self.editor.sniffer_editor_state,
                        &mut self.editor.sniffer_json_dirty,
                        document,
                    ),
                }
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
                    self.editor.is_saving_rules = false;
                }
            }
        }
        Task::none()
    }
    pub(super) fn commit_rule_list(&mut self) -> Task<Message> {
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
            Err(failure) => {
                self.editor.rule_list.record_failure(failure);
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

fn adopt_rules_document(
    cache: &mut String,
    identity: &mut Option<infiltrator_contract::profile_document_identity::ProfileDocumentIdentity>,
    content: &mut iced::widget::text_editor::Content,
    editor_state: EditorLazyState,
    dirty: &mut bool,
    document: &infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot,
) {
    if *dirty && content.text() != document.json {
        return;
    }
    *cache = document.json.clone();
    *identity = Some(document.identity.clone());
    *dirty = false;
    if editor_state == EditorLazyState::Loaded {
        *content = iced::widget::text_editor::Content::with_text(cache);
    }
}
