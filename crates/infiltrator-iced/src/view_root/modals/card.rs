//! Shared modal chrome: the scrim backdrop and the floating card surface.

use crate::types::message::Message;
use iced::widget::container;
use iced::{Border, Element, Length, Theme};

pub(in crate::view_root) fn modal_backdrop<'a>(
    dialog: Element<'a, Message>,
) -> Element<'a, Message> {
    container(
        container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|t: &Theme| container::Style {
        background: Some(crate::view::theme::tokens(t).scrim.into()),
        ..Default::default()
    })
    .into()
}

pub(in crate::view_root) fn modal_card<'a>(
    content: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .padding(24)
        .style(|theme: &Theme| {
            let tokens = crate::view::theme::tokens(theme);
            container::Style {
                background: Some(tokens.card_bg.into()),
                border: Border {
                    radius: 16.0.into(),
                    width: crate::view::theme::HAIRLINE,
                    color: tokens.card_border,
                },
                shadow: tokens.floating_shadow,
                text_color: Some(tokens.text_primary),
                ..Default::default()
            }
        })
        .into()
}
