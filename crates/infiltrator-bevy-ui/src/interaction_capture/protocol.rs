//! A pure codec host for capture; the native import observer still sends the real intent.
use super::InteractionCapture;
use super::host::{CaptureCapability, CaptureCommandSink};
use crate::command::CommandSinkHandle;
use crate::pages::business_panel::{BusinessPanelState, OpenBusinessPanel, PanelKind};
use crate::pages::proxies_custom::{CustomNodeUriField, ImportUriButton};
use crate::pages::proxies_form::CustomNodeForm;
use crate::route::surface_projection::replay_surface_snapshot;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::protocol_codec_application;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::parity::FeatureId;
use lurker_core_contract::page::{PageData, SurfaceOrigin};
use std::sync::Arc;

pub const URI: &str =
    "vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node";

/// The name the shared codec gives the decoded share link.
const IMPORTED_NODE_NAME: &str = "Imported-Node";

/// The capture's own progress: the share link is imported exactly once.
#[derive(Resource, Default)]
pub struct ProtocolCapture {
    imported: bool,
}

pub fn install(app: &mut App) {
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::default(),
        CaptureCapability::ImportUri,
    ))));
    app.init_resource::<ProtocolCapture>();
}

/// Two native input steps: open the actual modal, then activate its actual import control.
#[derive(SystemParam)]
pub struct ProtocolControls<'w, 's> {
    panel: Res<'w, BusinessPanelState>,
    form: Res<'w, CustomNodeForm>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, ProtocolCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    open: Query<'w, 's, (Entity, &'static OpenBusinessPanel)>,
    wrappers: Query<'w, 's, &'static Children, With<CustomNodeUriField>>,
    fields: Query<'w, 's, &'static mut TextField>,
    import: Query<'w, 's, Entity, With<ImportUriButton>>,
}
pub fn activate(
    mut controls: ProtocolControls,
    mut state: ResMut<InteractionCapture>,
    mut commands: Commands,
) {
    // The studio is republished every frame until the imported draft is in
    // it: the import creates that draft, so a publish before the import
    // would carry an empty studio.
    publish_imported_studio(&mut controls, &mut commands);
    if state.activated || controls.route.0 != Some(Route::Proxies) {
        return;
    }
    if controls.capture.imported {
        // The share link is imported exactly once: the editor's own readback
        // fence owns the rest, and a second import would retire the pending
        // observation the first one opened.
        return;
    }
    if controls.panel.0 != Some(PanelKind::CustomNode) {
        if let Some((entity, _)) = controls
            .open
            .iter()
            .find(|(_, open)| open.0 == PanelKind::CustomNode)
        {
            commands.trigger(Activate { entity });
            state.activated = state.feature == FeatureId::ProxiesCustomNodeModal;
        }
        return;
    }
    if controls.form.reset_fields || controls.form.operation.is_busy() {
        return;
    }
    let Some(children) = controls.wrappers.iter().next() else {
        return;
    };
    let Some(entity) = children
        .iter()
        .find(|entity| controls.fields.contains(*entity))
    else {
        return;
    };
    let Ok(mut field) = controls.fields.get_mut(entity) else {
        return;
    };
    field.0.apply(TextFieldInput::SetText(URI.into()));
    if let Some(entity) = controls.import.iter().next() {
        commands.trigger(Activate { entity });
        controls.capture.imported = true;
        state.activated = true;
    }
}

/// Republish the decoded share link as the surface's protocol studio.
///
/// The custom-node editor only accepts an imported draft when the owner
/// snapshot carries it: the editor's readback fence compares the owner's
/// studio against the draft it decoded, and it only accepts a snapshot that
/// is NEWER than the one the import started from.
///
/// The owner's terminal acknowledgment and the owner's snapshot are two
/// separate facts, and their order is not the capture's to decide: the
/// acknowledgment can land after the first publication, and then that
/// publication can never be accepted. The capture therefore keeps
/// republishing the decoded studio — each time with a newer revision — until
/// the editor really holds the decoded draft, and it stops the moment the
/// readback lands.
fn publish_imported_studio(controls: &mut ProtocolControls, commands: &mut Commands) {
    let Some(studio) = protocol_codec_application::studio_snapshot() else {
        return;
    };
    if !studio
        .draft
        .as_ref()
        .is_some_and(|draft| draft.name == IMPORTED_NODE_NAME)
    {
        return;
    }
    if controls
        .form
        .studio
        .draft
        .as_ref()
        .is_some_and(|draft| draft.name == IMPORTED_NODE_NAME)
    {
        // The editor's readback landed: the surface owns the decoded draft.
        return;
    }
    if controls.form.studio.last_error.is_some() {
        // The import failed, so no publication can ever be accepted.
        return;
    }
    let mut snapshot = controls.latest.0.clone();
    snapshot.protocol_studio = PageData::ready(studio);
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.revision += 1;
    controls.latest.0 = snapshot.clone();
    replay_surface_snapshot(snapshot, commands);
}
