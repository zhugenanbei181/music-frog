//! Iced adapter for the shared active-subscription quota dashboard.

use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_application::subscription_quota_projection::{QuotaGrade, project_quota};

use crate::view::component_card::card;
use crate::view::components::{BadgeKind, badge};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::advanced::Renderer;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

/// Overview quota dashboard. Every number is optional in the shared model;
/// this view renders an em dash or explicit “not reported” when the provider
/// did not supply it.
pub fn subscription_quota_card<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let snapshot = &state.runtime.subscription_quota;
    let presentation = project_quota(snapshot, lang.0);
    let progress = presentation
        .fraction
        .map(usage_bar)
        .unwrap_or_else(|| Space::new().height(0).into());
    let metrics = row![
        metric(
            lang.tr("sub_quota_used").to_string(),
            presentation.used.clone()
        ),
        metric(
            format!(
                "{} · {}",
                lang.tr("sub_quota_remaining"),
                presentation.remaining_percent
            ),
            presentation.remaining.clone(),
        ),
        metric(
            format!("{} · {}", lang.tr("quota_total_label"), presentation.usage),
            presentation.total.clone(),
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
                text(presentation.profile)
                    .size(13)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(Length::Fill),
                badge(presentation.status, status_kind(presentation.grade)),
            ]
            .align_y(Alignment::Center),
            text(presentation.expiry)
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            metrics,
            progress,
            row![
                text(presentation.metrics)
                    .size(10)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().width(Length::Fill),
                text(presentation.reset)
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

fn usage_bar<'a, R: Renderer + 'a>(fraction: f32) -> Element<'a, Message, Theme, R> {
    let fill_units = (fraction * 10000.0).round().clamp(0.0, 10000.0) as u16;
    let remaining_units = 10000 - fill_units;
    let fill: Element<'a, Message, Theme, R> = if fraction > 0.0 {
        container(Space::new())
            .width(Length::FillPortion(fill_units.max(1)))
            .height(Length::Fill)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).accent.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_XS),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::new().width(Length::Shrink).into()
    };
    let remaining = Space::new().width(if remaining_units == 0 {
        Length::Shrink
    } else {
        Length::FillPortion(remaining_units)
    });
    container(row![fill, remaining])
        .width(Length::Fill)
        .height(8)
        .style(|t: &Theme| container::Style {
            background: Some(
                Color {
                    a: 0.25,
                    ..tokens(t).card_border
                }
                .into(),
            ),
            border: Border {
                radius: border::Radius::from(theme::R_XS),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn status_kind(grade: QuotaGrade) -> BadgeKind {
    match grade {
        QuotaGrade::Observed => BadgeKind::Accent,
        QuotaGrade::Warning => BadgeKind::Warning,
        QuotaGrade::Danger => BadgeKind::Danger,
        QuotaGrade::Unknown => BadgeKind::Neutral,
    }
}

#[cfg(test)]
#[path = "../../tests/gui/subscription_quota_tests.rs"]
mod subscription_quota_tests;
