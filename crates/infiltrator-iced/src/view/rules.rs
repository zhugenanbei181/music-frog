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
use crate::view::components::{
    BadgeKind, icon_button, modern_scrollable, page_body_card, segmented_control,
};
use crate::view::rules_tracer::{TRACER_SCROLL_ID, tracer_view};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{SP_LG, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Id, Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_domain::rules::matrix::{RuleTypeFamily, matrix_family, matrix_label};
use infiltrator_presentation::rule_form_binding::form_targets;
use infiltrator_presentation::rule_list_projection::{editor_preview, editor_status};
use infiltrator_presentation::rules_page_model::RulesOperation;
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
    let filtered_count = state.editor.rules_filtered_count();
    let save_rules_action = container(
        button(theme::role_body_text(lang.tr("rules_save_btn").to_string())).on_press_maybe(
            state
                .editor
                .rule_list
                .can_save()
                .then_some(Message::SaveRules),
        ),
    )
    .id(InteractionRegion::RuleListSave.id());
    let discard = container(
        button(theme::role_body_text(
            lang.tr("rules_draft_discard").to_string(),
        ))
        .on_press_maybe(
            (!state.editor.rule_list.is_pending()
                && !state.editor.rule_list.is_awaiting_readback()
                && (state.editor.rule_list.dirty() || state.editor.rule_list.source_changed()))
            .then_some(Message::RuleList(RuleListAction::Discard)),
        ),
    )
    .id(InteractionRegion::RuleListDiscard.id());

    let header = row![
        theme::role_body_text(lang.tr("rules_title").to_string()).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary)
        }),
        Space::new().width(theme::SP_MD),
        theme::role_body_text(format!(
            "{} / {}",
            filtered_count,
            state.editor.rule_list.draft.len()
        ))
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary)
        }),
        Space::new().width(Length::Fill),
        if state
            .editor
            .rules_model
            .operation(RulesOperation::Load)
            .is_running()
        {
            Element::from(theme::role_body_text("...").style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }))
        } else {
            icon_button(Icon::RefreshCw, 16.0, Message::LoadRules)
        },
        Space::new().width(theme::SP_SM),
    ]
    .align_y(Alignment::Center);

    let preview = editor_preview(&state.editor.rule_list, &Lang(&state.shell.lang)).join("\n");
    let controls = row![
        save_rules_action,
        discard,
        button(theme::role_body_text(
            lang.tr("dns_query_settings").to_string()
        ))
        .on_press_maybe(
            state
                .editor
                .rule_list
                .can_guide()
                .then_some(Message::RuleList(RuleListAction::Settings))
        )
    ]
    .spacing(8)
    .wrap();
    let draft_panel = container({
        // The preview row is a row of the draft panel only when there is a
        // preview: an empty string still lays out a line box, and the
        // density judge measures the phantom line between the status and
        // the controls as a 39px dead band inside the page's own span.
        let mut panel = column![
            container(theme::role_body_text(editor_status(
                &state.editor.rule_list,
                &Lang(&state.shell.lang)
            )))
            .id(InteractionRegion::RuleListStatus.id())
        ]
        .spacing(8);
        if !preview.is_empty() {
            panel = panel.push(
                container(theme::role_body_text(preview))
                    .id(InteractionRegion::RuleListPreview.id()),
            );
        }
        panel.push(controls)
    })
    .id(InteractionRegion::RuleListEditor.id());

    // DUAL-11-14: the partition list, its order and its i18n keys are the
    // shared workspace vocabulary.
    let tab_labels: Vec<String> = RulesTab::ALL
        .iter()
        .map(|tab| lang.tr(tab.i18n_key()).to_string())
        .collect();
    let tab_index = state.editor.rules_model.tab_index();
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
                    theme::role_body_text(lang.tr("page_controls_loading").into_owned()).style(
                        |t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary)
                        }
                    ),
                    theme::role_body_text(lang.tr("page_controls_loading_hint").into_owned())
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

    // The rule document's target catalogue is authoritative. The old view-local
    // list sorted live proxy groups and appended literal DIRECT / REJECT, so it
    // could offer a target the commit path rejects and hide one it accepts.
    let available_targets = form_targets(&state.editor.rule_list);

    let tab_content: Element<'_, Message> = match state.editor.rules_model.tab() {
        // The list owns its own scroll window, so the page does not wrap it in
        // a second one: the frame below is the page body, and the list body
        // takes the leftover inside it.
        RulesTab::List => rules_list::rules_list_view(state, &lang, available_targets),
        RulesTab::Providers => providers::providers_view(state, &lang),
        RulesTab::JsonEditors => providers::json_editors_view(state, &lang),
        RulesTab::Tracer => modern_scrollable(tracer_view(state, &lang))
            .id(Id::new(TRACER_SCROLL_ID))
            .height(Length::Fill)
            .into(),
    };

    // One frame for the page body: the card's own ground is what makes the
    // body's last pixel used, and a content-sized list would leave the rest of
    // the body as page-coloured tail air.
    //
    // The gaps are explicit and the column's own spacing is zero, so one seam
    // is one token: a column spacing on top of an explicit spacer pays for the
    // same seam twice, and the density judge bills the page for the ground it
    // leaves between the tabs and the frame (the duplicated seam cost 5.6% of
    // the body against a 4% limit before it was stated once).
    let mut body = column![header].spacing(0.0).height(Length::Fill);
    if state.editor.rules_model.tab() == RulesTab::List {
        // The heading and the editor strip read as one cluster, so the seam
        // between them is the tight step: at 720x480 the four pixels the
        // looser step added were the whole 4% band budget for this shape
        // (`docs/SPACE_UTILIZATION_STANDARD.md` §5, 提升密度).
        body = body
            .push(Space::new().height(theme::SP_XS))
            .push(Element::from(draft_panel));
    }
    body.push(Space::new().height(theme::SP_MD))
        .push(tabs)
        .push(Space::new().height(theme::SP_MD))
        .push(page_body_card(tab_content))
        .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_rules_tests.rs"]
mod tests;
