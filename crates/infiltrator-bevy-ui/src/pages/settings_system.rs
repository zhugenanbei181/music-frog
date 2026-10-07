//! Bevy Settings control for the host system proxy.

use super::settings_core::{SettingsLine, SettingsLineKind};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BackgroundColor, JustifyContent, Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ValueChange;
use infiltrator_application::network_status_projection::system_proxy_status;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};

/// Parent marker for the host system proxy checkbox.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemProxyToggle;

pub(super) fn toggle_scene(checked: bool, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::horizontal(Val::Px(space::S4)),
            }
            SystemProxyToggle
            Children [
                @{ localized_checkbox_scene(LocalizedText::plain("settings_set_system_proxy"), checked, palette) }
            ]
    })
}

pub(super) fn status_row(
    snapshot: &SystemProxySnapshot,
    recovery: &SystemProxyRecoverySnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(space::S8)),
            }
            BackgroundColor({ palette.surface_elevated })
            Children [
                LocalizedText::plain("settings_system_proxy_status_title") TextRole(Role::Body)
                --
                Text({ system_proxy_status(snapshot, recovery, "zh-CN") }) SettingsLine(SettingsLineKind::SystemProxy) TextRole(Role::Mono)
            ]
    })
}

pub(super) fn on_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<SystemProxyToggle>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    if toggles.get(parent.0).is_ok() {
        handle.submit(UiCommand::SetSystemProxy {
            enabled: change.value,
        });
    }
}
