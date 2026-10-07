//! Canonical scissor-clipped card for native and headless renderers.
use crate::view::components::card_surface;
use crate::view::theme;
use iced::advanced::text::Renderer;
use iced::widget::{column, container, text};
use iced::{Element, Font, Length, Theme};

/// The canonical card with hardware scissor clipping enabled (`clip(true)`),
/// preventing overflowing child elements from piercing the rounded card boundary.
pub fn card<'a, Message: 'a, R: Renderer<Font = Font> + 'a>(
    title: Option<String>,
    content: impl Into<Element<'a, Message, Theme, R>>,
) -> Element<'a, Message, Theme, R> {
    let content = content.into();
    let body = match title {
        Some(title) => {
            let header = text(title)
                .size(14)
                .font(theme::FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary),
                });
            column![header, content]
                .spacing(theme::SP_MD)
                .width(Length::Fill)
                .into()
        }
        None => content,
    };
    container(body)
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(card_surface)
        .clip(true)
        .into()
}
