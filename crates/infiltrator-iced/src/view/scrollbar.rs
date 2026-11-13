//! Shared Iced scroll regions built from the canonical scrollbar contract.
use crate::view::theme;
use iced::advanced::text::Renderer;
use iced::widget::{Scrollable, container, scrollable};
use iced::{Background, Border, Color, Element, Padding, Shadow, Theme};
use infiltrator_contract::scrollbar::ScrollbarMetrics;
/// Content clearance owned by the shared floating rail.
pub const SCROLLBAR_GUTTER: f32 = ScrollbarMetrics::DEFAULT.content_gutter_px;
/// Iced's realization of the canonical scrollbar geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarGeometry {
    pub track_width_px: f32,
    pub margin_px: f32,
    pub scroller_width_px: f32,
}
/// Canonical slim, pill-shaped rail geometry used by every Iced scroll region.
pub(crate) fn canonical_geometry() -> ScrollbarGeometry {
    let metrics = ScrollbarMetrics::DEFAULT;
    ScrollbarGeometry {
        track_width_px: metrics.track_width_px,
        margin_px: metrics.thumb_margin_px,
        scroller_width_px: metrics.track_width_px,
    }
}
/// Canonical slim, pill-shaped rail geometry used by every Iced scroll region.
pub(crate) fn canonical_scrollbar() -> scrollable::Scrollbar {
    let geometry = canonical_geometry();
    scrollable::Scrollbar::new()
        .width(geometry.track_width_px)
        .margin(geometry.margin_px)
        .scroller_width(geometry.scroller_width_px)
}
fn vertical_direction() -> scrollable::Direction {
    scrollable::Direction::Vertical(canonical_scrollbar())
}
fn guttered<'a, T, R>(
    content: impl Into<Element<'a, T, Theme, R>>,
) -> container::Container<'a, T, Theme, R>
where
    T: 'a,
    R: Renderer + 'a,
{
    container(content).padding(Padding {
        top: 0.0,
        right: SCROLLBAR_GUTTER,
        bottom: 0.0,
        left: 0.0,
    })
}
/// A full-page scroll region following the shared auto-hide contract.
pub fn modern_scrollable<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Scrollable<'a, T> {
    scrollable(guttered(content))
        .direction(vertical_direction())
        .style(scrollable_style)
}
/// A nested scroll region (card, drawer or modal) following the same contract.
pub fn auto_hide_scrollable<'a, T, R>(
    content: impl Into<Element<'a, T, Theme, R>>,
) -> Scrollable<'a, T, Theme, R>
where
    T: 'a,
    R: Renderer + 'a,
{
    scrollable(guttered(content))
        .direction(vertical_direction())
        .style(scrollable_style)
}

/// A vertical scroll region without the right clearance.
///
/// The clearance belongs to a page-level region: a region that lives inside
/// another scroll (the body of a table that scrolls sideways) must not
/// reserve room for a rail that is not there.
pub fn vertical_scrollable<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Scrollable<'a, T> {
    scrollable(content)
        .direction(vertical_direction())
        .style(scrollable_style)
}

/// A sideways scroll region for a table that is wider than the page.
pub fn horizontal_scrollable<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Scrollable<'a, T> {
    scrollable(content)
        .direction(scrollable::Direction::Horizontal(canonical_scrollbar()))
        .style(scrollable_style)
}
/// Scrollbar appearance per the shared contract: each axis rests invisible and
/// only its own rail responds to pointer hover or drag.
pub fn scrollable_style(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let metrics = ScrollbarMetrics::DEFAULT;
    let ink = theme::tokens(theme).text_tertiary;
    let (vertical, horizontal) = match status {
        scrollable::Status::Active { .. } => (false, false),
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered,
            is_vertical_scrollbar_hovered,
            ..
        } => (
            is_vertical_scrollbar_hovered,
            is_horizontal_scrollbar_hovered,
        ),
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged,
            is_vertical_scrollbar_dragged,
            ..
        } => (
            is_vertical_scrollbar_dragged,
            is_horizontal_scrollbar_dragged,
        ),
    };
    scrollable::Style {
        container: container::Style {
            background: None,
            ..Default::default()
        },
        vertical_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scroller(ink, metrics, vertical),
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scroller(ink, metrics, horizontal),
        },
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color { a: 0.0, ..ink }),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}
fn scroller(ink: Color, metrics: ScrollbarMetrics, visible: bool) -> scrollable::Scroller {
    let opacity = if visible {
        metrics.active_opacity
    } else {
        metrics.idle_opacity
    };
    scrollable::Scroller {
        background: Background::Color(Color {
            a: ink.a * opacity,
            ..ink
        }),
        border: Border {
            radius: metrics.thumb_radius_px.into(),
            ..Default::default()
        },
    }
}
#[cfg(test)]
#[path = "../../tests/gui/scrollbar_contract_tests.rs"]
mod tests;
