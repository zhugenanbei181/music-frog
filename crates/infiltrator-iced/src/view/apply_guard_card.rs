//! Config Apply Multi-Stage Transaction Guard component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::ApplyTransactionStage;
use crate::view::components::{badge, card, style_accent, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn apply_guard_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let guard = &state.runtime.apply_guard;

    let trigger_btn = button(
        row![
            svg_icons::icon_themed(Icon::Zap, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text("Test Atomic Transaction").size(11).font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press(Message::TriggerAtomicConfigApply);

    let (stage_label, stage_badge_kind): (String, BadgeKind) = match &guard.stage {
        ApplyTransactionStage::Idle => ("Idle".to_string(), BadgeKind::Neutral),
        ApplyTransactionStage::Preflight => (lang.tr("apply_guard_stage_preflight").to_string(), BadgeKind::Warning),
        ApplyTransactionStage::Reloading => (lang.tr("apply_guard_stage_reloading").to_string(), BadgeKind::Warning),
        ApplyTransactionStage::Probing => (lang.tr("apply_guard_stage_probing").to_string(), BadgeKind::Accent),
        ApplyTransactionStage::Committed => (lang.tr("apply_guard_status_committed").to_string(), BadgeKind::Success),
        ApplyTransactionStage::RolledBack(_) => (lang.tr("apply_guard_status_rolled_back").to_string(), BadgeKind::Danger),
    };

    let stages_flow = row![
        text("1. Preflight").size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        text(" ➔ ").size(11),
        text("2. Staging").size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        text(" ➔ ").size(11),
        text("3. Core Reload").size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        text(" ➔ ").size(11),
        text("4. Health Probe").size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        text(" ➔ ").size(11),
        text("5. Commit / Rollback").size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
    ]
    .align_y(Alignment::Center);

    card(
        Some(lang.tr("apply_guard_title").to_string()),
        column![
            text(lang.tr("apply_guard_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            stages_flow,
            Space::new().height(theme::SP_XS),
            row![
                badge(stage_label.to_string(), stage_badge_kind),
                Space::new().width(Length::Fill),
                trigger_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
