//! A pure codec host for capture; the native import observer still sends the real intent.
use super::InteractionCapture;
use super::host::{CaptureCapability, CaptureCommandSink};
use crate::command::CommandSinkHandle;
use crate::pages::business_panel::{BusinessPanelState, OpenBusinessPanel, PanelKind};
use crate::pages::proxies_custom::{CustomNodeUriField, ImportUriButton};
use crate::pages::proxies_form::CustomNodeForm;
use crate::route::{ActiveRoute, Route};
use bevy::app::App;
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::parity::FeatureId;
use std::sync::Arc;

pub const URI: &str =
    "vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node";

pub fn install(app: &mut App) {
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::default(),
        CaptureCapability::ImportUri,
    ))));
}

/// Two native input steps: open the actual modal, then activate its actual import control.
#[derive(SystemParam)]
pub struct ProtocolControls<'w, 's> {
    panel: Res<'w, BusinessPanelState>,
    form: Res<'w, CustomNodeForm>,
    route: Res<'w, ActiveRoute>,
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
    if state.activated || controls.route.0 != Some(Route::Proxies) {
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
    if controls.form.reset_fields || controls.form.importing.is_some() {
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
        state.activated = true;
    }
}
