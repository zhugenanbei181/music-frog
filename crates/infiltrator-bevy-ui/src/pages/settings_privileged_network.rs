//! Bevy Settings projection for privileged network regression transactions.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::privileged_network::{PrivilegedNetworkSnapshot, PrivilegedNetworkState};

use super::SettingsProjectionUpdated;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrivilegedNetworkRunButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrivilegedNetworkStatusLine;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(space::S8)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            Children [
                (
                    Node {
                        flex_direction: bevy::ui::prelude::FlexDirection::Column,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        ( Text({ "特权网络无头回归 (Privileged Network Regression)".to_owned() }) TextRole(Role::Body) ),
                        ( Text(format_status(&projection.privileged_network)) PrivilegedNetworkStatusLine TextRole(Role::Mono) ),
                    ]
                ),
                (
                    Node {
                        min_height: px(palette.control_height_px),
                        padding: UiRect::horizontal(Val::Px(space::S12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.accent })
                    PrivilegedNetworkRunButton
                    Button
                    Children [
                        ( Text({ "运行回归".to_owned() }) TextRole(Role::BodyStrong) ),
                    ]
                ),
            ]
        })],
        palette,
    ))
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    buttons: Query<(), With<PrivilegedNetworkRunButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if buttons.contains(activate.entity)
        && let Some(handle) = handle
    {
        handle.submit(UiCommand::RunPrivilegedNetworkRegression);
    }
}

pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    mut status_lines: Query<&mut Text, With<PrivilegedNetworkStatusLine>>,
) {
    let status = format_status(&update.0.privileged_network);
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }
}

pub(super) fn format_status(snapshot: &PrivilegedNetworkSnapshot) -> String {
    match &snapshot.state {
        PrivilegedNetworkState::Idle => "未运行".to_owned(),
        PrivilegedNetworkState::Injecting => "注入中".to_owned(),
        PrivilegedNetworkState::Active => {
            format!("已注入 · operations={}", snapshot.operation_count)
        }
        PrivilegedNetworkState::RollingBack => "回滚清理中".to_owned(),
        PrivilegedNetworkState::Cleaned => format!(
            "已清理 · operations={} · rollback={}",
            snapshot.operation_count, snapshot.rollback_attempted
        ),
        PrivilegedNetworkState::Unsupported { reason } => format!("宿主不支持 · {reason}"),
        PrivilegedNetworkState::Failed { failure } => format!("失败 · {}", failure.message),
    }
}
