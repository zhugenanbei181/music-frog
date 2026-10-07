//! Native traffic legend; observed copy and optional peaks come from the shared readout.
use crate::types::message::Message;
use crate::view::theme;
use crate::view::theme::{MONO, tokens};
use iced::advanced::text::Renderer;
use iced::widget::{Space, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Theme, border};

/// Legend item showing colored bullet, label, current speed and optional peak.
pub(crate) fn legend_indicator<'a, R>(
    color: Color,
    label: &str,
    current_speed: &str,
    peak_speed: Option<String>,
) -> Element<'a, Message, Theme, R>
where
    R: Renderer<Font = Font> + 'a,
{
    let bullet =
        container(Space::new().width(8).height(8)).style(move |_t: &Theme| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: border::Radius::from(theme::R_PILL),
                ..Default::default()
            },
            ..Default::default()
        });

    let mut content = row![
        bullet,
        Space::new().width(theme::SP_XS),
        text(label.to_string())
            .size(11)
            .font(theme::FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_XS),
        text(current_speed.to_string())
            .size(11)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
    ]
    .align_y(Alignment::Center);

    if let Some(peak) = peak_speed {
        content = content.push(Space::new().width(theme::SP_XS)).push(
            text(format!("({peak})"))
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }

    content.into()
}
