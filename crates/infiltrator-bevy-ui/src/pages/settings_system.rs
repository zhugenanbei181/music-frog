//! Bevy Settings control for the host system proxy.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ValueChange;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::system_proxy::{SystemProxyOwnership, SystemProxySnapshot, SystemProxyStatus};

use crate::command::{CommandSinkHandle, UiCommand};
use super::settings_core::{SettingsLine, SettingsLineKind};

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

pub(super) fn status_row(
    snapshot: &SystemProxySnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: bevy::ui::prelude::AlignItems::Center,
            justify_content: bevy::ui::prelude::JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
        }
        bevy::ui::prelude::BackgroundColor({ palette.surface_elevated })
        Children [
            ( Text({ "系统代理状态 (System Proxy Status)".to_owned() }) TextRole(Role::Body) ),
            ( Text(format_status(snapshot)) SettingsLine(SettingsLineKind::SystemProxy) TextRole(Role::Mono) ),
        ]
    })
}

pub(super) fn format_status(snapshot: &SystemProxySnapshot) -> String {
    match &snapshot.status {
        SystemProxyStatus::Unknown => "未探测".to_owned(),
        SystemProxyStatus::Disabled => "已关闭".to_owned(),
        SystemProxyStatus::Enabled => match snapshot.ownership {
            SystemProxyOwnership::Repaired => "已自动修复外部修改".to_owned(),
            SystemProxyOwnership::Owned => snapshot
                .endpoint
                .as_deref()
                .map_or_else(|| "已接管".to_owned(), |endpoint| format!("已接管 {endpoint}")),
            _ => "已启用".to_owned(),
        },
        SystemProxyStatus::Unsupported { .. } => "宿主不支持".to_owned(),
        SystemProxyStatus::Failed { failure } => format!("失败 ({})", failure.message),
    }
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
