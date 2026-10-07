//! Typed native actions correlate the exact Hosts patch and request before publishing completion.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::dns_hosts::{DnsHostsEditorField, DnsHostsEditorState, HostAction};
use crate::pages::dns_hosts_sync::HostFieldWrappers;
use crate::route::{ActiveRoute, Route};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::Has;
use bevy::ecs::system::SystemParam;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_contract::error::{ErrorCode, Failure};
#[derive(SystemParam)]
pub struct HostNativeInputs<'w, 's> {
    wrappers: Query<'w, 's, (&'static Children, Has<DnsHostsEditorField>), HostFieldWrappers>,
    fields: Query<'w, 's, &'static TextField>,
}
pub fn activate(
    event: On<Activate>,
    actions: Query<&HostAction>,
    route: Res<ActiveRoute>,
    sink: Option<Res<CommandSinkHandle>>,
    mut state: ResMut<DnsHostsEditorState>,
    inputs: HostNativeInputs,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    if route.0 != Some(Route::Dns) || state.editor.pending.is_some() {
        return;
    }
    if matches!(action, HostAction::Open) {
        state.editor.show();
        state.sync_fields = true;
        return;
    }
    if !state.editor.open {
        return;
    }
    if matches!(
        action,
        HostAction::CommitRow | HostAction::Apply | HostAction::ImportLegacy | HostAction::Edit(_)
    ) {
        for (children, address) in &inputs.wrappers {
            if let Some(value) = children.iter().find_map(|entity| {
                inputs
                    .fields
                    .get(*entity)
                    .ok()
                    .map(|field| field.0.text().to_owned())
            }) {
                if address {
                    state.editor.edit_address(value);
                } else {
                    state.editor.edit_domain(value);
                }
            }
        }
    }
    match action {
        HostAction::Open => {}
        HostAction::Cancel => {
            state.editor.cancel();
            state.sync_fields = true;
        }
        HostAction::CancelRow => {
            state.editor.cancel_row();
            state.sync_fields = true;
        }
        HostAction::CommitRow => {
            if state.editor.commit_row() {
                state.sync_fields = true;
            }
        }
        HostAction::Edit(id) => {
            state.editor.select(*id);
            state.sync_fields = true;
        }
        HostAction::Remove(id) => {
            state.editor.remove(*id);
            state.sync_fields = true;
        }
        HostAction::ImportLegacy => {
            state.editor.import_legacy();
            state.sync_fields = true;
        }
        HostAction::Apply => {
            let pending = match state.editor.begin() {
                Ok(pending) => pending,
                Err(_) => return,
            };
            if let Some(request) = sink.and_then(|sink| {
                sink.submit_tracked(UiCommand::ApplyDnsSettings {
                    patch: pending.patch,
                })
            }) {
                state.request = Some((request, pending.token));
            } else {
                state.editor.finish(
                    pending.token,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "Hosts command service has no terminal response",
                        true,
                    )),
                );
            }
        }
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<DnsHostsEditorState>) {
    let Some((request, token)) = state.request else {
        return;
    };
    let UiCommand::ApplyDnsSettings { patch } = &event.command else {
        return;
    };
    if request != event.request_id
        || !state
            .editor
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token && &pending.patch == patch)
    {
        return;
    }
    if state.editor.finish(token, event.unit_result()) {
        state.request = None;
        state.sync_fields = true;
    }
}
