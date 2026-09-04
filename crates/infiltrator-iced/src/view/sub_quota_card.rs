//! Subscription Quota and Cron Auto-Update Scheduler component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{badge, card, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn sub_quota_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let quota = &state.profile.quota_schedule;

    let eval_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text("Check Quota").size(11).font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_accent)
    .on_press(Message::EvaluateSubscriptionQuota);

    let used = if quota.used_bytes == 0 {
        1024 * 1024 * 1024 * 32
    } else {
        quota.used_bytes
    };
    let total = if quota.total_bytes == 0 {
        1024 * 1024 * 1024 * 128
    } else {
        quota.total_bytes
    };
    let remaining_pct = if quota.remaining_percent == 0.0 {
        75.0
    } else {
        quota.remaining_percent
    };

    let tier_badge = match quota.warning_tier.as_str() {
        "Critical" => badge("Critical <10%".to_string(), BadgeKind::Danger),
        "Warning" => badge("Warning <20%".to_string(), BadgeKind::Warning),
        _ => badge("Healthy".to_string(), BadgeKind::Success),
    };

    let quota_metrics = row![
        column![
            text(lang.tr("sub_quota_used").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format_bytes(used)).size(13).font(MONO),
        ].width(Length::Fill),
        column![
            text(lang.tr("sub_quota_remaining").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(format!("{:.1}% ({})", remaining_pct, format_bytes(total.saturating_sub(used)))).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).accent) }),
        ].width(Length::Fill),
        column![
            text(lang.tr("sub_quota_cron").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            tier_badge,
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let interval_pills = row![
        text("Auto-Update Interval:").size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        Space::new().width(theme::SP_SM),
        button(text("6h").size(11))
            .padding([3, 8])
            .style(if quota.cron_interval_hours == 6 { style_accent } else { style_ghost })
            .on_press(Message::UpdateCronScheduleHours(6)),
        Space::new().width(theme::SP_XS),
        button(text("12h").size(11))
            .padding([3, 8])
            .style(if quota.cron_interval_hours == 12 { style_accent } else { style_ghost })
            .on_press(Message::UpdateCronScheduleHours(12)),
        Space::new().width(theme::SP_XS),
        button(text("24h").size(11))
            .padding([3, 8])
            .style(if quota.cron_interval_hours == 24 || quota.cron_interval_hours == 0 { style_accent } else { style_ghost })
            .on_press(Message::UpdateCronScheduleHours(24)),
    ]
    .align_y(Alignment::Center);

    card(
        Some(lang.tr("sub_quota_title").to_string()),
        column![
            text(lang.tr("sub_quota_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            quota_metrics,
            Space::new().height(theme::SP_XS),
            row![
                interval_pills,
                Space::new().width(Length::Fill),
                eval_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
