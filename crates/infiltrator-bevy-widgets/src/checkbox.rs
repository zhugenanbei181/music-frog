//! Check row: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Checkbox`]. The official widget owns behavior (focus, press semantics,
//! `ValueChange<bool>` emission); this module owns the token-backed box,
//! check mark / indeterminate minus dash, tri-state logic, and label.
//!
//! State stays external: the official `Checked` marker or [`Indeterminate`] /
//! [`TriStateCheckbox`] on the row is the single source of truth.
//!
//! [`sync_checkbox_visuals`] re-projects the box fill, outline and indeterminate
//! dash from `Checked`/`Indeterminate` and the live palette every pass
//! (compare-and-set).

use bevy::camera::visibility::Visibility;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{BorderColor, Checked};
use bevy::ui_widgets::Checkbox;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Tri-state checkbox value representation.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TriState {
    /// Completely unchecked.
    #[default]
    Unchecked,
    /// Checked / selected.
    Checked,
    /// Partially checked / mixed child states.
    Indeterminate,
}

impl TriState {
    /// Whether this is checked.
    pub fn is_checked(&self) -> bool {
        matches!(self, TriState::Checked)
    }

    /// Whether this is indeterminate.
    pub fn is_indeterminate(&self) -> bool {
        matches!(self, TriState::Indeterminate)
    }

    /// Whether this is unchecked.
    pub fn is_unchecked(&self) -> bool {
        matches!(self, TriState::Unchecked)
    }
}

/// Marker component for an indeterminate (semi-checked) checkbox row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Indeterminate;

/// Component wrapper for explicit tri-state checkbox tracking.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriStateCheckbox(pub TriState);

/// Marker on the visual box child of a check row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckboxBox;

/// Marker on the indeterminate horizontal dash indicator.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckboxDash;

/// The box fill for one tri-state: unchecked sits on elevated surface;
/// checked and indeterminate fill the box with the accent token.
/// Pure function — headless-testable.
pub fn tri_checkbox_fill(state: TriState, palette: &UiPalette) -> Color {
    match state {
        TriState::Unchecked => palette.surface_elevated,
        TriState::Checked | TriState::Indeterminate => palette.accent,
    }
}

/// The box outline for one tri-state: hairline while unchecked, accent edge when checked/indeterminate.
/// Pure function.
pub fn tri_checkbox_border(state: TriState, palette: &UiPalette) -> Color {
    match state {
        TriState::Unchecked => palette.border,
        TriState::Checked | TriState::Indeterminate => palette.accent,
    }
}

/// Compute the next state in tri-state cycle.
/// Pure function.
pub fn tri_checkbox_next(state: TriState, allow_indeterminate: bool) -> TriState {
    if allow_indeterminate {
        match state {
            TriState::Unchecked => TriState::Checked,
            TriState::Checked => TriState::Indeterminate,
            TriState::Indeterminate => TriState::Unchecked,
        }
    } else {
        match state {
            TriState::Unchecked => TriState::Checked,
            TriState::Checked | TriState::Indeterminate => TriState::Unchecked,
        }
    }
}

/// The box fill for standard boolean state. Pure function.
pub fn checkbox_fill(checked: bool, palette: &UiPalette) -> Color {
    if checked {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

/// The box outline for standard boolean state. Pure function.
pub fn checkbox_border(checked: bool, palette: &UiPalette) -> Color {
    if checked {
        palette.accent
    } else {
        palette.border
    }
}

/// Declarative scene constructor for a tri-state checkbox row.
pub fn tri_checkbox_scene(label: String, state: TriState, palette: &UiPalette) -> Box<dyn Scene> {
    match state {
        TriState::Checked => Box::new(checked_row(label, palette)),
        TriState::Unchecked => Box::new(unchecked_row(label, palette)),
        TriState::Indeterminate => Box::new(indeterminate_row(label, palette)),
    }
}

/// One check row: the official behavior primitive plus the token box and a body label.
pub fn checkbox_scene(label: String, checked: bool, palette: &UiPalette) -> Box<dyn Scene> {
    if checked {
        Box::new(checked_row(label, palette))
    } else {
        Box::new(unchecked_row(label, palette))
    }
}

fn checked_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = tri_checkbox_fill(TriState::Checked, palette);
    let edge = tri_checkbox_border(TriState::Checked, palette);
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
        TriStateCheckbox(TriState::Checked)
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
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ fill })
                BorderColor {
                    top: edge,
                    right: edge,
                    bottom: edge,
                    left: edge,
                }
                CheckboxBox
                Children [
                    (
                        Node {
                            width: px(palette.control_square_px * 0.55),
                            height: px(2.0),
                        }
                        BackgroundColor({ palette.on_accent })
                        Visibility::Hidden
                        CheckboxDash
                    ),
                ]
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

fn unchecked_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = tri_checkbox_fill(TriState::Unchecked, palette);
    let edge = tri_checkbox_border(TriState::Unchecked, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        Checkbox
        TriStateCheckbox(TriState::Unchecked)
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
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ fill })
                BorderColor {
                    top: edge,
                    right: edge,
                    bottom: edge,
                    left: edge,
                }
                CheckboxBox
                Children [
                    (
                        Node {
                            width: px(palette.control_square_px * 0.55),
                            height: px(2.0),
                        }
                        BackgroundColor({ palette.on_accent })
                        Visibility::Hidden
                        CheckboxDash
                    ),
                ]
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

fn indeterminate_row(label: String, palette: &UiPalette) -> impl Scene + use<> {
    let fill = tri_checkbox_fill(TriState::Indeterminate, palette);
    let edge = tri_checkbox_border(TriState::Indeterminate, palette);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        Checkbox
        Indeterminate
        TriStateCheckbox(TriState::Indeterminate)
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
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ fill })
                BorderColor {
                    top: edge,
                    right: edge,
                    bottom: edge,
                    left: edge,
                }
                CheckboxBox
                Children [
                    (
                        Node {
                            width: px(palette.control_square_px * 0.55),
                            height: px(2.0),
                        }
                        BackgroundColor({ palette.on_accent })
                        Visibility::Visible
                        CheckboxDash
                    ),
                ]
            ),
            ( Text(label) TextRole(Role::Body) ),
        ]
    }
}

/// Repaint every check box from the row's state and the live palette.
/// Compare-and-set: a box whose fill already matches is left untouched.
#[allow(clippy::type_complexity)]
pub fn sync_checkbox_visuals(
    palette: Res<UiPalette>,
    rows: Query<
        (
            &Children,
            Has<Checked>,
            Has<Indeterminate>,
            Option<&TriStateCheckbox>,
        ),
        With<Checkbox>,
    >,
    mut boxes: Query<(
        &CheckboxBox,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&Children>,
    )>,
    mut dashes: Query<(&CheckboxDash, &mut Visibility, &mut BackgroundColor), Without<CheckboxBox>>,
) {
    for (children, checked, indeterminate, tristate_opt) in &rows {
        let state =
            if indeterminate || tristate_opt == Some(&TriStateCheckbox(TriState::Indeterminate)) {
                TriState::Indeterminate
            } else if checked {
                TriState::Checked
            } else {
                TriState::Unchecked
            };

        let fill = tri_checkbox_fill(state, &palette);
        let border = tri_checkbox_border(state, &palette);
        let dash_visible = if state == TriState::Indeterminate {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let on_accent = palette.on_accent;

        for child in children.iter() {
            if let Ok((_, mut box_fill, mut box_border, box_children)) = boxes.get_mut(*child) {
                if box_fill.0 != fill {
                    box_fill.0 = fill;
                }
                if box_border.top != border {
                    box_border.set_all(border);
                }

                if let Some(inner_children) = box_children {
                    for inner in inner_children.iter() {
                        if let Ok((_, mut vis, mut dash_bg)) = dashes.get_mut(*inner) {
                            if *vis != dash_visible {
                                *vis = dash_visible;
                            }
                            if dash_bg.0 != on_accent {
                                dash_bg.0 = on_accent;
                            }
                        }
                    }
                }
            }
        }
    }
}
