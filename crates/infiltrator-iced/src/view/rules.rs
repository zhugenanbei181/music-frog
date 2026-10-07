//! Rules management page (分流规则与提供者管理):
//! - Rule Tracer sandbox for testing routing matches against domain/IP queries.
//! - Custom rules list with live filter, semantic badge coloring, enable toggle, reordering, and pagination.
//! - Providers management for proxy and rule providers (with diff inspect, unpack, and update).
//! - Geo databases updater for official MetaCubeX Geo data assets.
//! - Token-driven lazy JSON editors for rule providers, proxy providers, and sniffer.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use crate::types::rules::RuleBadgeKind;
use crate::view::component_card::card;
use crate::view::components::{BadgeKind, icon_button, modern_scrollable, segmented_control};
use crate::view::rules_tracer::{TRACER_SCROLL_ID, tracer_view};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, SP_LG, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Id, Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::rule_list_projection::{editor_preview, editor_status};
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::matrix::{RuleTypeFamily, matrix_family, matrix_label};
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) mod providers;
pub(crate) mod rules_list;

/// Format raw rule type into the shared semantic display label (`Domain`,
/// `DomainSuffix`, `IPCIDR`, `GeoIP`, `Match`, `RuleSet`, …) for all 33
/// catalogue spellings (DUAL-11-01). Unknown spellings keep their raw text.
pub fn display_rule_type(rule_type: &str) -> String {
    matrix_label(rule_type)
}

/// Map rule type and classifier to the shared badge palette (DUAL-11-01). The
/// family comes from the shared catalogue, so every known type is colored by
/// semantics rather than by a per-surface spelling list; `kind` is only the
/// fallback for a spelling the catalogue does not know.
pub fn semantic_badge_kind(rule_type: &str, kind: RuleBadgeKind) -> BadgeKind {
    match matrix_family(rule_type) {
        RuleTypeFamily::Host => BadgeKind::Accent,
        RuleTypeFamily::Address => BadgeKind::Warning,
        RuleTypeFamily::Unknown => match kind {
            RuleBadgeKind::Domain => BadgeKind::Accent,
            RuleBadgeKind::Ip => BadgeKind::Warning,
            RuleBadgeKind::Other => BadgeKind::Neutral,
        },
        _ => BadgeKind::Neutral,
    }
}
pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let filtered_count = state.editor.rules_filtered_indices.len();
    let save_rules_action = container(
        button(text(lang.tr("rules_save_btn").to_string())).on_press_maybe(
            state
                .editor
                .rule_list
                .can_save()
                .then_some(Message::SaveRules),
        ),
    )
    .id(InteractionRegion::RuleListSave.id());
    let discard = container(
        button(text(lang.tr("rules_draft_discard").to_string())).on_press_maybe(
            (state.editor.rule_list.pending.is_none()
                && !state.editor.rule_list.awaiting_read
                && (state.editor.rule_list.dirty() || state.editor.rule_list.source_changed()))
            .then_some(Message::RuleList(RuleListAction::Discard)),
        ),
    )
    .id(InteractionRegion::RuleListDiscard.id());

    let header = row![
        text(lang.tr("rules_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(theme::SP_MD),
        text(format!(
            "{} / {}",
            filtered_count,
            state.editor.rule_list.draft.len()
        ))
        .size(13)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary)
        }),
        Space::new().width(Length::Fill),
        if state.editor.is_loading_rules || state.editor.is_loading_providers {
            Element::from(text("...").size(12).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }))
        } else {
            icon_button(Icon::RefreshCw, 16.0, Message::LoadRules)
        },
        Space::new().width(theme::SP_SM),
    ]
    .align_y(Alignment::Center);

    let preview = editor_preview(&state.editor.rule_list, &state.shell.lang).join("\n");
    let controls = row![
        save_rules_action,
        discard,
        button(text(lang.tr("dns_query_settings").to_string())).on_press_maybe(
            state
                .editor
                .rule_list
                .can_guide()
                .then_some(Message::RuleList(RuleListAction::Settings))
        )
    ]
    .spacing(8)
    .wrap();
    let draft_panel = container(
        column![
            container(text(editor_status(&state.editor.rule_list, &state.shell.lang)).size(12))
                .id(InteractionRegion::RuleListStatus.id()),
            container(text(preview).size(12)).id(InteractionRegion::RuleListPreview.id()),
            controls
        ]
        .spacing(8),
    )
    .id(InteractionRegion::RuleListEditor.id());

    // DUAL-11-14: the partition list, its order and its i18n keys are the
    // shared workspace vocabulary.
    let tab_labels: Vec<String> = RulesTab::ALL
        .iter()
        .map(|tab| lang.tr(tab.i18n_key()).to_string())
        .collect();
    let tab_index = state.editor.rules_tab.index();
    let tabs = segmented_control(&tab_labels, tab_index, |index| {
        Message::SetRulesTab(RulesTab::from_index(index))
    });

    if !state.editor.rules_heavy_ready {
        return column![
            header,
            Space::new().height(theme::SP_MD),
            tabs,
            Space::new().height(SP_LG),
            card(
                None,
                column![
                    text(lang.tr("page_controls_loading").into_owned())
                        .size(14)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary)
                        }),
                    text(lang.tr("page_controls_loading_hint").into_owned())
                        .size(12)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
                        }),
                ]
                .spacing(theme::SP_SM)
            ),
        ]
        .spacing(10)
        .into();
    }

    let mut available_targets: Vec<String> = state
        .runtime
        .proxies
        .iter()
        .filter(|(_, p): &(&String, &Proxy)| p.is_group())
        .map(|(name, _)| name.clone())
        .collect();
    available_targets.sort();
    if !available_targets.contains(&"DIRECT".to_string()) {
        available_targets.push("DIRECT".to_string());
    }
    if !available_targets.contains(&"REJECT".to_string()) {
        available_targets.push("REJECT".to_string());
    }

    let tab_content: Element<'_, Message> = match state.editor.rules_tab {
        RulesTab::List => {
            modern_scrollable(rules_list::rules_list_view(state, &lang, available_targets))
                .id("rules-page-scroll")
                .height(Length::Fill)
                .into()
        }
        RulesTab::Providers => providers::providers_view(state, &lang),
        RulesTab::JsonEditors => providers::json_editors_view(state, &lang),
        RulesTab::Tracer => modern_scrollable(tracer_view(state, &lang))
            .id(Id::new(TRACER_SCROLL_ID))
            .height(Length::Fill)
            .into(),
    };

    column![
        header,
        if state.editor.rules_tab == RulesTab::List {
            Element::from(draft_panel)
        } else {
            Element::from(Space::new().height(0))
        },
        Space::new().height(theme::SP_MD),
        tabs,
        Space::new().height(theme::SP_MD),
        tab_content
    ]
    .spacing(theme::SP_SM)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_rules_tests.rs"]
mod tests;
