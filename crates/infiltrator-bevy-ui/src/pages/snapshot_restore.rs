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
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::ui_widgets::Activate;
use infiltrator_application::snapshot_restore_workbench::{
    RestorePending, SnapshotRestoreWorkbench,
};
use infiltrator_bevy_widgets::button::{
    sync_button_disabled, sync_control_labels, sync_control_visuals,
};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;

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
) {
    if let Some(pending) = state.model.open(event.0.clone()) {
        state.opened_on = route.0;
        submit(&mut state, handle.as_deref(), pending);
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
    let Some(pending) = &state.model.pending else {
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
fn dismiss(
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<RestoreState>,
) {
    if state.model.visible
        && !state.model.busy()
        && (route.0 != state.opened_on
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
        && let Some(pending) = state.model.cancel()
    {
        submit(&mut state, handle.as_deref(), pending);
    }
}
