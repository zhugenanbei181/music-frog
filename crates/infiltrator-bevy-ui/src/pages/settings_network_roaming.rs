//! Bevy projection and actions for physical-link roaming recovery.

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
use infiltrator_contract::network_roaming::{
    NetworkRoamingEvent, NetworkRoamingSnapshot, NetworkRoamingStatus,
};

use super::SettingsProjectionUpdated;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingRefreshButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingRepairButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingStatusLine;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingInterfacesLine;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingRouteLine;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let snapshot = &projection.network_roaming;
    let status = format_status(&snapshot.status);
    let interfaces = format_interfaces(snapshot);
    let route = format_route(snapshot);
    Box::new(surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S6),
            }
            Children [
                ( Text({ "物理网卡漫游与默认网关感知 (Network Roaming)".to_owned() }) TextRole(Role::BodyStrong) ),
                (
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S4),
                        padding: UiRect::all(Val::Px(space::S8)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    Children [
                        ( Text(status) NetworkRoamingStatusLine TextRole(Role::Mono) ),
                        ( Text(interfaces) NetworkRoamingInterfacesLine TextRole(Role::Caption) ),
                        ( Text(route) NetworkRoamingRouteLine TextRole(Role::Mono) ),
                        (
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexEnd,
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
                                    BackgroundColor({ palette.surface_elevated })
                                    NetworkRoamingRefreshButton
                                    Button
                                    Children [
                                        ( Text({ "刷新链路".to_owned() }) TextRole(Role::Body) ),
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
                                    NetworkRoamingRepairButton
                                    Button
                                    Children [
                                        ( Text({ "立即修复 TUN 路由".to_owned() }) TextRole(Role::BodyStrong) ),
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
    refresh_buttons: Query<(), With<NetworkRoamingRefreshButton>>,
    repair_buttons: Query<(), With<NetworkRoamingRepairButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if refresh_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RefreshNetworkRoaming);
    } else if repair_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RepairNetworkRoutes);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    mut lines: Query<(
        &mut Text,
        Option<&NetworkRoamingStatusLine>,
        Option<&NetworkRoamingInterfacesLine>,
        Option<&NetworkRoamingRouteLine>,
    )>,
) {
    let snapshot = &update.0.network_roaming;
    let status = format_status(&snapshot.status);
    let interfaces = format_interfaces(snapshot);
    let route = format_route(snapshot);
    for (mut line, status_line, interface_line, route_line) in &mut lines {
        if status_line.is_some() {
            line.0 = status.clone();
        }
        if interface_line.is_some() {
            line.0 = interfaces.clone();
        }
        if route_line.is_some() {
            line.0 = route.clone();
        }
    }
}

pub(super) fn format_status(status: &NetworkRoamingStatus) -> String {
    match status {
        NetworkRoamingStatus::Unknown => "未探测".to_owned(),
        NetworkRoamingStatus::Stable => "链路稳定 · 自动路由已监控".to_owned(),
        NetworkRoamingStatus::Recovering => "正在修复 TUN 路由".to_owned(),
        NetworkRoamingStatus::Degraded { reason } => format!("降级 · {reason}"),
        NetworkRoamingStatus::Unsupported { reason } => format!("宿主不支持 · {reason}"),
        NetworkRoamingStatus::Failed { failure } => format!("修复失败 · {}", failure.message),
    }
}

fn format_interfaces(snapshot: &NetworkRoamingSnapshot) -> String {
    if snapshot.interfaces.is_empty() {
        return "接口事实尚未可用".to_owned();
    }
    snapshot
        .interfaces
        .iter()
        .take(12)
        .map(|interface| {
            let state = if interface.is_up { "up" } else { "down" };
            let gateway = interface.gateway_ip.as_deref().unwrap_or("—");
            format!("{} [{state}] gw={gateway}", interface.name)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn format_route(snapshot: &NetworkRoamingSnapshot) -> String {
    let active = snapshot.active_interface.as_deref().unwrap_or("—");
    let gateway = snapshot.default_gateway.as_deref().unwrap_or("—");
    let mtu = snapshot
        .physical_mtu
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let tun_mtu = snapshot
        .recommended_tun_mtu
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let event = snapshot
        .last_event
        .as_ref()
        .map(format_event)
        .unwrap_or_else(|| "无最近事件".to_owned());
    format!(
        "active={active} · gateway={gateway} · physical MTU={mtu} → TUN MTU={tun_mtu} · {event}"
    )
}

fn format_event(event: &NetworkRoamingEvent) -> String {
    match event {
        NetworkRoamingEvent::InitialObservation { .. } => "初始观测".to_owned(),
        NetworkRoamingEvent::GatewayChanged { .. } => "检测到默认网关迁移".to_owned(),
        NetworkRoamingEvent::InterfaceAddressChanged { interface } => {
            format!("地址变化: {interface}")
        }
        NetworkRoamingEvent::RoutesRepaired { detail, .. } => format!("路由已修复: {detail}"),
        NetworkRoamingEvent::RepairSkipped { reason } => format!("跳过修复: {reason}"),
        NetworkRoamingEvent::RepairFailed { failure } => format!("修复失败: {}", failure.message),
    }
}
