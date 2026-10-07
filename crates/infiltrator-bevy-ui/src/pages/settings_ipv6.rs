//! Bevy Settings controls for Mihomo's top-level IPv6 routing policy.

use super::SettingsProjectionUpdated;
use super::settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};
use super::settings_runtime::{RuntimeField, RuntimePolicy};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
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
use infiltrator_application::settings_status_projection::format_ipv6;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Parent marker for the top-level Mihomo IPv6 policy checkbox.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ipv6RoutingToggle;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let status = format_ipv6(projection.ipv6_routing.as_ref(), UiLocale::default().code());
    Box::new(surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                        }
                        Ipv6RoutingToggle
                        Children [
                            @{ localized_checkbox_scene(LocalizedText::plain("settings_allow_ipv6_traffic"), projection.ipv6_routing.is_some_and(|snapshot| snapshot.enabled), palette) }
                ButtonDisabled({ projection.ipv6_routing.is_none() })
                            --
                            Text(status) SettingsLine(SettingsLineKind::Ipv6Routing) TextRole(Role::Mono)
                        ]
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
    policy: RuntimePolicy,
) {
    if !policy.available(RuntimeField::Ipv6) {
        return;
    }
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
    let wanted = update.0.ipv6_routing.is_some_and(|value| value.enabled);
    let status = format_ipv6(update.0.ipv6_routing.as_ref(), UiLocale::default().code());
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
