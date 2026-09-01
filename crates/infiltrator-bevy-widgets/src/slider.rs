//! Slider: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Slider`]. The official widget owns behavior — drag/track-click/keyboard
//! semantics and `ValueChange<f32>` emission — while this module owns the
//! token track, fill and thumb geometry. The official core never moves the
//! thumb itself ("that is the responsibility of the stylist"),
//! [`sync_slider_visuals`] is that stylist: it re-projects fill width and
//! thumb travel whenever [`SliderValue`] changes. The `ValueChange` →
//! [`SliderValue`] wiring stays with the caller (the official
//! `slider_self_update` observer, or an app-owned one), exactly the pill
//! pattern.
//!
//! [`slider_fraction`] is the pure projection from value to 0..=1 travel;
//! the headless surface for the whole geometry contract.

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

/// The value's travel through its range as 0..=1, clamped. Degenerate
/// ranges (end <= start) park at the midpoint, matching the official
/// `SliderRange::thumb_position`. Pure function — headless-testable.
pub fn slider_fraction(value: f32, start: f32, end: f32) -> f32 {
    SliderRange::new(start, end)
        .thumb_position(value)
        .clamp(0.0, 1.0)
}

/// The token slider chrome over the official behavior primitive. The
/// geometry is stamped for `value` at spawn; later values flow through the
/// [`SliderValue`] component and the repaint system. Interaction wiring
/// belongs to the caller.
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

/// Restamp fill width and thumb travel from the live [`SliderValue`] — the
/// stylist role the official core leaves to the skin. The fill hangs off
/// the track, so the whole subtree is searched. No color changes: the value
/// paints geometry, not ink.
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
