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
use infiltrator_bevy_widgets::text_input::{
    TextField, password_field_scene, text_field_with_placeholder_scene,
};
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, TextFieldState};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use infiltrator_contract::lan::LanCredentials;
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

/// Parent markers for the ACL/authentication draft fields.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanAllowedIpsField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanDisallowedIpsField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanSkipAuthPrefixesField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanAuthenticationToggle;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanAuthUsernameField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanAuthPasswordField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanSecurityApplyButton;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let mixed_port = projection.mixed_port.to_string();
    let bind_address = projection.lan_bind_address.clone();
    let allowed_ips = projection.lan_security.allowed_ips.join(", ");
    let disallowed_ips = projection.lan_security.disallowed_ips.join(", ");
    let skip_auth_prefixes = projection.lan_security.skip_auth_prefixes.join(", ");
    let auth_username = projection
        .lan_security
        .authentication_username
        .clone()
        .unwrap_or_else(|| "musicfrog".to_owned());
    let auth_status = if projection.lan_security.authentication_enabled {
        format!(
            "已启用 · {} 个账号",
            projection.lan_security.authentication_user_count
        )
    } else {
        "未启用".to_owned()
    };
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
                    (
                        Node {
                            width: percent(100),
                            padding: UiRect::top(Val::Px(space::S8)),
                        }
                        Children [
                            ( Text({ "接入 ACL 与 HTTP 基本认证".to_owned() }) TextRole(Role::BodyStrong) ),
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
                            ( Text({ "允许网段 (Allowed CIDR)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(360.0) }
                                LanAllowedIpsField
                                Children [
                                    ( { text_field_with_placeholder_scene(allowed_ips, "192.168.0.0/16, 10.0.0.0/8".to_owned(), palette) } ),
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
                            ( Text({ "拒绝网段 (Denied CIDR)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(360.0) }
                                LanDisallowedIpsField
                                Children [
                                    ( { text_field_with_placeholder_scene(disallowed_ips, "192.168.1.10/32".to_owned(), palette) } ),
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
                            ( Text({ "免认证网段 (Skip Auth CIDR)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(360.0) }
                                LanSkipAuthPrefixesField
                                Children [
                                    ( { text_field_with_placeholder_scene(skip_auth_prefixes, "127.0.0.0/8, ::1/128".to_owned(), palette) } ),
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
                        LanAuthenticationToggle
                        Children [
                            ( { checkbox_scene("启用 HTTP 基本认证 (HTTP Basic Auth)".to_owned(), projection.lan_security.authentication_enabled, palette) } ),
                            ( Text({ auth_status }) SettingsLine(SettingsLineKind::LanSecurity) TextRole(Role::Mono) ),
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
                            ( Text({ "认证用户名 (Username)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(240.0) }
                                LanAuthUsernameField
                                Children [
                                    ( { text_field_with_placeholder_scene(auth_username, "musicfrog".to_owned(), palette) } ),
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
                            ( Text({ "认证密码 (Password)".to_owned() }) TextRole(Role::Body) ),
                            (
                                Node { width: px(240.0) }
                                LanAuthPasswordField
                                Children [
                                    ( { password_field_scene(String::new(), "Apply to set password".to_owned(), palette) } ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexEnd,
                            padding: UiRect::all(Val::Px(space::S8)),
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
                                LanSecurityApplyButton
                                Button
                                Children [
                                    ( Text({ "应用 ACL 与认证 (Apply)".to_owned() }) TextRole(Role::BodyStrong) ),
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

#[allow(clippy::too_many_arguments)]
pub(super) fn on_security_apply_activated(
    activate: On<Activate>,
    apply_buttons: Query<(), With<LanSecurityApplyButton>>,
    allowed_fields: Query<&Children, With<LanAllowedIpsField>>,
    disallowed_fields: Query<&Children, With<LanDisallowedIpsField>>,
    skip_fields: Query<&Children, With<LanSkipAuthPrefixesField>>,
    auth_toggles: Query<&Children, With<LanAuthenticationToggle>>,
    username_fields: Query<&Children, With<LanAuthUsernameField>>,
    password_fields: Query<&Children, With<LanAuthPasswordField>>,
    checkboxes: Query<&Checked>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    let Some(handle) = handle else {
        return;
    };
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    let authentication_enabled = auth_toggles
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok());
    let credentials = authentication_enabled.then(|| LanCredentials {
        username: field_value(&username_fields, &text_fields).unwrap_or_default(),
        password: field_value(&password_fields, &text_fields).unwrap_or_default(),
    });
    handle.submit(UiCommand::SetLanSecurity {
        allowed_ips: field_value(&allowed_fields, &text_fields)
            .map_or_else(Vec::new, |value| split_values(&value)),
        disallowed_ips: field_value(&disallowed_fields, &text_fields)
            .map_or_else(Vec::new, |value| split_values(&value)),
        skip_auth_prefixes: field_value(&skip_fields, &text_fields)
            .map_or_else(Vec::new, |value| split_values(&value)),
        authentication_enabled,
        credentials,
    });
    for children in &password_fields {
        for child in children.iter() {
            commands.entity(*child).insert(TextField(
                TextFieldState::new("")
                    .with_masked(true)
                    .with_placeholder("Apply to set password"),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    lan_toggles: Query<&Children, With<LanSharingToggle>>,
    auth_toggles: Query<&Children, With<LanAuthenticationToggle>>,
    checkboxes: Query<(bevy::ecs::entity::Entity, bevy::ecs::query::Has<Checked>), With<Checkbox>>,
    mixed_fields: Query<&Children, With<LanMixedPortField>>,
    bind_fields: Query<&Children, With<LanBindAddressField>>,
    allowed_fields: Query<&Children, With<LanAllowedIpsField>>,
    disallowed_fields: Query<&Children, With<LanDisallowedIpsField>>,
    skip_fields: Query<&Children, With<LanSkipAuthPrefixesField>>,
    username_fields: Query<&Children, With<LanAuthUsernameField>>,
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
    for children in &auth_toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != projection.lan_security.authentication_enabled
            {
                if projection.lan_security.authentication_enabled {
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
    restamp_field(
        &allowed_fields,
        &mut text_fields,
        &projection.lan_security.allowed_ips.join(", "),
    );
    restamp_field(
        &disallowed_fields,
        &mut text_fields,
        &projection.lan_security.disallowed_ips.join(", "),
    );
    restamp_field(
        &skip_fields,
        &mut text_fields,
        &projection.lan_security.skip_auth_prefixes.join(", "),
    );
    if let Some(username) = projection.lan_security.authentication_username.as_deref() {
        restamp_field(&username_fields, &mut text_fields, username);
    }
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

fn split_values(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn format_auth_status(snapshot: &infiltrator_contract::lan::LanSecuritySnapshot) -> String {
    if snapshot.authentication_enabled {
        format!("已启用 · {} 个账号", snapshot.authentication_user_count)
    } else {
        "未启用".to_owned()
    }
}
