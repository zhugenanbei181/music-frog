//! Native clipboard writes are acknowledged only after the platform succeeds.
use crate::pages::connections::LastConnectionsProjection;
use crate::pages::connections_drawer::ConnectionsDrawerState;
use crate::toast::ShellToast;
use bevy::clipboard::Clipboard;
use bevy::ecs::component::Component;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
use std::env;

/// Explicit host composition, separate from the SDK editor's clipboard resource.
/// Presence permits an OS operation; only its actual result acknowledges success.
#[derive(Resource)]
pub struct ClipboardHost;

#[derive(Component, Default, Clone, Copy)]
pub struct CopyConnectionHostButton;

pub(crate) fn copy_connection_host(
    activation: On<Activate>,
    buttons: Query<(), With<CopyConnectionHostButton>>,
    state: Res<ConnectionsDrawerState>,
    projection: Res<LastConnectionsProjection>,
    clipboard: Option<ResMut<Clipboard>>,
    host: Option<Res<ClipboardHost>>,
    mut toasts: MessageWriter<ShellToast>,
) {
    if !buttons.contains(activation.entity) || !state.open {
        return;
    }
    let Some(item) = state.selected_id.as_ref().and_then(|id| {
        projection
            .0
            .as_ref()
            .and_then(|projection| projection.connections.iter().find(|item| &item.id == id))
    }) else {
        return;
    };
    let language = env::var("INFILTRATOR_LANG").unwrap_or_else(|_| get_system_language());
    let lang = Lang(&language);
    if host.is_none()
        || cfg!(any(target_os = "android", target_os = "ios"))
        || item.destination_host.is_empty()
    {
        toasts.write(ShellToast::warning(
            lang.tr("clipboard_unsupported").into_owned(),
        ));
        return;
    }
    let Some(mut clipboard) = clipboard else {
        toasts.write(ShellToast::warning(
            lang.tr("clipboard_unsupported").into_owned(),
        ));
        return;
    };
    match clipboard.set_text(item.destination_host.clone()) {
        Ok(()) => {
            toasts.write(ShellToast::info(lang.tr("clipboard_copied").into_owned()));
        }
        Err(error) => {
            toasts.write(ShellToast::danger(format!(
                "{}: {error}",
                lang.tr("clipboard_failed")
            )));
        }
    }
}
