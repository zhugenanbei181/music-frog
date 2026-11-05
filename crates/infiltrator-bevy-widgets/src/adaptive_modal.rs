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

use crate::feathers_theme::Token;
use crate::modal::ModalStage;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::responsive::{ModalForm, ResponsiveContext};
use crate::text::TextRole;
use crate::theme::{radius, space};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::{SurfaceLevel, ThemeBackgroundColor, ThemeContext};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::picking::events::{PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::scene::{Scene, bsn, on};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, JustifyContent,
    Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{FixedNode, UiTransform, Val2};
use bevy::ui_widgets::{Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose};
use bevy::window::PrimaryWindow;
use infiltrator_contract::design_resources;

use std::ops::Not;

/// The centered-dialog card's fixed width in baseline pixels.
const ADAPTIVE_MODAL_CARD_WIDTH_PX: f32 = 480.0;

/// The ActionSheet drag affordance's hit area, in baseline pixels.
const MODAL_SWIPE_HANDLE_WIDTH_PX: f32 = 56.0;
const MODAL_SWIPE_HANDLE_HEIGHT_PX: f32 = 20.0;
/// The visible drag indicator inside the larger touch hit area.
const MODAL_SWIPE_INDICATOR_WIDTH_PX: f32 = 32.0;
const MODAL_SWIPE_INDICATOR_HEIGHT_PX: f32 = 4.0;
/// The downward travel that resolves a drag as a dismiss gesture.
const MODAL_SWIPE_CLOSE_THRESHOLD_PX: f32 = 72.0;

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
    pub stage: ModalStage,
    /// The adaptive layer's own fact: the current presentation form.
    pub current_form: ModalForm,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            stage: ModalStage::Hidden,
            current_form: ModalForm::CenteredDialog,
        }
    }
}

impl ModalState {
    /// Present the modal.
    pub fn open(&mut self) {
        self.stage = ModalStage::Open;
    }

    /// The user dismissed the modal: the official layer asked for the close.
    pub fn dismiss(&mut self) {
        self.stage = match self.stage {
            ModalStage::Open => ModalStage::Dismissed,
            stage => stage,
        };
    }

    /// The owner closed the modal for good.
    pub fn close(&mut self) {
        self.stage = match self.stage {
            ModalStage::Open | ModalStage::Dismissed => ModalStage::Hidden,
            ModalStage::Hidden => ModalStage::Hidden,
        };
    }

    /// Whether a dialog is presented.
    pub fn is_presented(&self) -> bool {
        self.stage.is_open()
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

/// Marker on the ActionSheet's bottom-sheet drag handle.
///
/// The handle is the gesture affordance; the official `RequestClose` still
/// owns the dismissal, so a swipe cannot create a second close pathway.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalSwipeHandle;

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
                    // The centered form of this modal is a centered dialog,
                    // so it takes the contract's modal rung; the action
                    // sheet form below takes the bottom sheet rung.
                    border_radius: BorderRadius::all(rem(radius::MODAL)),
                }
                BackgroundColor({ card_fill })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ModalCard
                ModalDialog
                UiTransform
                Children [
                    Node {
                        width: rem(MODAL_SWIPE_HANDLE_WIDTH_PX),
                        height: rem(MODAL_SWIPE_HANDLE_HEIGHT_PX),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        display: Display::None,
                    }
                    ModalSwipeHandle
                    Children [
                        Node {
                            width: rem(MODAL_SWIPE_INDICATOR_WIDTH_PX),
                            height: rem(MODAL_SWIPE_INDICATOR_HEIGHT_PX),
                            border_radius: BorderRadius::MAX,
                        }
                        ThemeBackgroundColor({ Token::Border })
                    ]
                    --
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        Text(title) TextRole(design_resources::TextRole::Heading)
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
                            Text(close_label) TextRole(design_resources::TextRole::BodyStrong)
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

/// The card that owns a swipe gesture, found through the handle's parent.
fn modal_card_for_handle(
    target: Entity,
    parents: &Query<&ChildOf>,
    handles: &Query<(), With<ModalSwipeHandle>>,
    cards: &Query<(), With<ModalCard>>,
) -> Option<Entity> {
    let mut owns_handle = false;
    let mut current = Some(target);
    while let Some(entity) = current {
        if handles.contains(entity) {
            owns_handle = true;
        }
        if owns_handle && cards.contains(entity) {
            return Some(entity);
        }
        current = parents.get(entity).ok().map(|parent| parent.parent());
    }
    None
}

/// Reset the sheet's drag offset when a swipe begins.
pub fn on_modal_swipe_drag_start(
    drag_start: On<PointerDragStart>,
    mut cards: Query<&mut UiTransform, With<ModalCard>>,
    parents: Query<&ChildOf>,
    handles: Query<(), With<ModalSwipeHandle>>,
    card_markers: Query<(), With<ModalCard>>,
) {
    let Some(card) =
        modal_card_for_handle(drag_start.event_target(), &parents, &handles, &card_markers)
    else {
        return;
    };
    if let Ok(mut transform) = cards.get_mut(card)
        && transform.translation != Val2::ZERO
    {
        transform.translation = Val2::ZERO;
    }
}

/// Follow the drag downward; upward movement stays pinned to the origin.
pub fn on_modal_swipe_drag(
    drag: On<PointerDrag>,
    mut cards: Query<&mut UiTransform, With<ModalCard>>,
    parents: Query<&ChildOf>,
    handles: Query<(), With<ModalSwipeHandle>>,
    card_markers: Query<(), With<ModalCard>>,
) {
    let Some(card) = modal_card_for_handle(drag.event_target(), &parents, &handles, &card_markers)
    else {
        return;
    };
    let target_y = drag.distance.y.max(0.0);
    if let Ok(mut transform) = cards.get_mut(card)
        && transform.translation != Val2::new(Val::ZERO, rem(target_y))
    {
        transform.translation = Val2::new(Val::ZERO, rem(target_y));
    }
}

/// Resolve the drag: past the threshold the official close pathway runs.
pub fn on_modal_swipe_drag_end(
    drag_end: On<PointerDragEnd>,
    mut commands: Commands,
    mut cards: Query<&mut UiTransform, With<ModalCard>>,
    parents: Query<&ChildOf>,
    handles: Query<(), With<ModalSwipeHandle>>,
    card_markers: Query<(), With<ModalCard>>,
) {
    let Some(card) =
        modal_card_for_handle(drag_end.event_target(), &parents, &handles, &card_markers)
    else {
        return;
    };
    let should_close = drag_end.distance.y >= MODAL_SWIPE_CLOSE_THRESHOLD_PX
        && drag_end.distance.y > drag_end.distance.x.abs();
    if let Ok(mut transform) = cards.get_mut(card)
        && transform.translation != Val2::ZERO
    {
        transform.translation = Val2::ZERO;
    }
    if should_close {
        commands.trigger(RequestClose { source: card });
    }
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
/// The adaptive modal's own roots: the modal card is excluded, so the
/// morphology system and the card's own system never fight over a node.
type ModalCardMorphologyFilter = (
    With<ModalCard>,
    Without<AdaptiveModalRoot>,
    Without<ModalSwipeHandle>,
);
type ModalSwipeHandleMorphologyFilter = (
    With<ModalSwipeHandle>,
    Without<ModalCard>,
    Without<AdaptiveModalRoot>,
);

type AdaptiveModalRootFilter = (
    With<AdaptiveModalRoot>,
    Without<ModalCard>,
    Without<ModalSwipeHandle>,
);

/// The lookups the morphology system reads while it morphs the modal.
///
/// The three lookups travel together, so they are one parameter and not
/// three: a system that takes nine of them says nothing about which of them
/// belong together.
#[derive(SystemParam)]
pub struct AdaptiveModalLookups<'w, 's> {
    /// The parent of every entity in the tree.
    parents: Query<'w, 's, &'static ChildOf>,
    /// The control that closes the modal.
    closes: Query<'w, 's, Entity, With<ModalCloseButton>>,
    /// The window the modal lives in.
    windows: Query<'w, 's, Entity, With<PrimaryWindow>>,
    /// The ActionSheet drag affordance, shown only in the bottom-sheet form.
    swipe_handles: Query<'w, 's, &'static mut Node, ModalSwipeHandleMorphologyFilter>,
}

pub fn sync_adaptive_modal_morphology(
    ctx: Option<Res<ResponsiveContext>>,
    state: Option<Res<ModalState>>,
    mut roots: Query<(Entity, &mut Node), AdaptiveModalRootFilter>,
    mut cards: Query<&mut Node, ModalCardMorphologyFilter>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    mut lookups: AdaptiveModalLookups,
) {
    let is_open = state.as_ref().is_some_and(|s| s.is_presented());
    let form = ctx
        .map(|c| c.modal_form())
        .unwrap_or(ModalForm::CenteredDialog);

    let mut root_entity = None;
    for (entity, mut root) in &mut roots {
        root_entity = Some(entity);
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

    let handle_display = if is_open && form == ModalForm::ActionSheet {
        Display::Flex
    } else {
        Display::None
    };
    for mut handle in &mut lookups.swipe_handles {
        if handle.display != handle_display {
            handle.display = handle_display;
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
                        top_left: rem(radius::BOTTOM_SHEET).into(),
                        top_right: rem(radius::BOTTOM_SHEET).into(),
                        bottom_left: Val::Px(0.0).into(),
                        bottom_right: Val::Px(0.0).into(),
                    };
                }
                ModalForm::CenteredDialog => {
                    card.width = rem(ADAPTIVE_MODAL_CARD_WIDTH_PX);
                    card.max_width = percent(90);
                    card.padding = UiRect::all(rem(space::S20));
                    // The centered form is a centered dialog: the contract's
                    // modal rung, not the panel-style card rung.
                    card.border_radius = BorderRadius::all(rem(radius::MODAL));
                }
            }
        }
    }

    hand_off_focus(
        &mut commands,
        &mut focus,
        &lookups.parents,
        &lookups.closes,
        &lookups.windows,
        root_entity,
        is_open,
    );
}

/// The official focus hand-off.
///
/// The adaptive modal mounts closed, so Bevy's own `ModalDialog` spawn-time
/// focus hand-off lands on the hidden dialog and the official Escape
/// interception never reaches it. While the modal is presented the input
/// focus lives inside the dialog, so Bevy's own `ModalDialogBarrier` owns
/// the Escape interception; when the modal is no longer presented the focus
/// is released again.
///
/// The close control is the dialog's focusable control — the official
/// `TabIndex` marks it for Bevy's focus scheme — and the official
/// `AcquireFocus` request asks that scheme to move the input focus there.
fn hand_off_focus(
    commands: &mut Commands,
    focus: &mut InputFocus,
    parents: &Query<&ChildOf>,
    closes: &Query<Entity, With<ModalCloseButton>>,
    windows: &Query<Entity, With<PrimaryWindow>>,
    root: Option<Entity>,
    presented: bool,
) {
    let Some(root) = root else {
        return;
    };
    let inside = focus
        .get()
        .is_some_and(|entity| inside_dialog(parents, entity, root));
    if presented {
        if inside {
            return;
        }
        let Ok(close) = closes.single() else {
            return;
        };
        commands
            .entity(close)
            .entry::<TabIndex>()
            .or_insert(TabIndex(0));
        if let Ok(window) = windows.single() {
            commands.trigger(AcquireFocus {
                focused_entity: close,
                window,
            });
        }
        focus.set(close, FocusCause::Navigated);
    } else if inside {
        focus.clear();
    }
}

/// Whether the entity is the overlay root or lives inside it.
fn inside_dialog(parents: &Query<&ChildOf>, entity: Entity, root: Entity) -> bool {
    let mut current = Some(entity);
    while let Some(node) = current {
        if node == root {
            return true;
        }
        current = parents.get(node).ok().map(|parent| parent.parent());
    }
    false
}
