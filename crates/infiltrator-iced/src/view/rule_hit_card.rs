//! Rule Hit Counter and Stale Rule Analyzer component.
//!
//! Renders the application-owned `RuleHitAuditSnapshot` shared with Bevy. The
//! card never computes hit counts, dead rules or CIDR overlaps locally.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card, style_danger, style_ghost};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

fn metric<'a>(
    lang: &Lang<'_>,
    label_key: &str,
    value: String,
    color: fn(&Theme) -> iced::Color,
) -> Element<'a, Message> {
    column![
        text(lang.tr(label_key).to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().height(2.0),
        text(value)
            .size(14)
            .font(FONT_SEMIBOLD)
            .style(move |t: &Theme| text::Style {
                color: Some(color(t))
            }),
    ]
    .width(Length::Fill)
    .into()
}

pub fn rule_hit_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let audit_state = &state.editor.rule_hit_audit;
    let audit = &audit_state.audit;

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
    .on_press_maybe((!audit_state.is_auditing).then_some(Message::AuditStaleRules));

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
    .on_press_maybe(
        (!audit_state.zero_hit_rule_indices.is_empty()).then_some(Message::DisableZeroHitRules),
    );

    let clear_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("rule_hit_btn_clear").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press_maybe(audit.can_clear.then_some(Message::ClearRuleHitCounters));

    let dead_color: fn(&Theme) -> iced::Color = if audit.dead_rules.is_empty() {
        |t: &Theme| tokens(t).success
    } else {
        |t: &Theme| tokens(t).warning
    };

    let metrics_row = row![
        metric(
            lang,
            "rule_hit_total_hits",
            audit.total_hits.to_string(),
            |t: &Theme| tokens(t).accent,
        ),
        metric(
            lang,
            "rule_hit_dead_count",
            audit.dead_rules.len().to_string(),
            dead_color,
        ),
        metric(
            lang,
            "rule_hit_cidr_conflicts",
            audit.cidr_overlaps.len().to_string(),
            |t: &Theme| tokens(t).danger,
        ),
        metric(
            lang,
            "rule_hit_match_latency",
            audit
                .avg_match_latency_us
                .map(|avg| format!("{avg:.1} µs"))
                .unwrap_or_else(|| "—".to_owned()),
            |t: &Theme| tokens(t).text_primary,
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(theme::SP_SM);

    let last_hit: Element<'_, Message> = match audit.last_hit_rule.as_ref() {
        Some(rule) => container(
            row![
                badge("HIT", BadgeKind::Success),
                Space::new().width(theme::SP_XS),
                text(rule.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).accent)
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .into(),
        None => text(lang.tr("rule_hit_none").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            })
            .into(),
    };

    let summary_feedback: Element<'_, Message> = if let Some(sum) = &audit_state.audit_summary {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(sum.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).success)
                    }),
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
            text(lang.tr("rule_hit_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            metrics_row,
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("rule_hit_last_hit").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                Space::new().width(theme::SP_XS),
                last_hit,
            ]
            .align_y(Alignment::Center),
            summary_feedback,
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                clear_btn,
                Space::new().width(theme::SP_SM),
                audit_btn,
                Space::new().width(theme::SP_SM),
                clean_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
