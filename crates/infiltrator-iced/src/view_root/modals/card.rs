//! Shared modal chrome: the scrim backdrop and the floating card surface.

use crate::types::message::Message;
use crate::view::theme::{HAIRLINE, R_CARD, tokens};
use iced::advanced::Renderer;
use iced::widget::{container, mouse_area};
use iced::{Border, Element, Length, Theme, border};

pub(in crate::view_root) fn modal_backdrop<'a, R>(
    dialog: Element<'a, Message, Theme, R>,
) -> Element<'a, Message, Theme, R>
where
    R: Renderer + 'a,
{
    mouse_area(
        container(
            container(dialog)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).scrim.into()),
            ..Default::default()
        }),
    )
    .on_press(Message::Noop)
    .on_right_press(Message::Noop)
    .on_middle_press(Message::Noop)
    .on_scroll(|_| Message::Noop)
    .into()
}

pub(in crate::view_root) fn modal_card<'a, R: Renderer + 'a>(
    content: Element<'a, Message, Theme, R>,
    width: f32,
) -> Element<'a, Message, Theme, R> {
    container(content)
        .width(Length::Fixed(width))
        .padding(24)
        .style(|theme: &Theme| {
            let tk = tokens(theme);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(R_CARD),
                    width: HAIRLINE,
                    color: tk.card_border,
                },
                shadow: tk.floating_shadow,
                text_color: Some(tk.text_primary),
                ..Default::default()
            }
        })
        .into()
}

#[cfg(test)]
#[path = "../../../tests/gui/modal_chrome_tests.rs"]
mod tests;
