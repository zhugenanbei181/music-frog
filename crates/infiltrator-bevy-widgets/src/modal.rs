//! Modal / Dialog: token-styled floating dialog cards over a dark viewport scrim.
//!
//! **Pure Core**: [`ModalState`], [`ModalKind`], and [`ModalOutcome`] define the
//! modal dialog contracts, supporting Informational, Confirm, Warning, and Danger workflows.
//!
//! **Scene Adapters**: [`modal_scene`] and [`confirm_dialog_scene`] build declarative
//! centered overlay panels on token surface and border layers.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::Message;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::button::{ButtonVariant, button_scene};
use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Semantic kind of a dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalKind {
    #[default]
    Info,
    Confirm,
    Warning,
    Danger,
}

/// Dialog outcome / resolution event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalOutcome {
    Confirmed,
    Cancelled,
    Dismissed,
}

/// Pure state of a modal dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModalState {
    pub is_open: bool,
    pub title: String,
    pub dismiss_on_scrim: bool,
    pub width_px: f32,
    pub kind: ModalKind,
}

impl ModalState {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            is_open: true,
            title: title.into(),
            dismiss_on_scrim: true,
            width_px: 440.0,
            kind: ModalKind::Info,
        }
    }

    pub fn with_kind(mut self, kind: ModalKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_width(mut self, width_px: f32) -> Self {
        self.width_px = width_px;
        self
    }
}

/// Marker component on full-screen scrim.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalScrim;

/// Marker component on modal dialog panel card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalDialogCard;

/// Marker component on modal title text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalTitle;

/// Marker component on modal close button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalCloseButton;

/// Marker component on modal confirm action button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalConfirmButton;

/// Marker component on modal cancel action button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalCancelButton;

/// Message dispatched when modal outcome is chosen.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalEvent {
    pub modal: Entity,
    pub outcome: ModalOutcome,
}

/// Message to open a modal dialog.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalOpenEvent(pub Entity);

/// Message to close a modal dialog.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalCloseEvent(pub Entity);

/// Construct a general modal dialog scene.
pub fn modal_scene(
    title: String,
    content: Box<dyn Scene>,
    actions: Option<Vec<Box<dyn Scene>>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let action_scenes: Vec<Box<dyn Scene>> = actions.unwrap_or_default();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({ palette.scrim() })
        ModalScrim
        Children [
            (
                Node {
                    width: px(460.0),
                    max_width: percent(90),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S16)),
                    row_gap: Val::Px(space::S16),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ModalDialogCard
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            ( Text(title) TextRole(Role::Heading) ModalTitle ),
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
                    ( { content } ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            { action_scenes },
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// Construct a standard confirm / alert dialog scene.
pub fn confirm_dialog_scene(
    title: String,
    message: String,
    confirm_text: String,
    cancel_text: String,
    is_danger: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let confirm_variant = if is_danger {
        ButtonVariant::Danger
    } else {
        ButtonVariant::Primary
    };

    let actions = vec![
        Box::new(button_scene(cancel_text, ButtonVariant::Default, palette)) as Box<dyn Scene>,
        Box::new(button_scene(confirm_text, confirm_variant, palette)) as Box<dyn Scene>,
    ];

    let content = Box::new(bsn! {
        Node {
            width: percent(100),
            padding: UiRect::vertical(Val::Px(space::S8)),
        }
        Children [
            ( Text(message) TextRole(Role::Body) ),
        ]
    });

    modal_scene(title, content, Some(actions), palette)
}

/// System to repaint modal card and scrim from live palette.
#[allow(clippy::type_complexity)]
pub fn sync_modal_visuals(
    palette: Res<UiPalette>,
    mut scrims: Query<&mut BackgroundColor, (With<ModalScrim>, Without<ModalDialogCard>)>,
    mut cards: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<ModalDialogCard>, Without<ModalScrim>),
    >,
) {
    let scrim_bg = palette.scrim();
    for mut fill in &mut scrims {
        if fill.0 != scrim_bg {
            fill.0 = scrim_bg;
        }
    }

    let edge = palette.border;
    for (mut fill, mut border) in &mut cards {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
        if border.top != edge {
            border.set_all(edge);
        }
    }
}
