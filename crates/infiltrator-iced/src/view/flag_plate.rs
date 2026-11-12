//! One square-corner identity plate for a proxy node's country.
//!
//! A flag is rectangular artwork from the shared catalog. It is never a
//! Unicode emoji glyph (whose wavy edge and even shape depend on the host's
//! font) and never sits inside a rounded or circular container. A routing
//! target or unknown region gets a square-corner plate with its own text.

use crate::flag_assets_generated::{FLAG_CODES, flag_svg};
use crate::view::theme::{self, tokens};
use iced::widget::{container, svg, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_contract::design_resources::TextRole;
use std::collections::HashMap;
use std::sync::LazyLock;

const PLATE_WIDTH: f32 = 24.0;
const PLATE_HEIGHT: f32 = 18.0;

static HANDLES: LazyLock<HashMap<&'static str, svg::Handle>> = LazyLock::new(|| {
    let mut handles = HashMap::new();
    for code in FLAG_CODES {
        if let Some(bytes) = flag_svg(code) {
            handles.insert(*code, svg::Handle::from_memory(bytes));
        }
    }
    handles
});

fn handle(code: &str) -> Option<svg::Handle> {
    HANDLES.get(code.to_ascii_uppercase().as_str()).cloned()
}

fn plate<'a, Message: 'a>(body: Element<'a, Message>, fallback: bool) -> Element<'a, Message> {
    container(body)
        .width(Length::Fixed(PLATE_WIDTH))
        .height(Length::Fixed(PLATE_HEIGHT))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |t: &Theme| container::Style {
            background: fallback.then(|| theme::tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(0.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn label<'a, Message: 'a>(label: &str) -> Element<'a, Message> {
    theme::role_text(label.to_owned(), TextRole::Tag)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        })
        .into()
}

/// Draw the plate identified by an uppercase ISO code, or a square fallback
/// tile carrying the same code when no rectangular asset exists.
pub fn flag_plate<'a, Message: 'a>(code: Option<&str>) -> Element<'a, Message> {
    match code.and_then(handle) {
        Some(handle) => plate(
            svg(handle)
                .width(Length::Fixed(PLATE_WIDTH))
                .height(Length::Fixed(PLATE_HEIGHT))
                .content_fit(iced::ContentFit::Contain)
                .into(),
            false,
        ),
        None => plate(label(code.unwrap_or("—")), true),
    }
}

/// Draw the plate for a name-derived optional flag code.
pub fn flag_plate_coded<'a, Message: 'a>(code: Option<&'static str>) -> Element<'a, Message> {
    flag_plate(code)
}

/// Draw the plate for an already-classified code or routing word.
pub fn flag_plate_label<'a, Message: 'a>(label_text: &str) -> Element<'a, Message> {
    match handle(label_text) {
        Some(handle) => plate(
            svg(handle)
                .width(Length::Fixed(PLATE_WIDTH))
                .height(Length::Fixed(PLATE_HEIGHT))
                .content_fit(iced::ContentFit::Contain)
                .into(),
            false,
        ),
        None => plate(label(label_text), true),
    }
}

#[cfg(test)]
#[path = "../../tests/gui/flag_plate_tests.rs"]
mod tests;
