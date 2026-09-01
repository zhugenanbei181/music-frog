//! Check row: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Checkbox`]. The official widget owns behavior (focus, press semantics,
//! `ValueChange<bool>` emission); this module owns the token-backed box and
//! label. State stays external: the official `Checked` marker on the row is
//! the single source of truth, and the `ValueChange` → `Checked` wiring
//! belongs to the caller (the official `checkbox_self_update` observer, or
//! an app-owned one).
//!
//! [`sync_checkbox_visuals`] re-projects the box fill from `Checked` and the
//! live palette every pass (compare-and-set, so it costs nothing when
//! nothing moved) — which is also what makes a theme switch repaint boxes
//! without any switch-specific hook.

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
use bevy::ui_widgets::Checkbox;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Marker on the visual box child of a check row; the row itself carries the
/// official [`Checkbox`] and [`Checked`] state. Pure routing for the repaint
/// system, never a state store.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckboxBox;

/// The box fill for one checked state: idle boxes sit on the elevated
/// control surface; checking fills the box with the accent. Pure function —
/// headless-testable without any picking runtime.
pub fn checkbox_fill(checked: bool, palette: &UiPalette) -> Color {
    if checked {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

/// The box outline for one checked state: a hairline while idle, the accent
/// edge once checked. Pure function.
pub fn checkbox_border(checked: bool, palette: &UiPalette) -> Color {
    if checked {
        palette.accent
    } else {
        palette.border
    }
}

/// One check row: the official behavior primitive plus the token box and a
/// body label. Interaction wiring belongs to the caller. `checked` decides
/// only the spawned state; later changes flow through `Checked` itself.
pub fn checkbox_scene(label: String, checked: bool, palette: &UiPalette) -> Box<dyn Scene> {
    if checked {
        Box::new(checked_row(label, palette))
    } else {
        Box::new(unchecked_row(label, palette))
    }
}

fn checked_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = checkbox_fill(true, palette);
    let edge = checkbox_border(true, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        Checkbox
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
                CheckboxBox
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

fn unchecked_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = checkbox_fill(false, palette);
    let edge = checkbox_border(false, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        Checkbox
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
                CheckboxBox
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

/// Repaint every check box from the row's `Checked` state and the live
/// palette. Compare-and-set: a box whose fill already matches is left
/// untouched, so unchanged frames produce no change detection noise.
#[allow(clippy::type_complexity)]
pub fn sync_checkbox_visuals(
    palette: Res<UiPalette>,
    rows: Query<(&Children, Has<Checked>), With<Checkbox>>,
    mut boxes: Query<(&CheckboxBox, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (children, checked) in &rows {
        let fill = checkbox_fill(checked, &palette);
        let border = checkbox_border(checked, &palette);
        for child in children.iter() {
            if let Ok((_, mut box_fill, mut box_border)) = boxes.get_mut(*child) {
                if box_fill.0 != fill {
                    box_fill.0 = fill;
                }
                if box_border.top != border {
                    box_border.set_all(border);
                }
            }
        }
    }
}
