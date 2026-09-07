//! Iced adapter for the shared active-subscription quota dashboard.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{BadgeKind, badge, card};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::subscription_quota::{
    SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// Overview quota dashboard. Every number is optional in the shared model;
/// this view renders an em dash or explicit “not reported” when the provider
/// did not supply it.
pub fn subscription_quota_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    let snapshot = &state.runtime.subscription_quota;
    let used = snapshot
        .used_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_owned());
    let total = snapshot
        .total_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_owned());
    let remaining = snapshot
        .remaining_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_owned());
    let usage = snapshot
        .usage_percent
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "—".to_owned());
    let remaining_pct = snapshot
        .remaining_percent
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "—".to_owned());
    let profile = snapshot
        .profile_name
        .clone()
        .unwrap_or_else(|| lang.tr("overview_subscription_quota_no_profile").to_string());
    let expiry = snapshot
        .expires_at_label
        .clone()
        .unwrap_or_else(|| lang.tr("overview_subscription_quota_expiry_unknown").to_string());
    let expiry = match snapshot.remaining_days {
        Some(days) if snapshot.expires_at_label.is_some() => format!("{expiry} · {days}d"),
        _ => expiry,
    };
    let reset = snapshot
        .reset_days
        .map(|days| format!("reset in {days}d"))
        .unwrap_or_else(|| lang.tr("overview_subscription_quota_reset_unknown").to_string());
    let status = status_label(snapshot, lang);

    let progress = usage_bar(snapshot);
    let metrics = row![
        metric(lang.tr("sub_quota_used").to_string(), used.clone()),
        metric(
            format!("{} · {}", lang.tr("sub_quota_remaining"), remaining_pct),
            remaining,
        ),
        metric(
            format!("{} · {}", lang.tr("overview_connections"), usage),
            total.clone(),
        ),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill);

    card(
        Some(lang.tr("overview_subscription_quota").to_string()),
        column![
            row![
                icon_themed(Icon::FileText, 14.0, |t: &Theme| tokens(t).accent),
                Space::new().width(theme::SP_XS),
                text(profile)
                    .size(13)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(Length::Fill),
                badge(status, status_kind(snapshot.status)),
            ]
            .align_y(Alignment::Center),
            text(expiry)
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            metrics,
            progress,
            row![
                text(format!("used {used} / total {total} · {usage}"))
                    .size(10)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().width(Length::Fill),
                text(reset)
                    .size(10)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

fn metric<'a>(label: String, value: String) -> Element<'a, Message> {
    column![
        text(label)
            .size(10)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        text(value)
            .size(12)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
    ]
    .spacing(2)
    .width(Length::Fill)
    .into()
}

fn usage_bar<'a>(snapshot: &SubscriptionQuotaSnapshot) -> Element<'a, Message> {
    let fraction = snapshot.usage_fraction();
    let fill: Element<'a, Message> = if fraction > 0.0 {
        container(Space::new())
            .width(Length::FillPortion(
                (fraction * 100.0).ceil().clamp(1.0, 100.0) as u16,
            ))
            .height(Length::Fill)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).accent.into()),
                border: Border {
                    radius: border::Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::new().width(Length::Shrink).into()
    };
    container(row![fill, Space::new().width(Length::Fill)])
        .width(Length::Fill)
        .height(8)
        .style(|t: &Theme| container::Style {
            background: Some(Color { a: 0.25, ..tokens(t).card_border }.into()),
            border: Border {
                radius: border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn status_label(
    snapshot: &SubscriptionQuotaSnapshot,
    lang: &Lang<'_>,
) -> String {
    match snapshot.status {
        SubscriptionQuotaStatus::Ready => lang.tr("overview_subscription_quota_healthy").to_string(),
        SubscriptionQuotaStatus::Warning => lang.tr("overview_subscription_quota_warning").to_string(),
        SubscriptionQuotaStatus::Critical => lang.tr("overview_subscription_quota_critical").to_string(),
        SubscriptionQuotaStatus::Exhausted => lang.tr("overview_subscription_quota_exhausted").to_string(),
        SubscriptionQuotaStatus::Expired => lang.tr("overview_subscription_quota_expired").to_string(),
        SubscriptionQuotaStatus::ExpiringSoon => lang.tr("overview_subscription_quota_expiring").to_string(),
        SubscriptionQuotaStatus::Empty => lang.tr("overview_subscription_quota_no_profile").to_string(),
        SubscriptionQuotaStatus::Unknown => "pending".to_owned(),
        SubscriptionQuotaStatus::Unsupported | SubscriptionQuotaStatus::Failed => snapshot
            .failure
            .clone()
            .unwrap_or_else(|| "unavailable".to_owned()),
    }
}

fn status_kind(status: SubscriptionQuotaStatus) -> BadgeKind {
    match status {
        SubscriptionQuotaStatus::Ready => BadgeKind::Success,
        SubscriptionQuotaStatus::Warning | SubscriptionQuotaStatus::ExpiringSoon => {
            BadgeKind::Warning
        }
        SubscriptionQuotaStatus::Critical
        | SubscriptionQuotaStatus::Exhausted
        | SubscriptionQuotaStatus::Expired
        | SubscriptionQuotaStatus::Failed => BadgeKind::Danger,
        SubscriptionQuotaStatus::Unknown
        | SubscriptionQuotaStatus::Empty
        | SubscriptionQuotaStatus::Unsupported => BadgeKind::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_status_label_has_no_fake_success_for_missing_metadata() {
        let lang = Lang("en-US");
        let snapshot = SubscriptionQuotaSnapshot::unsupported(1, 1, "provider unavailable");
        assert_eq!(status_label(&snapshot, &lang), "provider unavailable");
        assert_eq!(status_kind(snapshot.status), BadgeKind::Neutral);
    }
}
