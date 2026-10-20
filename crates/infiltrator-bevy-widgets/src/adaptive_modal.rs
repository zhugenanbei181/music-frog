//! Dialog to ActionSheet automatic morphology transformation system.
//!
//! Compact (<600px) screens: displays as a bottom-docked ActionSheet with full width and rounded top corners.
//! Medium/Expanded/Ultra screens: displays as a centered floating Dialog card.
//!
//! The adaptive modal rides Bevy's official dialog layer: the overlay root is
//! the official `ModalDialogBarrier` (the click-outside close and the Escape
//! interception) and the card is the official `ModalDialog` (the focus
//! containment and the dialog role). The official layer only *reports* the
//! close request — the product owns the typed stage and moves it through
//! typed transitions.

use crate::modal::ModalStage;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::responsive::{ModalForm, ResponsiveContext};
use crate::text::{Role, TextRole};
use crate::theme::{radius, space};
use bevy::ecs::component::Component;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::{SurfaceLevel, ThemeContext};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::scene::{Scene, bsn, on};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, JustifyContent,
    Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose};
use std::ops::Not;

/// The centered-dialog card's fixed width in baseline pixels.
const ADAPTIVE_MODAL_CARD_WIDTH_PX: f32 = 480.0;

/// State tracking the adaptive modal's typed stage and its presentation form.
///
/// The stage is the shared typed stage: it is the single truth about whether
/// a dialog is presented, and it moves through typed transitions only. The
/// adaptive layer's own fact — the presentation form the current breakpoint
/// selects — rides beside it.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalState {
    /// The shared typed stage of the adaptive modal.
    ///
    /// The field keeps the name the product's readers already use; the value
    /// is the typed stage itself, never a flag: `!stage` answers "no dialog
    /// is presented".
    pub is_open: ModalStage,
    /// The adaptive layer's own fact: the current presentation form.
    pub current_form: ModalForm,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            is_open: ModalStage::Hidden,
            current_form: ModalForm::CenteredDialog,
        }
    }
}

impl ModalState {
    /// Present the modal.
    pub fn open(&mut self) {
        self.is_open = ModalStage::Open;
    }

    /// The user dismissed the modal: the official layer asked for the close.
    pub fn dismiss(&mut self) {
        self.is_open = match self.is_open {
            ModalStage::Open => ModalStage::Dismissed,
            stage => stage,
        };
    }

    /// The owner closed the modal for good.
    pub fn close(&mut self) {
        self.is_open = match self.is_open {
            ModalStage::Open | ModalStage::Dismissed => ModalStage::Hidden,
            ModalStage::Hidden => ModalStage::Hidden,
        };
    }

    /// Whether a dialog is presented.
    pub fn is_presented(&self) -> bool {
        self.is_open.is_open()
    }
}

/// The shared stage's negated reading: a stage that presents no dialog.
///
/// The product's readers ask "is a dialog presented?" by negating the stage,
/// so the shared stage owns that reading and every consumer of it speaks one
/// vocabulary.
impl Not for ModalStage {
    type Output = bool;

    fn not(self) -> bool {
        matches!(self, ModalStage::Hidden | ModalStage::Dismissed)
    }
}

/// Root container marker covering the entire viewport.
///
/// The root is the official barrier node: the marker names the product's
/// overlay surface, the behaviour comes from `ModalDialogBarrier`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdaptiveModalRoot;

/// Dark semi-transparent backdrop scrim.
///
/// The scrim is the official barrier: the marker names the product's backdrop
/// surface, the behaviour comes from `ModalDialogBarrier`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalScrim;

/// Inner card container that morphs between bottom sheet and centered dialog.
///
/// The card is the official dialog node: the marker names the product's card
/// surface, the behaviour comes from `ModalDialog`.
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
///
/// The overlay root is the official barrier: it covers the screen behind the
/// card, owns the click-outside close and the Escape interception. The card
/// is the official dialog: it owns the focus containment. The official layer
/// only reports the close request — the product's typed stage moves on that
/// report.
pub fn adaptive_modal_scene(
    title: String,
    close_label: String,
    body: Box<dyn Scene>,
    actions: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let scrim_fill = palette.scrim;
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
            BackgroundColor({ scrim_fill })
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            AdaptiveModalRoot
            ModalScrim
            ModalDialogBarrier
            Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    width: rem(ADAPTIVE_MODAL_CARD_WIDTH_PX),
                    max_width: percent(90),
                    max_height: percent(92),
                    min_height: px(0.0),
                    overflow: Overflow::clip(),
                    padding: UiRect::all(rem(space::S20)),
                    row_gap: rem(space::S16),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(rem(radius::CARD)),
                }
                BackgroundColor({ card_fill })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ModalCard
                ModalDialog
                Children [
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        Text(title) TextRole(Role::Heading)
                        --
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                        }
                        Button
                        ModalCloseButton
                        TabIndex(0)
                        // The close button asks the official layer to close;
                        // the product never closes the modal on its own.
                        on(|activate: On<Activate>, mut commands: Commands| {
                            commands.trigger(RequestClose { source: activate.entity });
                        })
                        Children [
                            Text(close_label) TextRole(Role::BodyStrong)
                        ]
                    ]
                    --
                    @{ body }
                    --
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        { actions }
                    ]
                ]
            ]
    })
}

/// The official close request reaches the adaptive layer's own close path.
///
/// The official layer reports the dismissal (the barrier click-outside, the
/// Escape interception, the close button); the adaptive layer moves its typed
/// stage on that report and never keeps a second "is open" fact.
pub fn on_adaptive_modal_request_close(
    close: On<RequestClose>,
    barriers: Query<(), With<AdaptiveModalRoot>>,
    mut state: Option<ResMut<ModalState>>,
) {
    if !barriers.contains(close.event_target()) {
        return;
    }
    if let Some(ref mut state) = state {
        state.dismiss();
    }
}

/// Observer for OpenModal event: presents the adaptive modal.
pub fn on_modal_open(_trigger: On<OpenModal>, mut state: Option<ResMut<ModalState>>) {
    if let Some(ref mut state) = state {
        state.open();
    }
}

/// Observer for CloseModal event: the owner closes the adaptive modal.
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
    let is_open = state.as_ref().is_some_and(|s| s.is_presented());
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
                    card.padding = UiRect::all(rem(space::S16));
                    card.border_radius = BorderRadius {
                        top_left: rem(radius::SHEET_TOP).into(),
                        top_right: rem(radius::SHEET_TOP).into(),
                        bottom_left: Val::Px(0.0).into(),
                        bottom_right: Val::Px(0.0).into(),
                    };
                }
                ModalForm::CenteredDialog => {
                    card.width = rem(ADAPTIVE_MODAL_CARD_WIDTH_PX);
                    card.max_width = percent(90);
                    card.padding = UiRect::all(rem(space::S20));
                    card.border_radius = BorderRadius::all(rem(radius::CARD));
                }
            }
        }
    }
}
