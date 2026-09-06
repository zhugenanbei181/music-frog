//! Bevy Settings projection for Android VpnService lifecycle.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};

use super::SettingsProjectionUpdated;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStartButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStopButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStatusLine;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let snapshot = &projection.vpn;
    let start_label = match snapshot.state {
        VpnSessionState::Running | VpnSessionState::Starting => "VPN 已启动",
        _ => "启动 VPN",
    };
    let stop_label = if matches!(
        snapshot.state,
        VpnSessionState::Running | VpnSessionState::Starting | VpnSessionState::Stopping
    ) {
        "停止 VPN"
    } else {
        "已停止"
    };
    Box::new(surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S6),
            }
            Children [
                ( Text({ "Android VpnService 与前台保活 (VPN)".to_owned() }) TextRole(Role::BodyStrong) ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(space::S8)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    Children [
                        ( Text(format_status(snapshot)) VpnStatusLine TextRole(Role::Mono) ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S6),
                            }
                            Children [
                                (
                                    Node {
                                        min_height: px(palette.control_height_px),
                                        padding: UiRect::horizontal(Val::Px(space::S12)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                    }
                                    BackgroundColor({ palette.accent })
                                    VpnStartButton
                                    Button
                                    Children [
                                        ( Text({ start_label.to_owned() }) TextRole(Role::BodyStrong) ),
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
                                    BackgroundColor({ palette.surface_elevated })
                                    VpnStopButton
                                    Button
                                    Children [
                                        ( Text({ stop_label.to_owned() }) TextRole(Role::Body) ),
                                    ]
                                ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    ))
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    start_buttons: Query<(), With<VpnStartButton>>,
    stop_buttons: Query<(), With<VpnStopButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if start_buttons.contains(activate.entity) {
        handle.submit(UiCommand::StartVpn);
    } else if stop_buttons.contains(activate.entity) {
        handle.submit(UiCommand::StopVpn);
    }
}

pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    mut status_lines: Query<&mut Text, With<VpnStatusLine>>,
) {
    let status = format_status(&update.0.vpn);
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }
}

pub(super) fn format_status(snapshot: &VpnSessionSnapshot) -> String {
    match &snapshot.state {
        VpnSessionState::Idle => "未启动".to_owned(),
        VpnSessionState::PermissionRequired => "等待 Android VPN 用户授权".to_owned(),
        VpnSessionState::Starting => format!(
            "启动中 · foreground={} · routes={}",
            snapshot.foreground, snapshot.route_count
        ),
        VpnSessionState::Running => format!(
            "运行中 · foreground={} · MTU={} · routes={} · IPv6={}",
            snapshot.foreground,
            snapshot.mtu.unwrap_or_default(),
            snapshot.route_count,
            snapshot.ipv6
        ),
        VpnSessionState::Stopping => "停止中".to_owned(),
        VpnSessionState::Stopped => "已停止".to_owned(),
        VpnSessionState::Revoked => "系统已撤销 VPN 授权".to_owned(),
        VpnSessionState::Unsupported { reason } => format!("宿主不支持 · {reason}"),
        VpnSessionState::Failed { failure } => format!("失败 · {}", failure.message),
    }
}
