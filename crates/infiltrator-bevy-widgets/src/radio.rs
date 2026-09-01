//! Radio rows and groups: our product skin over the official unstyled
//! `bevy_ui_widgets` [`RadioButton`] / [`RadioGroup`]. The official widgets
//! own the behavior — group-scoped keyboard navigation, mutually exclusive
//! `ValueChange<Entity>` emission on the group, `ValueChange<bool>` on the
//! button — while this module owns the token-backed ring and label. State
//! stays external: the official `Checked` marker on a row is the single
//! source of truth, and the wiring from a group's `ValueChange<Entity>` back
//! to `Checked` belongs to the caller (the official `radio_self_update`
//! observer, or an app-owned one).
//!
//! [`sync_radio_visuals`] re-projects ring fill and outline from `Checked`
//! and the live palette every pass (compare-and-set) — which is also what
//! makes a theme switch repaint rings without any switch-specific hook.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{BorderColor, Checked};
use bevy::ui_widgets::{RadioButton, RadioGroup};

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Marker on the visual ring child of a radio row; the row itself carries
/// the official [`RadioButton`] and [`Checked`] state. Pure routing for the
/// repaint system, never a state store.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioRing;

/// The ring fill for one checked state: idle rings sit on the elevated
/// control surface; selecting fills the disc with the accent. Pure function —
/// headless-testable without any picking runtime.
pub fn radio_fill(selected: bool, palette: &UiPalette) -> Color {
    if selected {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

/// The ring outline for one checked state: a hairline while idle, the accent
/// edge once selected. Pure function.
pub fn radio_ring(selected: bool, palette: &UiPalette) -> Color {
    if selected {
        palette.accent
    } else {
        palette.border
    }
}

/// One radio row: the official behavior primitive plus the token ring and a
/// body label. `selected` decides only the spawned state; later changes
/// flow through `Checked` itself.
pub fn radio_scene(label: String, selected: bool, palette: &UiPalette) -> Box<dyn Scene> {
    if selected {
        Box::new(selected_row(label, palette))
    } else {
        Box::new(idle_row(label, palette))
    }
}

/// The group container: the official [`RadioGroup`] behavior primitive over
/// a token-spaced column of rows. Members are already-composed radio (or
/// other) scenes — the module never creates children imperatively.
pub fn radio_group_scene(members: Vec<Box<dyn Scene>>) -> impl Scene + use<> {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        RadioGroup
        Children [
            { members },
        ]
    }
}

fn selected_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = radio_fill(true, palette);
    let edge = radio_ring(true, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        RadioButton
        Checked
        Children [
            (
                Node {
                    width: px(palette.control_square_px),
                    height: px(palette.control_square_px),
                    flex_shrink: 0.0,
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_square_px * 0.5,
                    )),
                }
                BackgroundColor({ fill })
                BorderColor {
                    top: edge,
                    right: edge,
                    bottom: edge,
                    left: edge,
                }
                RadioRing
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

fn idle_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = radio_fill(false, palette);
    let edge = radio_ring(false, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        RadioButton
        Children [
            (
                Node {
                    width: px(palette.control_square_px),
                    height: px(palette.control_square_px),
                    flex_shrink: 0.0,
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_square_px * 0.5,
                    )),
                }
                BackgroundColor({ fill })
                BorderColor {
                    top: edge,
                    right: edge,
                    bottom: edge,
                    left: edge,
                }
                RadioRing
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

/// Repaint every radio ring from the row's `Checked` state and the live
/// palette. Compare-and-set: a ring whose fill already matches is left
/// untouched, so unchanged frames produce no change detection noise.
#[allow(clippy::type_complexity)]
pub fn sync_radio_visuals(
    palette: Res<UiPalette>,
    rows: Query<(&Children, Has<Checked>), With<RadioButton>>,
    mut rings: Query<(&RadioRing, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (children, selected) in &rows {
        let fill = radio_fill(selected, &palette);
        let ring = radio_ring(selected, &palette);
        for child in children.iter() {
            if let Ok((_, mut ring_fill, mut ring_border)) = rings.get_mut(*child) {
                if ring_fill.0 != fill {
                    ring_fill.0 = fill;
                }
                if ring_border.top != ring {
                    ring_border.set_all(ring);
                }
            }
        }
    }
}
