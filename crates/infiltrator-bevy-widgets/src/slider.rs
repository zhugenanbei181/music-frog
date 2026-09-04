//! Slider & RangeSlider: our product skin over the official unstyled
//! `bevy_ui_widgets` [`Slider`] along with dual-thumb [`RangeSlider`].
//! The official widget owns single-value behavior while this module owns
//! token tracks, fills, thumb geometry, dual-thumb range projections, and
//! compare-and-set repaint stylists.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Changed, With, Without};
use bevy::ecs::system::Query;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Node, PositionType, Val, percent, px,
};
use bevy::ui_widgets::{Slider, SliderRange, SliderThumb, SliderValue};

use crate::palette::UiPalette;

/// Marker on the fill bar; its width carries the value between range start
/// and the thumb. Pure routing for the repaint system.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliderFill;

/// Marker component on a dual-thumb range slider root node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeSlider;

/// Current values (start and end) for a dual-thumb range slider.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RangeSliderValues {
    pub start: f32,
    pub end: f32,
}

impl RangeSliderValues {
    pub fn new(start: f32, end: f32) -> Self {
        Self { start, end }
    }
}

/// Permissible bounds for a dual-thumb range slider.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RangeSliderRange {
    pub min: f32,
    pub max: f32,
}

impl RangeSliderRange {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

/// Marker on the dual-thumb range slider fill bar between min and max thumbs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeSliderFill;

/// Marker on the lower/start thumb of a range slider.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeSliderThumbMin;

/// Marker on the upper/end thumb of a range slider.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeSliderThumbMax;

/// The single value's travel through its range as 0..=1, clamped.
pub fn slider_fraction(value: f32, start: f32, end: f32) -> f32 {
    SliderRange::new(start, end)
        .thumb_position(value)
        .clamp(0.0, 1.0)
}

/// The dual values' travels through range as (0..=1, 0..=1), clamped. Pure function.
pub fn range_slider_fractions(start_val: f32, end_val: f32, min: f32, max: f32) -> (f32, f32) {
    if max <= min {
        return (0.0, 1.0);
    }
    let range = max - min;
    let s_pct = ((start_val - min) / range).clamp(0.0, 1.0);
    let e_pct = ((end_val - min) / range).clamp(0.0, 1.0);
    (s_pct.min(e_pct), s_pct.max(e_pct))
}

/// Clamp range slider start and end values ensuring valid ordering and min span. Pure function.
pub fn clamp_range_values(start: f32, end: f32, min: f32, max: f32, min_span: f32) -> (f32, f32) {
    let lower_bound = min;
    let upper_bound = max.max(min + min_span);

    let mut s = start.clamp(lower_bound, upper_bound - min_span);
    let mut e = end.clamp(s + min_span, upper_bound);

    if e - s < min_span {
        if s + min_span <= upper_bound {
            e = s + min_span;
        } else {
            s = e - min_span;
        }
    }

    (s, e)
}

/// Snap value to nearest step increment. Pure function.
pub fn step_value(value: f32, step: f32, min: f32, max: f32) -> f32 {
    if step <= 0.0 {
        return value.clamp(min, max);
    }
    let steps = ((value - min) / step).round();
    (min + steps * step).clamp(min, max)
}

/// The token slider chrome over the official behavior primitive.
pub fn slider_scene(value: f32, start: f32, end: f32, palette: &UiPalette) -> impl Scene + use<> {
    let travel = slider_fraction(value, start, end) * 100.0;
    bsn! {
        Node {
            width: percent(100),
            height: px(palette.control_square_px),
            align_items: AlignItems::Center,
        }
        Slider
        SliderValue({ value })
        template_value(SliderRange::new(start, end))
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(palette.track_height_px),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.track_height_px * 0.5,
                    )),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    (
                        Node {
                            width: Val::Percent(travel),
                            height: px(palette.track_height_px),
                            border_radius: BorderRadius::all(Val::Px(
                                palette.track_height_px * 0.5,
                            )),
                        }
                        BackgroundColor({ palette.accent })
                        SliderFill
                    ),
                ]
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(travel),
                    top: px(0.0),
                    width: px(palette.control_square_px),
                    height: px(palette.control_square_px),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_square_px * 0.5,
                    )),
                }
                SliderThumb
                BackgroundColor({ palette.accent })
            ),
        ]
    }
}

/// Dual-thumb range slider scene constructor.
pub fn range_slider_scene(
    start: f32,
    end: f32,
    min: f32,
    max: f32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (s_pct, e_pct) = range_slider_fractions(start, end, min, max);
    let fill_left = s_pct * 100.0;
    let fill_width = (e_pct - s_pct) * 100.0;
    let thumb_min_left = s_pct * 100.0;
    let thumb_max_left = e_pct * 100.0;

    bsn! {
        Node {
            width: percent(100),
            height: px(palette.control_square_px),
            align_items: AlignItems::Center,
        }
        RangeSlider
        RangeSliderValues { start, end }
        RangeSliderRange { min, max }
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(palette.track_height_px),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.track_height_px * 0.5,
                    )),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(fill_left),
                            width: Val::Percent(fill_width),
                            height: px(palette.track_height_px),
                            border_radius: BorderRadius::all(Val::Px(
                                palette.track_height_px * 0.5,
                            )),
                        }
                        BackgroundColor({ palette.accent })
                        RangeSliderFill
                    ),
                ]
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(thumb_min_left),
                    top: px(0.0),
                    width: px(palette.control_square_px),
                    height: px(palette.control_square_px),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_square_px * 0.5,
                    )),
                }
                RangeSliderThumbMin
                BackgroundColor({ palette.accent })
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(thumb_max_left),
                    top: px(0.0),
                    width: px(palette.control_square_px),
                    height: px(palette.control_square_px),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_square_px * 0.5,
                    )),
                }
                RangeSliderThumbMax
                BackgroundColor({ palette.accent })
            ),
        ]
    }
}

/// Restamp fill width and thumb travel from the live [`SliderValue`].
#[allow(clippy::type_complexity)]
pub fn sync_slider_visuals(
    mut sliders: Query<(Entity, &SliderValue, &SliderRange), Changed<SliderValue>>,
    groups: Query<&Children>,
    mut fills: Query<&mut Node, (With<SliderFill>, Without<SliderThumb>)>,
    mut thumbs: Query<&mut Node, With<SliderThumb>>,
) {
    for (entity, value, range) in &mut sliders {
        let travel = slider_fraction(value.0, range.start(), range.end()) * 100.0;
        if !groups.contains(entity) {
            continue;
        }
        for child in groups.iter_descendants(entity) {
            if let Ok(mut fill) = fills.get_mut(child) {
                fill.width = Val::Percent(travel);
            }
            if let Ok(mut thumb) = thumbs.get_mut(child) {
                thumb.left = Val::Percent(travel);
            }
        }
    }
}

/// Restamp dual-thumb range slider fill and thumb positions from live [`RangeSliderValues`].
#[allow(clippy::type_complexity)]
pub fn sync_range_slider_visuals(
    sliders: Query<(Entity, &RangeSliderValues, &RangeSliderRange), Changed<RangeSliderValues>>,
    groups: Query<&Children>,
    mut fills: Query<
        &mut Node,
        (
            With<RangeSliderFill>,
            Without<RangeSliderThumbMin>,
            Without<RangeSliderThumbMax>,
        ),
    >,
    mut min_thumbs: Query<
        &mut Node,
        (
            With<RangeSliderThumbMin>,
            Without<RangeSliderFill>,
            Without<RangeSliderThumbMax>,
        ),
    >,
    mut max_thumbs: Query<
        &mut Node,
        (
            With<RangeSliderThumbMax>,
            Without<RangeSliderFill>,
            Without<RangeSliderThumbMin>,
        ),
    >,
) {
    for (entity, values, range) in &sliders {
        let (s_pct, e_pct) = range_slider_fractions(values.start, values.end, range.min, range.max);
        let fill_left = s_pct * 100.0;
        let fill_width = (e_pct - s_pct) * 100.0;
        let min_left = s_pct * 100.0;
        let max_left = e_pct * 100.0;

        if !groups.contains(entity) {
            continue;
        }

        for child in groups.iter_descendants(entity) {
            if let Ok(mut fill) = fills.get_mut(child) {
                fill.left = Val::Percent(fill_left);
                fill.width = Val::Percent(fill_width);
            }
            if let Ok(mut min_thumb) = min_thumbs.get_mut(child) {
                min_thumb.left = Val::Percent(min_left);
            }
            if let Ok(mut max_thumb) = max_thumbs.get_mut(child) {
                max_thumb.left = Val::Percent(max_left);
            }
        }
    }
}
