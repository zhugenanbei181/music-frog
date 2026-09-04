//! Rule Hit Counter and Stale Rule Analyzer component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_danger, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn rule_hit_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let audit = &state.editor.rule_hit_audit;

    let audit_btn = button(
        row![
            svg_icons::icon_themed(Icon::Search, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("rule_hit_btn_audit").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press_maybe((!audit.is_auditing).then_some(Message::AuditStaleRules));

    let clean_btn = button(
        row![
            svg_icons::icon_themed(Icon::Trash2, 12.0, |t: &Theme| tokens(t).danger),
            Space::new().width(theme::SP_XS),
            text(lang.tr("rule_hit_btn_clean").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_danger)
    .on_press_maybe((!audit.zero_hit_rule_indices.is_empty()).then_some(Message::DisableZeroHitRules));

    let metrics_row = row![
        column![
            text(lang.tr("rule_hit_total_hits").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format!("{}", audit.total_rule_hits)).size(14).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).accent) }),
        ].width(Length::Fill),
        column![
            text("0-Hit Rules").size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            badge(format!("{} stale", audit.zero_hit_rule_indices.len()), if audit.zero_hit_rule_indices.is_empty() { BadgeKind::Success } else { BadgeKind::Warning }),
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let summary_feedback: Element<'_, Message> = if let Some(sum) = &audit.audit_summary {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(sum.clone()).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("rule_hit_title").to_string()),
        column![
            text(lang.tr("rule_hit_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            metrics_row,
            summary_feedback,
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                audit_btn,
                Space::new().width(theme::SP_SM),
                clean_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
