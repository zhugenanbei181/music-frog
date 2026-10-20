//! Modal / Dialog: the shared modal layer, built on Bevy's official dialog
//! widgets.
//!
//! **Pure Core**: [`ModalStage`] is the typed stage of a dialog, [`ModalState`]
//! carries the product's business facts (which dialog is open, its kind and its
//! title) and [`ModalOutcome`] names the dialog resolutions. The stage is the
//! single source of truth for "is a dialog presented"; there is no pair of
//! booleans that hides a stage table.
//!
//! **Official behaviour layer**: the mounted scene carries Bevy's own
//! `ModalDialogBarrier` / `ModalDialog` / `RequestClose` components, so the
//! barrier click-outside, the Escape interception, the focus containment and
//! the dialog stacking come from `bevy_ui_widgets`. The product keeps the
//! business state and its typed transitions — it never re-implements the
//! official behaviour.

use crate::button::{ButtonVariant, button_action_scene};
use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::Message;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::{SurfaceLevel, ThemeContext};
use bevy::input_focus::InputFocusPlugin;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::scene::{Scene, bsn, on};
use bevy::ui::BorderColor;
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{
    Activate, Button, DialogPlugin, ModalDialog, ModalDialogBarrier, ModalDialogPlugin,
    RequestClose, TabPlugin,
};

/// One-off layout value (not a ladder rung): the width of the shared modal
/// dialog card. The shared ladder carries no dialog width.
const MODAL_DIALOG_WIDTH_PX: f32 = 460.0;

/// Semantic kind of a dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalKind {
    #[default]
    Info,
    Confirm,
    Warning,
    Danger,
}

/// Typed stage of a shared modal dialog.
///
/// The official widget layer models the *behaviour* of a dialog (barrier,
/// Escape, focus containment) but never the business stage, so the product
/// owns this enum and moves it through typed transitions only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalStage {
    /// No dialog is presented.
    #[default]
    Hidden,
    /// A dialog is presented and owns the interaction.
    Open,
    /// A dialog was presented and the user dismissed it — the official layer
    /// asked for the close (barrier click-outside or Escape).
    Dismissed,
}

impl ModalStage {
    /// Whether a dialog is presented at this stage.
    pub fn is_open(self) -> bool {
        matches!(self, ModalStage::Open)
    }
}

/// Dialog outcome / resolution event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalOutcome {
    Confirmed,
    Cancelled,
    Dismissed,
}

/// Business state of a modal dialog, mounted on the dialog's barrier node.
///
/// The stage is typed; the title and the kind are the product's own facts —
/// the official widget layer models neither.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ModalState {
    pub stage: ModalStage,
    pub title: String,
    pub kind: ModalKind,
}

impl ModalState {
    /// A dialog that is presented right away.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            stage: ModalStage::Open,
            title: title.into(),
            kind: ModalKind::Info,
        }
    }

    pub fn with_kind(mut self, kind: ModalKind) -> Self {
        self.kind = kind;
        self
    }

    /// Whether a dialog is presented.
    pub fn is_open(&self) -> bool {
        self.stage.is_open()
    }

    /// Present the dialog (from any stage).
    pub fn open(&mut self) {
        self.stage = ModalStage::Open;
    }

    /// The user dismissed the dialog: the official layer asked for the close.
    pub fn dismiss(&mut self) {
        self.stage = match self.stage {
            ModalStage::Open => ModalStage::Dismissed,
            stage => stage,
        };
    }

    /// The owner closed the dialog for good.
    pub fn hide(&mut self) {
        self.stage = match self.stage {
            ModalStage::Open | ModalStage::Dismissed => ModalStage::Hidden,
            ModalStage::Hidden => ModalStage::Hidden,
        };
    }
}

/// Marker component on full-screen scrim.
///
/// The scrim is the official barrier node: the marker only names the
/// product's scrim surface, the behaviour comes from `ModalDialogBarrier`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModalScrim;

/// Marker component on modal dialog panel card.
///
/// The card is the official dialog node: the marker only names the product's
/// card surface, the behaviour comes from `ModalDialog`.
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

#[derive(QueryFilter)]
pub struct ModalScrimFilter {
    with_modal_scrim: With<ModalScrim>,
    without_modal_dialog_card: Without<ModalDialogCard>,
}

#[derive(QueryFilter)]
pub struct ModalDialogCardFilter {
    with_modal_dialog_card: With<ModalDialogCard>,
    without_modal_scrim: Without<ModalScrim>,
}

/// Query filter for the shared modal roots: they carry the typed stage.
#[derive(QueryFilter)]
pub struct ModalStagedFilter {
    with_modal_state: With<ModalState>,
    without_modal_dialog_card: Without<ModalDialogCard>,
}

/// The official dialog behaviour layer.
///
/// Registers Bevy's own `Dialog` / `ModalDialog` observers — the barrier
/// click-outside, the Escape interception, the focus containment and the
/// `DialogStack` that orders the floating dialogs — plus the typed open and
/// close transitions the product sends as messages.
pub struct ModalWidgetsPlugin;

impl Plugin for ModalWidgetsPlugin {
    fn build(&self, app: &mut App) {
        // The official dialog layer needs the tab navigation (the focus
        // containment inside the dialog) and the input focus resources (the
        // focus hand-off when the dialog opens).
        if !app.is_plugin_added::<TabPlugin>() {
            app.add_plugins(TabPlugin);
        }
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<DialogPlugin>() {
            app.add_plugins(DialogPlugin);
        }
        if !app.is_plugin_added::<ModalDialogPlugin>() {
            app.add_plugins(ModalDialogPlugin);
        }
        app.add_systems(Update, (apply_modal_open, apply_modal_close).chain());
    }
}

/// Open the addressed modal through its typed transition.
pub fn apply_modal_open(
    mut opens: MessageReader<ModalOpenEvent>,
    mut states: Query<&mut ModalState>,
) {
    for open in opens.read() {
        if let Ok(mut state) = states.get_mut(open.0) {
            state.open();
        }
    }
}

/// Close the addressed modal through its typed transition.
pub fn apply_modal_close(
    mut closes: MessageReader<ModalCloseEvent>,
    mut states: Query<&mut ModalState>,
) {
    for close in closes.read() {
        if let Ok(mut state) = states.get_mut(close.0) {
            state.hide();
        }
    }
}

/// Construct a general modal dialog scene on the official dialog widgets.
///
/// The barrier node carries `ModalDialogBarrier` (click-outside and Escape)
/// and the card carries `ModalDialog` (focus containment and dialog role);
/// the product's business state rides on the barrier as a [`ModalState`].
///
/// The barrier is the modal layer's root: it declares the floating surface
/// level (`ThemeContext(SurfaceLevel::Floating)`) and mounts `FixedNode`, so
/// the overlay is positioned against the camera viewport instead of
/// inheriting a host container's layout, clipping or transform.
pub fn modal_scene(
    title: String,
    kind: ModalKind,
    content: Box<dyn Scene>,
    actions: Option<Vec<Box<dyn Scene>>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let action_scenes: Vec<Box<dyn Scene>> = actions.unwrap_or_default();
    let state = ModalState {
        stage: ModalStage::Open,
        title: title.clone(),
        kind,
    };

    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            BackgroundColor({ palette.scrim })
            ModalScrim
            ModalDialogBarrier
            state
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            on(|close: On<RequestClose>, mut states: Query<&mut ModalState>, mut outcomes: Commands, confirms: Query<(), With<ModalConfirmButton>>, cancels: Query<(), With<ModalCancelButton>>| {
                let target = close.event_target();
                // The request's source names the outcome: the confirm and the
                // cancel actions resolve the dialog, every other close (the
                // barrier click-outside, the Escape interception, the close
                // button) dismisses it.
                let source = close.original_event_target();
                let outcome = if confirms.contains(source) {
                    ModalOutcome::Confirmed
                } else if cancels.contains(source) {
                    ModalOutcome::Cancelled
                } else {
                    ModalOutcome::Dismissed
                };
                if let Ok(mut state) = states.get_mut(target) {
                    match outcome {
                        ModalOutcome::Dismissed => state.dismiss(),
                        ModalOutcome::Confirmed | ModalOutcome::Cancelled => state.hide(),
                    }
                    outcomes.write_message(ModalEvent {
                        modal: target,
                        outcome,
                    });
                }
            })
            Children [
                Node {
                    width: rem(MODAL_DIALOG_WIDTH_PX),
                    max_width: percent(90),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(rem(space::S16)),
                    row_gap: rem(space::S16),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ModalDialogCard
                ModalDialog
                Children [
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        Text(title) TextRole(Role::Heading) ModalTitle
                        --
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                        }
                        Button
                        ModalCloseButton
                        TabIndex(0)
                        on(|activate: On<Activate>, mut commands: Commands| {
                            commands.trigger(RequestClose { source: activate.entity });
                        })
                        Children [
                            @{ icon_tile_scene(IconId::Trash, 20.0, palette) }
                        ]
                    ]
                    --
                    @{ content }
                    --
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        { action_scenes }
                    ]
                ]
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
    let kind = if is_danger {
        ModalKind::Danger
    } else {
        ModalKind::Confirm
    };

    let actions = vec![
        Box::new(bsn! {
                @{ button_action_scene(cancel_text, ButtonVariant::Default, ModalCancelButton, palette) }
                // The cancel action asks the official layer to close; the
                // outcome is read back from the request's source.
                on(|activate: On<Activate>, mut commands: Commands| {
                    commands.trigger(RequestClose { source: activate.entity });
                })
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                @{ button_action_scene(confirm_text, confirm_variant, ModalConfirmButton, palette) }
                // The confirm action asks the official layer to close; the
                // outcome is read back from the request's source.
                on(|activate: On<Activate>, mut commands: Commands| {
                    commands.trigger(RequestClose { source: activate.entity });
                })
        }) as Box<dyn Scene>,
    ];

    let content = Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::vertical(rem(space::S8)),
            }
            Children [
                Text(message) TextRole(Role::Body)
            ]
    });

    modal_scene(title, kind, content, Some(actions), palette)
}

/// System to repaint the modal card and scrim from the live palette and to
/// apply the typed stage to the mounted modal.
pub fn sync_modal_visuals(
    palette: Res<UiPalette>,
    mut scrims: Query<&mut BackgroundColor, ModalScrimFilter>,
    mut cards: Query<(&mut BackgroundColor, &mut BorderColor), ModalDialogCardFilter>,
    mut staged: Query<(&ModalState, &mut Node), ModalStagedFilter>,
) {
    let scrim_bg = palette.scrim;
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

    // The typed stage is the only truth about the modal's presentation: the
    // mounted dialog is shown while it is open and hidden otherwise.
    for (state, mut node) in &mut staged {
        let display = if state.is_open() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetsPlugin;
    use crate::button::ButtonVariantStyle;
    use crate::theme::Theme;
    use bevy::MinimalPlugins;
    use bevy::app::Startup;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::system::ResMut;
    use bevy::input_focus::tab_navigation::TabGroup;
    use bevy::scene::{CommandsSceneExt, ScenePlugin};

    /// The outcome the shared modal reported to the product.
    #[derive(Resource, Default)]
    struct RecordedOutcome(Option<ModalOutcome>);

    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
        // The official dialog behaviour layer (barrier click-outside, Escape
        // interception, focus containment and the dialog stack) is added by
        // the widget kit itself, so the test must not add it twice.
        app.init_resource::<RecordedOutcome>();
        app.add_systems(
            Update,
            |mut reader: MessageReader<ModalEvent>, mut recorded: ResMut<RecordedOutcome>| {
                for event in reader.read() {
                    recorded.0 = Some(event.outcome);
                }
            },
        );
        app
    }

    fn mount_confirm_dialog(app: &mut App) {
        app.add_systems(
            Startup,
            |mut commands: Commands, palette: Res<UiPalette>| {
                commands.spawn_scene(confirm_dialog_scene(
                    "Delete Profile".to_owned(),
                    "Are you sure?".to_owned(),
                    "Delete".to_owned(),
                    "Cancel".to_owned(),
                    true,
                    &palette,
                ));
            },
        );
        app.update();
    }

    fn barrier_of(app: &mut App) -> Entity {
        let world = app.world_mut();
        let mut barriers = world.query_filtered::<Entity, With<ModalDialogBarrier>>();
        barriers
            .iter(world)
            .next()
            .expect("the official barrier is mounted")
    }

    fn close_button_of(app: &mut App) -> Entity {
        let world = app.world_mut();
        let mut buttons = world.query_filtered::<Entity, With<ModalCloseButton>>();
        buttons
            .iter(world)
            .next()
            .expect("the modal close button is mounted")
    }

    #[test]
    fn the_shared_modal_mounts_the_official_dialog_components() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);

        let barrier = barrier_of(&mut app);
        let world = app.world_mut();
        assert!(
            world.get::<ModalScrim>(barrier).is_some(),
            "the barrier is the product's scrim surface"
        );
        let state = world
            .get::<ModalState>(barrier)
            .expect("the business state rides on the barrier");
        assert_eq!(state.stage, ModalStage::Open);
        assert_eq!(state.kind, ModalKind::Danger);
        assert_eq!(state.title, "Delete Profile");

        let mut dialogs = world.query_filtered::<Entity, With<ModalDialog>>();
        let dialog = dialogs.iter(world).next().expect("the official dialog");
        assert!(
            world.get::<ModalDialogCard>(dialog).is_some(),
            "the dialog is the product's card surface"
        );
        // The official focus containment: the dialog's tab group is modal.
        let group = world
            .get::<TabGroup>(dialog)
            .expect("the official tab group");
        assert!(group.modal, "the official layer traps the focus");
    }

    #[test]
    fn the_official_request_close_dismisses_the_typed_stage() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);
        let barrier = barrier_of(&mut app);

        app.world_mut()
            .commands()
            .trigger(RequestClose { source: barrier });
        app.update();
        app.update();

        let world = app.world_mut();
        let state = world.get::<ModalState>(barrier).expect("the typed stage");
        assert_eq!(state.stage, ModalStage::Dismissed);
        assert_eq!(
            app.world().resource::<RecordedOutcome>().0,
            Some(ModalOutcome::Dismissed),
            "the product is told the user dismissed the dialog"
        );
        assert_eq!(
            app.world()
                .get::<Node>(barrier)
                .expect("the barrier")
                .display,
            Display::None,
            "a dismissed dialog is hidden"
        );
    }

    #[test]
    fn the_close_button_asks_the_official_layer_to_close() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);
        let barrier = barrier_of(&mut app);
        let close = close_button_of(&mut app);

        app.world_mut()
            .commands()
            .trigger(Activate { entity: close });
        app.update();

        let state = app
            .world()
            .get::<ModalState>(barrier)
            .expect("the typed stage");
        assert_eq!(state.stage, ModalStage::Dismissed);
    }

    #[test]
    fn the_typed_stage_moves_only_through_its_transitions() {
        let mut state = ModalState::new("Delete Profile");
        assert!(state.is_open());
        assert_eq!(state.stage, ModalStage::Open);

        state.dismiss();
        assert_eq!(state.stage, ModalStage::Dismissed);
        assert!(!state.is_open());

        state.open();
        assert_eq!(state.stage, ModalStage::Open);

        state.hide();
        assert_eq!(state.stage, ModalStage::Hidden);

        // A dismissal outside the open stage is not a transition.
        state.dismiss();
        assert_eq!(state.stage, ModalStage::Hidden);
    }

    #[test]
    fn the_shared_modal_opens_and_closes_through_its_messages() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);
        let barrier = barrier_of(&mut app);

        app.world_mut()
            .commands()
            .write_message(ModalCloseEvent(barrier));
        app.update();
        assert_eq!(
            app.world()
                .get::<ModalState>(barrier)
                .expect("the typed stage")
                .stage,
            ModalStage::Hidden
        );

        app.world_mut()
            .commands()
            .write_message(ModalOpenEvent(barrier));
        app.update();
        assert_eq!(
            app.world()
                .get::<ModalState>(barrier)
                .expect("the typed stage")
                .stage,
            ModalStage::Open
        );
    }

    #[test]
    fn the_confirm_dialog_mounts_its_cancel_and_confirm_actions() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);

        let world = app.world_mut();
        let mut variants = world.query::<&ButtonVariantStyle>();
        let mut actions = Vec::new();
        for style in variants.iter(world) {
            actions.push(style.0);
        }
        assert!(
            actions.contains(&ButtonVariant::Default),
            "the cancel action is mounted"
        );
        assert!(
            actions.contains(&ButtonVariant::Danger),
            "the danger confirm action is mounted"
        );
    }

    /// The confirm and the cancel action: their marker rides the official
    /// button's own node.
    fn action_entity(app: &mut App, confirm: bool) -> Entity {
        let world = app.world_mut();
        if confirm {
            let mut confirms = world.query_filtered::<Entity, With<ModalConfirmButton>>();
            confirms
                .iter(world)
                .next()
                .expect("the confirm action is mounted")
        } else {
            let mut cancels = world.query_filtered::<Entity, With<ModalCancelButton>>();
            cancels
                .iter(world)
                .next()
                .expect("the cancel action is mounted")
        }
    }

    #[test]
    fn the_confirm_dialog_actions_carry_their_marker_on_the_button_node() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);

        let confirm = action_entity(&mut app, true);
        let cancel = action_entity(&mut app, false);
        let world = app.world_mut();
        assert!(
            world.get::<Button>(confirm).is_some(),
            "the confirm marker rides the official button's own node"
        );
        assert!(
            world.get::<Button>(cancel).is_some(),
            "the cancel marker rides the official button's own node"
        );
    }

    #[test]
    fn the_confirm_dialog_actions_report_their_outcome() {
        let mut app = headless_app();
        mount_confirm_dialog(&mut app);

        let confirm = action_entity(&mut app, true);
        app.world_mut()
            .commands()
            .trigger(Activate { entity: confirm });
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<RecordedOutcome>().0,
            Some(ModalOutcome::Confirmed),
            "the confirm action resolves the dialog"
        );

        let cancel = action_entity(&mut app, false);
        app.world_mut()
            .commands()
            .trigger(Activate { entity: cancel });
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<RecordedOutcome>().0,
            Some(ModalOutcome::Cancelled),
            "the cancel action resolves the dialog"
        );
    }
}
