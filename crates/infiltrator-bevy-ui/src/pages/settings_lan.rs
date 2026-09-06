//! Bevy Settings controls for Mihomo Allow-LAN listener settings.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent, px};
use bevy::ui::widget::Text;
use bevy::ui::Checked;
use bevy::ui_widgets::{Activate, Button, Checkbox, ValueChange};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use super::settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};
use super::SettingsProjectionUpdated;

/// Parent marker for the Allow-LAN checkbox.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanSharingToggle;

/// Parent marker for the live mixed-port text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanMixedPortField;

/// Parent marker for the live bind-address text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanBindAddressField;

/// Apply button for the complete Allow-LAN listener tuple.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanSharingApplyButton;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let mixed_port = projection.mixed_port.to_string();
    let bind_address = projection.lan_bind_address.clone();
    Box::new(surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "局域网共享代理 (Allow LAN)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            width: percent(100),
                            padding: UiRect::horizontal(Val::Px(space::S4)),
                        }
                        LanSharingToggle
                        Children [
                            ( { checkbox_scene("开启局域网共享 (Allow LAN)".to_owned(), projection.allow_lan, palette) } ),
                        ]
                    ),
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
                            ( Text({ "混合代理端口 (Mixed Port)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(180.0) }
                                LanMixedPortField
                                Children [
                                    ( { text_field_with_placeholder_scene(mixed_port, "7890".to_owned(), palette) } ),
                                ]
                            ),
                        ]
                    ),
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
                            ( Text({ "绑定地址 (Bind Address)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(300.0) }
                                LanBindAddressField
                                Children [
                                    ( { text_field_with_placeholder_scene(bind_address.clone(), "* / 192.168.1.10 / [::1]".to_owned(), palette) } ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                        }
                        Children [
                            ( Text({ "当前绑定".to_owned() }) TextRole(Role::Caption) ),
                            ( Text({ bind_address.clone() }) SettingsLine(SettingsLineKind::LanBindAddress) TextRole(Role::Mono) ),
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                LanSharingApplyButton
                                Button
                                Children [
                                    ( Text({ "应用并回读 (Apply)".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    ))
}

pub(super) fn on_toggle_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<LanSharingToggle>>,
    mixed_fields: Query<&Children, With<LanMixedPortField>>,
    bind_fields: Query<&Children, With<LanBindAddressField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    if toggles.get(parent.0).is_ok() {
        submit_lan_command(
            &handle,
            change.value,
            &mixed_fields,
            &bind_fields,
            &text_fields,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn on_apply_activated(
    activate: On<Activate>,
    apply_buttons: Query<(), With<LanSharingApplyButton>>,
    mixed_fields: Query<&Children, With<LanMixedPortField>>,
    bind_fields: Query<&Children, With<LanBindAddressField>>,
    text_fields: Query<&TextField>,
    toggles: Query<&Children, With<LanSharingToggle>>,
    checkboxes: Query<&bevy::ui::Checked>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    let enabled = toggles
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok());
    submit_lan_command(
        &handle,
        enabled,
        &mixed_fields,
        &bind_fields,
        &text_fields,
    );
}

pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    lan_toggles: Query<&Children, With<LanSharingToggle>>,
    checkboxes: Query<(bevy::ecs::entity::Entity, bevy::ecs::query::Has<Checked>), With<Checkbox>>,
    mixed_fields: Query<&Children, With<LanMixedPortField>>,
    bind_fields: Query<&Children, With<LanBindAddressField>>,
    mut text_fields: Query<&mut TextField>,
    mut commands: Commands,
) {
    let projection = &update.0;
    for children in &lan_toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != projection.allow_lan
            {
                if projection.allow_lan {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }
    restamp_field(
        &mixed_fields,
        &mut text_fields,
        &projection.mixed_port.to_string(),
    );
    restamp_field(
        &bind_fields,
        &mut text_fields,
        &projection.lan_bind_address,
    );
}

fn restamp_field<T: Component>(
    fields: &Query<&Children, With<T>>,
    text_fields: &mut Query<&mut TextField>,
    value: &str,
) {
    for children in fields.iter() {
        for child in children.iter() {
            if let Ok(mut text_field) = text_fields.get_mut(*child)
                && text_field.0.text() != value
            {
                text_field.0.apply(TextFieldInput::SetText(value.to_owned()));
            }
        }
    }
}

fn field_value<T>(fields: &Query<&Children, With<T>>, text_fields: &Query<&TextField>) -> Option<String>
where
    T: Component,
{
    fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok().map(|field| field.0.text().to_owned()))
}

fn submit_lan_command(
    handle: &CommandSinkHandle,
    enabled: bool,
    mixed_fields: &Query<&Children, With<LanMixedPortField>>,
    bind_fields: &Query<&Children, With<LanBindAddressField>>,
    text_fields: &Query<&TextField>,
) {
    let Some(mixed_port) = field_value(mixed_fields, text_fields)
        .and_then(|value| value.trim().parse::<u16>().ok())
    else {
        return;
    };
    let Some(bind_address) = field_value(bind_fields, text_fields) else {
        return;
    };
    handle.submit(UiCommand::SetLanSharing {
        enabled,
        mixed_port,
        bind_address,
    });
}
