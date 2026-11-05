use super::*;
use iced::Background;
use infiltrator_contract::scrollbar::ScrollbarMetrics;

fn scroller_alpha(style: &scrollable::Style, vertical: bool) -> f32 {
    let rail = if vertical {
        &style.vertical_rail
    } else {
        &style.horizontal_rail
    };
    match rail.scroller.background {
        Background::Color(color) => color.a,
        Background::Gradient(_) => panic!("the canonical scrollbar uses a flat ink wash"),
    }
}

#[test]
fn hovering_scrollable_content_does_not_reveal_its_scrollbars() {
    let style = scrollable_style(
        &Theme::Dark,
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: false,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        },
    );

    assert_eq!(scroller_alpha(&style, true), 0.0);
    assert_eq!(scroller_alpha(&style, false), 0.0);
}

#[test]
fn only_the_interacted_scrollbar_axis_becomes_visible() {
    let style = scrollable_style(
        &Theme::Dark,
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        },
    );
    let metrics = ScrollbarMetrics::DEFAULT;
    let ink = theme::tokens(&Theme::Dark).text_tertiary;

    assert_eq!(scroller_alpha(&style, true), ink.a * metrics.active_opacity);
    assert_eq!(scroller_alpha(&style, false), 0.0);
}

#[test]
fn dragging_one_axis_reveals_only_that_axis() {
    let style = scrollable_style(
        &Theme::Dark,
        scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged: true,
            is_vertical_scrollbar_dragged: false,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        },
    );
    let metrics = ScrollbarMetrics::DEFAULT;
    let ink = theme::tokens(&Theme::Dark).text_tertiary;

    assert_eq!(scroller_alpha(&style, true), 0.0);
    assert_eq!(
        scroller_alpha(&style, false),
        ink.a * metrics.active_opacity
    );
}

#[test]
fn canonical_geometry_uses_the_contracted_pill_rail() {
    let geometry = canonical_geometry();
    let metrics = ScrollbarMetrics::DEFAULT;

    assert_eq!(geometry.track_width_px, metrics.track_width_px);
    assert_eq!(geometry.margin_px, metrics.thumb_margin_px);
    assert_eq!(geometry.scroller_width_px, metrics.track_width_px);
}
