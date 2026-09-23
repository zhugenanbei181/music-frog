//! Multi-target latency comparison bars.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{card_surface, icon_button};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CHIP, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn latency_card<'a>(
    _state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
) -> Element<'a, Message> {
    let avg_pill = container(
        row![
            icon_themed(Icon::Activity, 11.0, |t| tokens(t).success),
            Space::new().width(theme::SP_XS),
            text(lang.tr("overview_avg_latency").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([3, 9])
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(
                Color {
                    a: 0.12,
                    ..tk.success
                }
                .into(),
            ),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: theme::HAIRLINE,
                color: Color {
                    a: 0.25,
                    ..tk.success
                },
            },
            ..Default::default()
        }
    });

    let card_header = row![
        row![
            icon_themed(Icon::Target, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("runtime_delay_title").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        avg_pill,
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let bars = column![
        latency_comparison_bar("Google", 180, 300.0),
        latency_comparison_bar("Cloudflare", 178, 300.0),
        latency_comparison_bar("GitHub", 182, 300.0),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill);

    container(column![card_header, Space::new().height(theme::SP_MD), bars].spacing(theme::SP_XS))
        .width(Length::FillPortion(1))
        .padding(theme::SP_XXL)
        .style(card_surface)
        .into()
}

fn latency_comparison_bar<'a>(name: &'static str, ms: u32, max_ms: f32) -> Element<'a, Message> {
    let fill_pct = (ms as f32 / max_ms).clamp(0.05, 1.0);
    let fill_portion = (fill_pct * 100.0) as u16;
    let empty_portion = (100 - fill_portion).max(1);

    let bar = container(
        row![
            container(
                Space::new()
                    .width(Length::FillPortion(fill_portion))
                    .height(6)
            )
            .style(move |t: &Theme| {
                let c = theme::latency_color(tokens(t), Some(ms));
                container::Style {
                    background: Some(c.into()),
                    border: Border {
                        radius: border::Radius::from(3.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
            Space::new().width(Length::FillPortion(empty_portion)),
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(6)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: border::Radius::from(3.0),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    row![
        text(name)
            .size(12)
            .font(FONT_MEDIUM)
            .width(Length::Fixed(80.0))
            .style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_primary),
                }
            }),
        bar,
        Space::new().width(theme::SP_MD),
        text(format!("{ms} ms"))
            .size(12)
            .font(MONO)
            .width(Length::Fixed(55.0))
            .style(move |t: &Theme| {
                text::Style {
                    color: Some(theme::latency_color(tokens(t), Some(ms))),
                }
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}
