//! Real-time traffic card: the waveform chart plus its scale label and
//! up/down speed legend.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::card_surface;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CHIP, tokens};
use crate::view::waveform::TrafficChart;
use iced::widget::{Space, canvas, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn traffic_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let (up_speed, down_speed) = state
        .diag
        .traffic
        .as_ref()
        .map(|t| (t.up, t.down))
        .unwrap_or((0, 0));

    let speed_legend = row![
        speed_pill(Icon::ArrowUp, up_speed, |t| tokens(t).success),
        Space::new().width(theme::SP_MD),
        speed_pill(Icon::ArrowDown, down_speed, |t| tokens(t).accent),
    ]
    .align_y(Alignment::Center);
    let scale = if state.runtime.traffic_waveform.is_drawable() {
        state.runtime.traffic_scale.clone()
    } else {
        let upload: Vec<f64> = state
            .diag
            .traffic_history
            .iter()
            .map(|(up, _)| *up as f64)
            .collect();
        let download: Vec<f64> = state
            .diag
            .traffic_history
            .iter()
            .map(|(_, down)| *down as f64)
            .collect();
        infiltrator_domain::traffic_scale::compute_from_rates(&upload, &download, 0)
    };

    let card_header = row![
        row![
            icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_traffic").into_owned())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        text(format!(
            "{} {}",
            lang.tr("overview_scale_max"),
            scale.format_max()
        ))
        .size(10)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
        Space::new().width(theme::SP_SM),
        speed_legend,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let chart = canvas::Canvas::new(TrafficChart {
        history: state.diag.traffic_history.clone(),
        shared: state
            .runtime
            .traffic_waveform
            .is_drawable()
            .then(|| state.runtime.traffic_waveform.clone()),
        scale: Some(scale),
    })
    .width(Length::Fill)
    .height(Length::Fixed(130.0));

    container(column![card_header, Space::new().height(theme::SP_MD), chart].spacing(theme::SP_XS))
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(card_surface)
        .into()
}

fn speed_pill<'a>(
    glyph: Icon,
    bytes_per_second: u64,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    container(
        row![
            icon_themed(glyph, 13.0, color),
            Space::new().width(theme::SP_XS),
            text(format!(
                "{}/s",
                crate::utils::format_bytes(bytes_per_second)
            ))
            .size(13)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(color(t))
            }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(move |t: &Theme| {
        let c = color(t);
        container::Style {
            background: Some(Color { a: 0.10, ..c }.into()),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: theme::HAIRLINE,
                color: Color { a: 0.20, ..c },
            },
            ..Default::default()
        }
    })
    .into()
}
