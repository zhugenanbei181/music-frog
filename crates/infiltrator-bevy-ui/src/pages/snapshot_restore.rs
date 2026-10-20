//! Native restoration requests use the shared transaction and correlated command results.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::snapshot_restore_scene::{RestoreControl, spawn_review};
use crate::pages::snapshot_restore_view::replay;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, Plugin, PostUpdate, Startup};
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::ui_widgets::Activate;
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::button::{
    sync_button_disabled, sync_control_labels, sync_control_visuals,
};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_presentation::snapshot_restore_workbench::{
    RestorePending, SnapshotRestoreWorkbench,
};
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Event)]
pub struct OpenSnapshotRestore(pub SnapshotRestoreTarget);
#[derive(Resource, Default)]
pub struct RestoreState {
    pub model: SnapshotRestoreWorkbench,
    pub request: Option<(RequestId, u64)>,
    pub opened_on: Option<Route>,
    pub blocked: Vec<Entity>,
    pub previous_focus: Option<Entity>,
}
pub struct SnapshotRestorePlugin;
impl Plugin for SnapshotRestorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RestoreState>()
            .add_observer(open)
            .add_observer(action)
            .add_observer(completed)
            .add_systems(Startup, spawn_review)
            .add_systems(
                PostUpdate,
                (dismiss, replay, ApplyDeferred)
                    .chain()
                    .before(sync_button_disabled)
                    .before(sync_control_visuals)
                    .before(sync_control_labels),
            );
    }
}
fn submit(state: &mut RestoreState, handle: Option<&CommandSinkHandle>, pending: RestorePending) {
    if let Some(handle) = handle
        && let Some(id) = handle.submit_tracked(UiCommand::SnapshotRestore {
            intent: pending.intent.clone(),
        })
    {
        state.request = Some((id, pending.operation));
    } else {
        state.model.finish(
            pending.operation,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Snapshot command service is unavailable",
                true,
            )),
        );
    }
}
fn open(
    event: On<OpenSnapshotRestore>,
    route: Res<ActiveRoute>,
    handle: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<RestoreState>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    controls: Query<(Entity, &RestoreControl), Without<ModalScrim>>,
    windows: Query<Entity, With<PrimaryWindow>>,
) {
    if let Some(pending) = state.model.open(event.0.clone()) {
        state.opened_on = route.0;
        submit(&mut state, handle.as_deref(), pending);
        // The review is presented: the official focus hand-off moves the
        // input focus into the dialog, so Bevy's own `ModalDialogBarrier`
        // owns the Escape interception.
        if let Some(control) = dismiss_control(&controls) {
            hand_off_focus(&mut commands, &mut focus, &windows, control);
        }
    }
}
fn action(
    event: On<Activate>,
    controls: Query<&RestoreControl>,
    handle: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<RestoreState>,
) {
    let Ok(control) = controls.get(event.entity) else {
        return;
    };
    let pending = match control {
        RestoreControl::Confirm => state.model.confirm(),
        RestoreControl::Cancel => state.model.cancel(),
        RestoreControl::Retry => state.model.retry(),
    };
    if let Some(pending) = pending {
        submit(&mut state, handle.as_deref(), pending);
    }
}
fn completed(event: On<CommandExecutedEvent>, mut state: ResMut<RestoreState>) {
    let Some((id, operation)) = state.request else {
        return;
    };
    let Some(pending) = state.model.pending() else {
        return;
    };
    if id != event.request_id
        || event.command
            != (UiCommand::SnapshotRestore {
                intent: pending.intent.clone(),
            })
    {
        return;
    }
    state.request = None;
    state.model.finish(operation, event.result.clone());
}
/// The review's dismiss control: the one cancel control that is not the
/// scrim.
fn dismiss_control(
    controls: &Query<(Entity, &RestoreControl), Without<ModalScrim>>,
) -> Option<Entity> {
    controls
        .iter()
        .find(|(_, control)| matches!(control, RestoreControl::Cancel))
        .map(|(entity, _)| entity)
}

/// The official focus hand-off.
///
/// The dialog's dismiss control is the dialog's focusable control — the
/// official `TabIndex` marks it for Bevy's focus scheme — and the official
/// `AcquireFocus` request asks that scheme to move the input focus into the
/// dialog. The official focus resource carries the result, so the page and
/// the official layer read the same fact. While the focus lives inside the
/// barrier, Bevy's own `ModalDialogBarrier` owns the Escape interception.
fn hand_off_focus(
    commands: &mut Commands,
    focus: &mut InputFocus,
    windows: &Query<Entity, With<PrimaryWindow>>,
    control: Entity,
) {
    commands
        .entity(control)
        .entry::<TabIndex>()
        .or_insert(TabIndex(0));
    if let Ok(window) = windows.single() {
        commands.trigger(AcquireFocus {
            focused_entity: control,
            window,
        });
    }
    focus.set(control, FocusCause::Navigated);
}

fn dismiss(
    route: Res<ActiveRoute>,
    handle: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<RestoreState>,
    mut focus: ResMut<InputFocus>,
    controls: Query<(Entity, &RestoreControl), Without<ModalScrim>>,
) {
    // The Escape interception belongs to Bevy's official dialog layer: the
    // barrier reports the close request and the review's own close path runs
    // from it. This system only owns the page navigation.
    if state.model.is_visible()
        && !state.model.is_busy()
        && route.0 != state.opened_on
        && let Some(pending) = state.model.cancel()
    {
        submit(&mut state, handle.as_deref(), pending);
    }
    // The review is closed: the focus leaves the dialog again.
    if !state.model.is_visible()
        && let Some(control) = dismiss_control(&controls)
        && focus.get() == Some(control)
    {
        focus.clear();
    }
}
