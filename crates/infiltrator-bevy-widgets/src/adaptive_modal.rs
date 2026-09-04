//! Dialog to ActionSheet automatic morphology transformation system.
//!
//! Compact (<600px) screens: displays as a bottom-docked ActionSheet with full width and rounded top corners.
//! Medium/Expanded/Ultra screens: displays as a centered floating Dialog card.

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::responsive::{ModalForm, ResponsiveContext};
use crate::text::{Role, TextRole};
use crate::theme::{radius, space};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, JustifyContent,
    Node, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};

/// State tracking modal open/closed and current presentation mode.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalState {
    pub is_open: bool,
    pub current_form: ModalForm,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            current_form: ModalForm::CenteredDialog,
        }
    }
}

impl ModalState {
    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }
}

/// Root container marker covering the entire viewport.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdaptiveModalRoot;

/// Dark semi-transparent backdrop scrim.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalScrim;

/// Inner card container that morphs between bottom sheet and centered dialog.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalCard;

/// Marker for modal close / dismiss buttons.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalCloseButton;

/// Event to request opening the adaptive modal.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenModal;

/// Event to request closing the adaptive modal.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseModal;

/// Scene builder for an adaptive modal dialog / action sheet.
pub fn adaptive_modal_scene(
    title: String,
    body: Box<dyn Scene>,
    actions: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let scrim_fill = Color::srgba(0.0, 0.0, 0.0, 0.55);
    let card_fill = palette.surface;
    let edge = palette.border;

    Box::new(bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
        }
        AdaptiveModalRoot
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                }
                BackgroundColor({ scrim_fill })
                ModalScrim
                Button
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    width: px(480.0),
                    max_width: percent(90),
                    padding: UiRect::all(Val::Px(space::S20)),
                    row_gap: Val::Px(space::S16),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(radius::CARD)),
                }
                BackgroundColor({ card_fill })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ModalCard
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            ( Text(title) TextRole(Role::Heading) ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                }
                                Button
                                ModalCloseButton
                                Children [
                                    ( { icon_tile_scene(IconId::Trash, 20.0, palette) } ),
                                ]
                            ),
                        ]
                    ),
                    ( { body } ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            { actions },
                        ]
                    ),
                ]
            ),
        ]
    })
}

/// Observer for ModalCloseButton and Scrim clicks: triggers CloseModal.
pub fn on_modal_close_activated(
    activate: On<Activate>,
    close_btns: Query<(), With<ModalCloseButton>>,
    scrims: Query<(), With<ModalScrim>>,
    mut commands: Commands,
) {
    if close_btns.contains(activate.entity) || scrims.contains(activate.entity) {
        commands.trigger(CloseModal);
    }
}

/// Observer for OpenModal event: updates ModalState.
pub fn on_modal_open(_trigger: On<OpenModal>, mut state: Option<ResMut<ModalState>>) {
    if let Some(ref mut state) = state {
        state.open();
    }
}

/// Observer for CloseModal event: updates ModalState.
pub fn on_modal_close(_trigger: On<CloseModal>, mut state: Option<ResMut<ModalState>>) {
    if let Some(ref mut state) = state {
        state.close();
    }
}

/// System to sync ModalRoot visibility and ModalCard morphology (ActionSheet vs Centered Dialog).
pub fn sync_adaptive_modal_morphology(
    ctx: Option<Res<ResponsiveContext>>,
    state: Option<Res<ModalState>>,
    mut roots: Query<&mut Node, (With<AdaptiveModalRoot>, Without<ModalCard>)>,
    mut cards: Query<&mut Node, (With<ModalCard>, Without<AdaptiveModalRoot>)>,
) {
    let is_open = state.as_ref().is_some_and(|s| s.is_open);
    let form = ctx
        .map(|c| c.modal_form())
        .unwrap_or(ModalForm::CenteredDialog);

    for mut root in &mut roots {
        root.display = if is_open {
            Display::Flex
        } else {
            Display::None
        };

        match form {
            ModalForm::ActionSheet => {
                root.justify_content = JustifyContent::FlexEnd;
                root.align_items = AlignItems::Stretch;
            }
            ModalForm::CenteredDialog => {
                root.justify_content = JustifyContent::Center;
                root.align_items = AlignItems::Center;
            }
        }
    }

    if is_open {
        for mut card in &mut cards {
            match form {
                ModalForm::ActionSheet => {
                    card.width = percent(100);
                    card.max_width = percent(100);
                    card.padding = UiRect::all(Val::Px(space::S16));
                    card.border_radius = BorderRadius {
                        top_left: Val::Px(radius::SHEET_TOP),
                        top_right: Val::Px(radius::SHEET_TOP),
                        bottom_left: Val::Px(0.0),
                        bottom_right: Val::Px(0.0),
                    };
                }
                ModalForm::CenteredDialog => {
                    card.width = px(480.0);
                    card.max_width = percent(90);
                    card.padding = UiRect::all(Val::Px(space::S20));
                    card.border_radius = BorderRadius::all(Val::Px(radius::CARD));
                }
            }
        }
    }
}
