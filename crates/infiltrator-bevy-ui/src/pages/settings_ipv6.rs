//! Bevy Settings controls for Mihomo's top-level IPv6 routing policy.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Checkbox, ValueChange};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use super::settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};
use super::SettingsProjectionUpdated;
use crate::command::{CommandSinkHandle, UiCommand};

/// Parent marker for the top-level Mihomo IPv6 policy checkbox.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ipv6RoutingToggle;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let status = format_status(&projection.ipv6_routing);
    Box::new(surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(space::S8)),
                    }
                    Ipv6RoutingToggle
                    Children [
                        ( { checkbox_scene("允许 IPv6 内核流量 (Mihomo IPv6)".to_owned(), projection.ipv6_routing.enabled, palette) } ),
                        ( Text(status) SettingsLine(SettingsLineKind::Ipv6Routing) TextRole(Role::Mono) ),
                    ]
                ),
            ]
        })],
        palette,
    ))
}

pub(super) fn on_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<Ipv6RoutingToggle>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    if toggles.get(parent.0).is_ok() {
        handle.submit(UiCommand::SetIpv6Routing {
            enabled: change.value,
        });
    }
}

pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    toggles: Query<&Children, With<Ipv6RoutingToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    mut lines: Query<(&mut Text, &SettingsLine)>,
    mut commands: Commands,
) {
    let wanted = update.0.ipv6_routing.enabled;
    let status = format_status(&update.0.ipv6_routing);
    for (mut text, line) in &mut lines {
        if line.0 == SettingsLineKind::Ipv6Routing {
            text.0 = status.clone();
        }
    }
    for children in &toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != wanted
            {
                if wanted {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }
}

pub(super) fn format_status(snapshot: &infiltrator_contract::ipv6::Ipv6RoutingSnapshot) -> String {
    let policy = if snapshot.enabled {
        "已允许 IPv6"
    } else {
        "已禁用 IPv6（防止旁路泄漏）"
    };
    let tun = if snapshot.tun_enabled {
        "TUN 已启用"
    } else {
        "TUN 未启用"
    };
    format!("{policy} · {tun}")
}
