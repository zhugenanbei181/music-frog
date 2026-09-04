//! Radio rows and groups: our product skin over the official unstyled
//! `bevy_ui_widgets` [`RadioButton`] / [`RadioGroup`]. The official widgets
//! own the behavior — group-scoped keyboard navigation, mutually exclusive
//! `ValueChange<Entity>` emission on the group, `ValueChange<bool>` on the
//! button — while this module owns the token-backed ring, keyboard flow
//! state machine, index tracking, and label.
//!
//! [`sync_radio_visuals`] re-projects ring fill and outline from `Checked`
//! and the live palette every pass (compare-and-set).

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query, Res};
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

/// Marker on the visual ring child of a radio row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioRing;

/// Component on a radio button indicating its 0-based index in a group.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioButtonIndex(pub usize);

/// Group state tracking total items and active selection.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadioGroupState {
    pub active_index: Option<usize>,
    pub count: usize,
}

impl RadioGroupState {
    pub fn new(count: usize, active_index: Option<usize>) -> Self {
        Self {
            active_index,
            count,
        }
    }
}

/// Navigation actions for keyboard traversal within a radio group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioNavAction {
    /// Move to next item (ArrowDown / ArrowRight).
    Next,
    /// Move to previous item (ArrowUp / ArrowLeft).
    Previous,
    /// Move to first item (Home).
    First,
    /// Move to last item (End).
    Last,
    /// Select explicit index (e.g. number key or click).
    SelectIndex(usize),
}

/// Navigation message sent to a specific radio group entity.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadioGroupNavEvent {
    pub group: Entity,
    pub action: RadioNavAction,
}

/// Pure state machine: navigate within a radio group.
/// Returns the new selected index. Headless-testable with zero Bevy dependency.
pub fn navigate_radio_group(
    current_selected: Option<usize>,
    count: usize,
    action: RadioNavAction,
    wrap: bool,
) -> usize {
    if count == 0 {
        return 0;
    }

    match action {
        RadioNavAction::Next => match current_selected {
            Some(idx) => {
                if idx + 1 < count {
                    idx + 1
                } else if wrap {
                    0
                } else {
                    idx
                }
            }
            None => 0,
        },
        RadioNavAction::Previous => match current_selected {
            Some(idx) => {
                if idx > 0 {
                    idx - 1
                } else if wrap {
                    count - 1
                } else {
                    0
                }
            }
            None => count - 1,
        },
        RadioNavAction::First => 0,
        RadioNavAction::Last => count.saturating_sub(1),
        RadioNavAction::SelectIndex(idx) => idx.min(count.saturating_sub(1)),
    }
}

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

/// One indexed radio row with [`RadioButtonIndex`].
pub fn indexed_radio_scene(
    index: usize,
    label: String,
    selected: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let fill = radio_fill(selected, palette);
    let edge = radio_ring(selected, palette);

    if selected {
        Box::new(bsn! {
            Node {
                min_height: px(palette.control_height_px),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
                padding: UiRect::horizontal(Val::Px(space::S4)),
            }
            RadioButton
            Checked
            RadioButtonIndex(index)
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
        })
    } else {
        Box::new(bsn! {
            Node {
                min_height: px(palette.control_height_px),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
                padding: UiRect::horizontal(Val::Px(space::S4)),
            }
            RadioButton
            RadioButtonIndex(index)
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
        })
    }
}

/// The group container: the official [`RadioGroup`] behavior primitive over
/// a token-spaced column of rows. Members are already-composed radio scenes.
pub fn radio_group_scene(members: Vec<Box<dyn Scene>>) -> impl Scene + use<> {
    let count = members.len();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        RadioGroup
        RadioGroupState {
            active_index: None,
            count: count,
        }
        Children [
            { members },
        ]
    }
}

/// High-level indexed radio group scene constructor.
pub fn indexed_radio_group_scene(
    options: Vec<String>,
    selected_index: Option<usize>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let count = options.len();
    let members: Vec<Box<dyn Scene>> = options
        .into_iter()
        .enumerate()
        .map(|(idx, opt)| indexed_radio_scene(idx, opt, Some(idx) == selected_index, palette))
        .collect();

    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        RadioGroup
        RadioGroupState {
            active_index: selected_index,
            count: count,
        }
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

/// System to advance radio group keyboard navigation events.
pub fn advance_radio_group_navigation(
    mut events: MessageReader<RadioGroupNavEvent>,
    mut groups: Query<(&mut RadioGroupState, &Children), With<RadioGroup>>,
    buttons: Query<(Entity, &RadioButtonIndex, Has<Checked>), With<RadioButton>>,
    mut commands: Commands,
) {
    for event in events.read() {
        if let Ok((mut group_state, children)) = groups.get_mut(event.group) {
            let next_idx = navigate_radio_group(
                group_state.active_index,
                group_state.count,
                event.action,
                true,
            );
            group_state.active_index = Some(next_idx);

            for child in children.iter() {
                if let Ok((entity, btn_idx, is_checked)) = buttons.get(*child) {
                    if btn_idx.0 == next_idx && !is_checked {
                        commands.entity(entity).insert(Checked);
                    } else if btn_idx.0 != next_idx && is_checked {
                        commands.entity(entity).remove::<Checked>();
                    }
                }
            }
        }
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
