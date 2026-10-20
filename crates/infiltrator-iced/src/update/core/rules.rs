//! Rules domain: the custom rules list (filter, reorder, add, toggle),
//! rule/proxy provider refresh and the lazy JSON editors for rule
//! providers, proxy providers and the sniffer config.

use crate::state::AppState;

use crate::types::app::{ConfirmAction, Route, ToastStatus};
use crate::types::message::Message;
use crate::types::rule_trace::RuleTraceAction;
use crate::types::rules::{RuleBadgeKind, RuleRenderItem};
use crate::view::rules_window::{
    RULES_LIST_SCROLL_ID, page_scroll_offset, rules_window, rules_window_page,
};
use iced::Task;
use iced::widget::Id;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable::AbsoluteOffset;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_domain::rules::view::{
    clamp_page, effective_page_size, filter_rule_indices, page_count, rule_scroll_offset_for_index,
};
use infiltrator_presentation::rule_list_editor::RuleListReadback;
use infiltrator_presentation::rule_list_projection::draft_page;
use infiltrator_presentation::rule_row_projection::rule_row;
use infiltrator_presentation::rules_page_model::RulesOperation;
use lurker_core_contract::error::InfiltratorError;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::time::Instant;

impl AppState {
    fn rule_badge_kind(rule_type: &str) -> RuleBadgeKind {
        match rule_type {
            "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" => RuleBadgeKind::Domain,
            "IP-CIDR" | "IP-CIDR6" | "GEOIP" => RuleBadgeKind::Ip,
            _ => RuleBadgeKind::Other,
        }
    }

    /// Rebuild the rules render cache from `rules`. pub(crate) so the demo
    /// constructor can seed the cache for its fixture rules.
    pub(crate) fn rebuild_rules_render_cache(&mut self) {
        let start = Instant::now();
        let observed = self
            .surface
            .latest()
            .and_then(|snapshot| snapshot.pages.rules.data.as_ref())
            .map(|page| draft_page(page, &self.editor.rule_list));
        self.editor.rules_render_cache = self
            .editor
            .rule_list
            .draft
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let row = rule_row(&entry.rule);
                let rule_type = row.rule_type;
                let payload = row.payload;
                let target = row.target;
                RuleRenderItem {
                    hit_count: observed
                        .as_ref()
                        .and_then(|page| page.rules.get(index))
                        .and_then(|row| row.hit_count),
                    is_shadowed: observed
                        .as_ref()
                        .and_then(|page| page.rules.get(index))
                        .is_some_and(|row| row.is_shadowed),
                    source_ip: row.source_ip,
                    no_resolve: row.no_resolve,
                    failure: row.failure,
                    source_index: index,
                    badge: Self::rule_badge_kind(&rule_type),
                    rule_type,
                    payload,
                    target,
                }
            })
            .collect();
        self.diag.perf_snapshot.rules_cache_build_ms = start.elapsed().as_millis();
    }

    /// Recompute the filtered rules page indices. pub(crate) so the demo
    /// constructor can apply its empty filter once at boot. DUAL-11-13: the
    /// match predicate, page size fallback, clamp and visible-row arithmetic
    /// all delegate to `infiltrator_domain::rules::view` so both surfaces page
    /// identically. DUAL-11-08: the visible rows are the shared virtual window
    /// at the current scroll offset, not a whole page.
    pub(crate) fn apply_rules_filter(&mut self) {
        self.editor.rules_filtered_indices =
            filter_rule_indices(&self.editor.rule_list.draft, &self.editor.rules_filter);
        self.editor.rules_page_size = effective_page_size(self.editor.rules_page_size);
        self.editor.rules_page = clamp_page(
            self.editor.rules_page,
            self.editor.rules_filtered_indices.len(),
            self.editor.rules_page_size,
        );
        self.sync_rules_window_facts();
    }

    /// DUAL-11-08: keep the scroll offset inside the filtered list and publish
    /// the honest render count of the shared window. The window's own
    /// arithmetic is O(1): it is derived from the offset and the viewport, so
    /// this never walks the rule list.
    pub(crate) fn sync_rules_window_facts(&mut self) {
        let total = self.editor.rules_filtered_indices.len();
        let content_height = rule_scroll_offset_for_index(total);
        if self.editor.rules_scroll_offset_px > content_height {
            self.editor.rules_scroll_offset_px = content_height;
        }
        let window = rules_window(self);
        self.diag.perf_snapshot.rules_visible_rows = window.rendered_rows();
    }

    /// DUAL-11-08: align the scrollable with the render window after the
    /// window moved on its own (a new list or a recomputed filter). Without
    /// this the mounted rows and the viewport could disagree.
    fn scroll_rules_list_to_window(&self) -> Task<Message> {
        scroll_to(
            Id::new(RULES_LIST_SCROLL_ID),
            AbsoluteOffset {
                x: None,
                y: Some(self.editor.rules_scroll_offset_px),
            },
        )
    }

    /// DUAL-11-08: move the paging cursor to `page` and hand the scrollable
    /// the absolute offset of that page's first row. The window follows the
    /// offset immediately, so the jump never waits for a scroll event.
    pub(crate) fn goto_rules_page(&mut self, page: usize) -> Task<Message> {
        self.editor.rules_page = clamp_page(
            page,
            self.editor.rules_filtered_indices.len(),
            self.editor.rules_page_size,
        );
        let offset = page_scroll_offset(self, self.editor.rules_page);
        self.editor.rules_scroll_offset_px = offset;
        self.sync_rules_window_facts();
        scroll_to(
            Id::new(RULES_LIST_SCROLL_ID),
            AbsoluteOffset {
                x: None,
                y: Some(offset),
            },
        )
    }

    fn run_rules_tracer(&mut self) -> Task<Message> {
        let commands = self.command_ingress();
        let model = &mut self.editor.rule_trace;
        model.set_query(self.editor.rules_tracer_input.clone());
        model.set_source_ip(self.editor.rules_tracer_src_ip.clone());
        let Ok((operation, request)) = model.begin() else {
            return Task::none();
        };
        let Some(commands) = commands else {
            model.finish(
                operation,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Rule simulation command service is unavailable",
                    true,
                )),
            );
            return Task::none();
        };
        Task::perform(
            async move {
                match (commands
                    .execute(CommandIntent::SimulateRuleTrace { operation, request })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::RuleTrace(RuleTraceAction::Finished { operation, result }),
        )
    }

    pub(super) fn update_core_rules(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilterRules(filter) => {
                self.editor.rules_filter = filter;
                self.editor.rules_page = 0;
                // DUAL-11-08: the filtered list shrank or changed shape, so the
                // viewport returns to the top of the new result set — the
                // window and the scrollable move together.
                self.editor.rules_scroll_offset_px = 0.0;
                self.apply_rules_filter();
                self.scroll_rules_list_to_window()
            }
            Message::RuleTrace(RuleTraceAction::ToggleSandbox) => {
                if !self.editor.rule_trace.busy() && self.editor.rule_trace.confirmation().is_none()
                {
                    self.editor.rule_trace.advanced_open = !self.editor.rule_trace.advanced_open;
                }
                Task::none()
            }
            Message::RuleTrace(RuleTraceAction::Sandbox(field, value)) => {
                self.editor.rule_trace.set_sandbox(field, value);
                Task::none()
            }
            Message::RuleTrace(RuleTraceAction::Settings) => {
                if self
                    .editor
                    .rule_trace
                    .override_failure()
                    .is_some_and(|failure| {
                        matches!(
                            failure.code,
                            ErrorCode::Permission | ErrorCode::Authentication
                        )
                    })
                    && self.editor.rule_trace.cancel_override()
                {
                    self.shell.confirmation = None;
                    return self.update(Message::Navigate(Route::Settings));
                }
                Task::none()
            }
            Message::RuleTrace(RuleTraceAction::Finished { operation, result }) => {
                self.editor.rule_trace.finish(operation, result);
                Task::none()
            }
            Message::UpdateRulesTracerInput(input) => {
                if self.editor.rule_trace.busy() || self.editor.rule_trace.confirmation().is_some()
                {
                    return Task::none();
                }
                self.editor.rule_trace.set_query(input.clone());
                self.editor.rules_tracer_input = input;
                Task::none()
            }
            Message::UpdateTracerSourceIp(input) => {
                if self.editor.rule_trace.busy() || self.editor.rule_trace.confirmation().is_some()
                {
                    return Task::none();
                }
                self.editor.rule_trace.set_source_ip(input.clone());
                self.editor.rules_tracer_src_ip = input;
                Task::none()
            }
            Message::RunRulesTracer => self.run_rules_tracer(),
            Message::UpdateTracerOverrideTarget(target) => {
                if self.editor.rule_trace.busy() || self.editor.rule_trace.confirmation().is_some()
                {
                    return Task::none();
                }
                self.editor.rules_tracer_override_target = target;
                Task::none()
            }
            Message::ApplyTracerRuleOverride { rule_index } => {
                let report = self
                    .surface
                    .latest()
                    .and_then(|snapshot| snapshot.pages.rules.data.as_ref())
                    .map(|rules| rules.tracer.clone());
                if let Some(report) = report
                    && self.editor.rule_trace.prepare_override(
                        &report,
                        rule_index,
                        self.editor.rules_tracer_override_target.clone(),
                    )
                {
                    self.shell.confirmation = self
                        .editor
                        .rule_trace
                        .confirmation()
                        .cloned()
                        .map(ConfirmAction::TracerOverride);
                }
                Task::none()
            }
            Message::RuleTrace(RuleTraceAction::ConfirmOverride) => {
                let Some((operation, request)) = self.editor.rule_trace.begin_override() else {
                    return Task::none();
                };
                let Some(commands) = self.command_ingress() else {
                    self.editor.rule_trace.finish_override(
                        operation,
                        Err(Failure::new(
                            ErrorCode::NotReady,
                            "Rule apply command service is unavailable",
                            true,
                        )),
                    );
                    return Task::none();
                };
                Task::perform(
                    async move {
                        match (commands
                            .execute(CommandIntent::ApplyTracerRuleOverride { request })
                            .await)
                            .into_unit()
                        {
                            Ok(()) => Ok(()),
                            Err(failure) => Err(failure),
                        }
                    },
                    move |result| {
                        Message::RuleTrace(RuleTraceAction::OverrideFinished { operation, result })
                    },
                )
            }
            Message::RuleTrace(RuleTraceAction::OverrideFinished { operation, result }) => {
                if self.editor.rule_trace.finish_override(operation, result)
                    && self.editor.rule_trace.confirmation().is_none()
                {
                    self.shell.confirmation = None;
                }
                Task::none()
            }
            Message::UpdateNewRuleType(t) => {
                self.editor.rule_form_binding.edit(&self.editor.rule_list);
                self.editor.new_rule_type = t;
                Task::none()
            }
            Message::UpdateNewRulePayload(p) => {
                self.editor.rule_form_binding.edit(&self.editor.rule_list);
                self.editor.new_rule_payload = p;
                Task::none()
            }
            Message::UpdateNewRuleTarget(t) => {
                self.editor.rule_form_binding.edit(&self.editor.rule_list);
                self.editor.new_rule_target = t;
                Task::none()
            }
            Message::AddCustomRule => {
                self.editor
                    .rule_form_binding
                    .observe(&self.editor.rule_list);
                if let Err(failure) = self
                    .editor
                    .rule_form_binding
                    .require_current(&self.editor.rule_list)
                {
                    self.editor.rule_list.record_failure(failure);
                    return Task::none();
                }
                let payload = self.editor.new_rule_payload.trim().to_string();
                if payload.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Payload cannot be empty".to_string(),
                        ToastStatus::Error,
                    ));
                }

                let draft = RuleDraft {
                    rule_type: self.editor.new_rule_type.clone(),
                    payload,
                    target: self.editor.new_rule_target.clone(),
                };
                match self.editor.rule_list.add(&draft) {
                    Ok(true) => {
                        self.editor.new_rule_payload.clear();
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();
                    }
                    Ok(false) => {}
                    Err(failure) => self.editor.rule_list.record_failure(failure),
                }
                Task::none()
            }
            Message::SetRulesTab(tab) => {
                self.editor.rules_model.select_tab(tab);
                self.editor.rules_page = 0;
                match tab {
                    RulesTab::JsonEditors => {
                        Task::done(match self.editor.rules_model.json_section() {
                            RulesJsonSection::RuleProviders => {
                                Message::EnsureRuleProvidersEditorLoaded
                            }
                            RulesJsonSection::ProxyProviders => {
                                Message::EnsureProxyProvidersEditorLoaded
                            }
                            RulesJsonSection::Sniffer => Message::EnsureSnifferEditorLoaded,
                        })
                    }
                    _ => Task::none(),
                }
            }
            Message::SetRulesJsonTab(tab) => {
                self.editor.rules_model.select_json_section(tab);
                Task::done(match tab {
                    RulesJsonSection::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                    RulesJsonSection::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                    RulesJsonSection::Sniffer => Message::EnsureSnifferEditorLoaded,
                })
            }
            Message::ToggleRulesProvidersExpanded => {
                self.editor.rules_providers_expanded = !self.editor.rules_providers_expanded;
                Task::none()
            }
            Message::RulesPrevPage => {
                self.goto_rules_page(self.editor.rules_page.saturating_sub(1))
            }
            Message::RulesNextPage => {
                let total_pages = page_count(
                    self.editor.rules_filtered_indices.len(),
                    self.editor.rules_page_size,
                );
                let target = if self.editor.rules_page + 1 < total_pages {
                    self.editor.rules_page + 1
                } else {
                    self.editor.rules_page
                };
                self.goto_rules_page(target)
            }
            Message::RulesSetPage(page) => self.goto_rules_page(page),
            Message::RulesListScrolled {
                offset_px,
                viewport_px,
            } => {
                self.editor.rules_scroll_offset_px = offset_px;
                if viewport_px > 0.0 {
                    self.editor.rules_viewport_px = viewport_px;
                }
                self.sync_rules_window_facts();
                self.editor.rules_page = rules_window_page(self);
                Task::none()
            }
            Message::EnsureRuleProvidersEditorLoaded => {
                self.ensure_rule_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureProxyProvidersEditorLoaded => {
                self.ensure_proxy_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureSnifferEditorLoaded => {
                self.ensure_sniffer_editor_loaded();
                Task::none()
            }
            Message::ActivateRulesHeavyView => {
                self.editor.rules_heavy_ready = true;
                if self.editor.rules_model.tab() == RulesTab::JsonEditors {
                    Task::done(match self.editor.rules_model.json_section() {
                        RulesJsonSection::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonSection::ProxyProviders => {
                            Message::EnsureProxyProvidersEditorLoaded
                        }
                        RulesJsonSection::Sniffer => Message::EnsureSnifferEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadRules => {
                if let Some(snapshot) = self.surface.latest() {
                    let page = snapshot.pages.rules.clone();
                    self.observe_rule_list_page(&page);
                }
                Task::none()
            }
            Message::RulesLoaded(result) => {
                self.editor
                    .rules_model
                    .operation_mut(RulesOperation::Load)
                    .reset();
                match result {
                    Ok(document) => {
                        if self
                            .editor
                            .rule_list
                            .observe_readback(RuleListReadback::Observed(document))
                        {
                            self.editor.rules_loaded_once = true;
                            self.rebuild_rules_render_cache();
                            self.apply_rules_filter();
                        }
                    }
                    Err(error) => {
                        self.editor
                            .rule_list
                            .observe_readback(RuleListReadback::Failed(Failure::new(
                                ErrorCode::Storage,
                                error.to_string(),
                                true,
                            )));
                    }
                }
                Task::none()
            }
            Message::ToggleRuleEnabled(index) => {
                if self.editor.rule_list.toggle(index) {
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleUp(index) => {
                if self
                    .editor
                    .rule_list
                    .move_rule(index, RuleMoveDirection::Up)
                {
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleDown(index) => {
                if self
                    .editor
                    .rule_list
                    .move_rule(index, RuleMoveDirection::Down)
                {
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::ApplyGameRoutingPresets => {
                self.editor
                    .rule_form_binding
                    .observe(&self.editor.rule_list);
                if let Err(failure) = self
                    .editor
                    .rule_form_binding
                    .require_current(&self.editor.rule_list)
                {
                    self.editor.rule_list.record_failure(failure);
                    return Task::none();
                }
                let target = self.editor.new_rule_target.clone();
                if !self.editor.rule_list.game_presets(&target) {
                    return Task::none();
                }
                self.rebuild_rules_render_cache();
                self.apply_rules_filter();
                Task::done(Message::ShowToast(
                    "Game routing presets injected to top of rules".to_string(),
                    ToastStatus::Success,
                ))
            }
            Message::UpdateGeoDatabases => {
                let Some(application) = self.command_ingress() else {
                    return Task::done(Message::ShowToast(
                        "Geo database update is not available on this host".to_string(),
                        ToastStatus::Error,
                    ));
                };
                self.editor
                    .rules_model
                    .operation_mut(RulesOperation::UpdateGeoDatabases)
                    .begin();
                Task::perform(
                    async move {
                        application
                            .execute(CommandIntent::UpgradeGeoDatabases)
                            .await
                            .into_unit()
                            .map_err(|failure| InfiltratorError::Internal(failure.message))
                    },
                    Message::GeoDatabasesUpdated,
                )
            }
            Message::GeoDatabasesUpdated(result) => {
                self.editor
                    .rules_model
                    .operation_mut(RulesOperation::UpdateGeoDatabases)
                    .reset();
                match result {
                    Ok(()) => Task::done(Message::ShowToast(
                        "GeoIP & GeoSite database update request accepted; completion is asynchronous".to_string(),
                        ToastStatus::Success,
                    )),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SaveRules => self.commit_rule_list(),
            Message::RuleList(action) => self.update_rule_list(action),
            Message::UpdateRuleProvider(reference) => {
                let Some(application) = self.command_ingress() else {
                    return Task::done(Message::ShowToast(
                        "Rule-provider control is not available on this host".to_owned(),
                        ToastStatus::Error,
                    ));
                };
                Task::perform(
                    async move {
                        application
                            .execute(CommandIntent::RefreshRuleProvider { reference })
                            .await
                            .into_unit()
                            .map_err(|failure| InfiltratorError::Internal(failure.message))
                    },
                    Message::OperationResult,
                )
            }
            Message::InspectRuleProviderDiff(_)
            | Message::RuleProviderDiffLoaded(_)
            | Message::RuleProviderUnpacked(_)
            | Message::RuleProviderCachePurged(_)
            | Message::UnpackRuleProvider(_)
            | Message::UnpackRuleProviderToCustom(_)
            | Message::PurgeRuleProviderCache => self.update_rule_provider(message),
            other => self.update_core_json_editors(other),
        }
    }
}
