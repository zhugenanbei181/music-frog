//! Rules management page (分流规则与提供者管理):
//! - Rule Tracer sandbox for testing routing matches against domain/IP queries.
//! - Custom rules list with live filter, semantic badge coloring, enable toggle, reordering, and pagination.
//! - Providers management for proxy and rule providers (with diff inspect, unpack, and update).
//! - Geo databases updater for official MetaCubeX Geo data assets.
//! - Token-driven lazy JSON editors for rule providers, proxy providers, and sniffer.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::view::components::{BadgeKind, card, icon_button, segmented_control};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, SP_LG, tokens};
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_domain::rules::matrix::RuleTypeFamily;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) mod providers;
pub(crate) mod rules_list;

/// Format raw rule type into the shared semantic display label (`Domain`,
/// `DomainSuffix`, `IPCIDR`, `GeoIP`, `Match`, `RuleSet`, …) for all 33
/// catalogue spellings (DUAL-11-01). Unknown spellings keep their raw text.
pub fn display_rule_type(rule_type: &str) -> String {
    infiltrator_domain::rules::matrix::matrix_label(rule_type)
}

/// Map rule type and classifier to the shared badge palette (DUAL-11-01). The
/// family comes from the shared catalogue, so every known type is colored by
/// semantics rather than by a per-surface spelling list; `kind` is only the
/// fallback for a spelling the catalogue does not know.
pub fn semantic_badge_kind(rule_type: &str, kind: RuleBadgeKind) -> BadgeKind {
    match infiltrator_domain::rules::matrix::matrix_family(rule_type) {
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
    let save_rules_action = rules_list::save_action(
        state.editor.rules_dirty,
        state.editor.is_saving_rules,
        lang.tr("rules_save_btn").to_string(),
        lang.tr("rules_saved").to_string(),
        Message::SaveRules,
    );

    let header = row![
        text(lang.tr("rules_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(theme::SP_MD),
        text(format!("{} / {}", filtered_count, state.editor.rules.len()))
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
        save_rules_action,
    ]
    .align_y(Alignment::Center);

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
                    text("Preparing Rules panels...")
                        .size(14)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary)
                        }),
                    text("Heavy widgets mount asynchronously to keep first paint responsive.")
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
        .filter(|(_, p): &(&String, &infiltrator_domain::proxy::Proxy)| p.is_group())
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
        RulesTab::List => rules_list::rules_list_view(state, &lang, available_targets),
        RulesTab::Providers => providers::providers_view(state, &lang),
        RulesTab::JsonEditors => providers::json_editors_view(state, &lang),
        RulesTab::Tracer => crate::view::rules_tracer::tracer_view(state, &lang),
    };

    column![
        header,
        Space::new().height(theme::SP_MD),
        tabs,
        Space::new().height(theme::SP_MD),
        tab_content
    ]
    .spacing(SP_LG)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_rules_tests.rs"]
mod tests;
