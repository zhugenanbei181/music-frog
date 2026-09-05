//! Bevy Settings control for the host system proxy.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, UiRect, Val, percent};
use bevy::ui_widgets::ValueChange;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};

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
            ( { checkbox_scene("设置系统代理 (Set System Proxy)".to_owned(), checked, palette) } ),
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
        handle.submit(UiCommand::SetSystemProxy { enabled: change.value });
    }
}
