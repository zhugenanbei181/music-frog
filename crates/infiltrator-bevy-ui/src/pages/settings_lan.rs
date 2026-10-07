//! Bevy Settings controls for Mihomo Allow-LAN listener settings.

#[path = "settings_lan_query_access.rs"]
pub mod query_access;
use self::query_access::{LanProjectionTargets, LanSecurityControls};

use super::SettingsProjectionUpdated;
use super::settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};
use super::settings_runtime::{RuntimeField, RuntimePolicy};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{BorderRadius, Checked};
use bevy::ui_widgets::{Activate, Button, ValueChange};
use infiltrator_application::settings_status_projection::format_lan_auth;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, TextFieldState};
use infiltrator_bevy_widgets::text_input::{
    TextField, TextFieldFocused, password_field_scene, text_field_with_placeholder_scene,
};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::lan::LanCredentials;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub(super) struct LanFieldObservations(HashMap<Entity, String>);
#[derive(Component, Clone, Debug, Default)]
pub(super) struct LanFieldBaseline(pub String);

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
    let mixed_port = projection
        .mixed_port
        .map(|value| value.to_string())
        .unwrap_or_default();
    let bind_address = projection.lan_bind_address.clone().unwrap_or_default();
    let allowed_ips = projection
        .lan_security
        .as_ref()
        .map(|value| value.allowed_ips.join(", "))
        .unwrap_or_default();
    let disallowed_ips = projection
        .lan_security
        .as_ref()
        .map(|value| value.disallowed_ips.join(", "))
        .unwrap_or_default();
    let skip_auth_prefixes = projection
        .lan_security
        .as_ref()
        .map(|value| value.skip_auth_prefixes.join(", "))
        .unwrap_or_default();
    let auth_username = projection
        .lan_security
        .as_ref()
        .and_then(|value| value.authentication_username.clone())
        .unwrap_or_default();
    let auth_status = format_lan_auth(projection.lan_security.as_ref(), UiLocale::default().code());
    Box::new(surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("settings_allow_lan_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    padding: UiRect::horizontal(Val::Px(space::S4)),
                                }
                                LanSharingToggle
                                Children [
                                    @{ localized_checkbox_scene(LocalizedText::plain("lan_allow_sharing"), projection.allow_lan == Some(true), palette) }
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_mixed_port_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(180.0) }
                                    LanMixedPortField
                                    Children [
                                        @{ text_field_with_placeholder_scene(mixed_port.clone(), "7890".to_owned(), palette) } LanFieldBaseline({ mixed_port.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_bind_address_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(300.0) }
                                    LanBindAddressField
                                    Children [
                                        @{ text_field_with_placeholder_scene(bind_address.clone(), "* / 192.168.1.10 / [::1]".to_owned(), palette) } LanFieldBaseline({ bind_address.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                }
                                Children [
                                    LocalizedText::plain("settings_current_bind_label") TextRole(Role::Caption)
                                    --
                                    Text({ bind_address.clone() }) SettingsLine(SettingsLineKind::LanBindAddress) TextRole(Role::Mono)
                                    --
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
                                        LocalizedText::plain("settings_apply_readback_action") TextRole(Role::BodyStrong)
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    padding: UiRect::top(Val::Px(space::S8)),
                                }
                                Children [
                                    LocalizedText::plain("settings_lan_access_control_title") TextRole(Role::BodyStrong)
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_allowed_cidr_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(360.0) }
                                    LanAllowedIpsField
                                    Children [
                                        @{ text_field_with_placeholder_scene(allowed_ips.clone(), "192.168.0.0/16, 10.0.0.0/8".to_owned(), palette) } LanFieldBaseline({ allowed_ips.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_denied_cidr_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(360.0) }
                                    LanDisallowedIpsField
                                    Children [
                                        @{ text_field_with_placeholder_scene(disallowed_ips.clone(), "192.168.1.10/32".to_owned(), palette) } LanFieldBaseline({ disallowed_ips.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_skip_auth_cidr_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(360.0) }
                                    LanSkipAuthPrefixesField
                                    Children [
                                        @{ text_field_with_placeholder_scene(skip_auth_prefixes.clone(), "127.0.0.0/8, ::1/128".to_owned(), palette) } LanFieldBaseline({ skip_auth_prefixes.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                }
                                LanAuthenticationToggle
                                Children [
                                    @{ localized_checkbox_scene(LocalizedText::plain("lan_require_basic_auth"), projection.lan_security.as_ref().is_some_and(|value| value.authentication_enabled), palette) }
                                    --
                                    Text({ auth_status }) SettingsLine(SettingsLineKind::LanSecurity) TextRole(Role::Mono)
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_auth_username_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(240.0) }
                                    LanAuthUsernameField
                                    Children [
                                        @{ text_field_with_placeholder_scene(auth_username.clone(), "musicfrog".to_owned(), palette) } LanFieldBaseline({ auth_username.clone() })
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_auth_password_label") TextRole(Role::Body)
                                    --
                                    Node { width: px(240.0) }
                                    LanAuthPasswordField
                                    Children [
                                        @{ password_field_scene(String::new(), "Apply to set password".to_owned(), palette) }
                                    ]
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::FlexEnd,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                }
                                Children [
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
                                        LocalizedText::plain("settings_lan_access_apply_action") TextRole(Role::BodyStrong)
                                    ]
                                ]
                            ]
            }),
        ],
        palette,
    ))
}

#[derive(SystemParam)]
pub struct LanListenerInputs<'w, 's> {
    mixed_fields: Query<'w, 's, &'static Children, With<LanMixedPortField>>,
    bind_fields: Query<'w, 's, &'static Children, With<LanBindAddressField>>,
    text_fields: Query<'w, 's, &'static TextField>,
}

pub(super) fn on_toggle_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<LanSharingToggle>>,
    handle: Option<Res<CommandSinkHandle>>,
    policy: RuntimePolicy,
    inputs: LanListenerInputs,
) {
    let LanListenerInputs {
        mixed_fields,
        bind_fields,
        text_fields,
    } = inputs;
    if !policy.available(RuntimeField::Lan) {
        return;
    }
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

pub(super) fn on_apply_activated(
    activate: On<Activate>,
    apply_buttons: Query<(), With<LanSharingApplyButton>>,
    toggles: Query<&Children, With<LanSharingToggle>>,
    checkboxes: Query<&Checked>,
    handle: Option<Res<CommandSinkHandle>>,
    policy: RuntimePolicy,
    inputs: LanListenerInputs,
) {
    let LanListenerInputs {
        mixed_fields,
        bind_fields,
        text_fields,
    } = inputs;
    if !policy.available(RuntimeField::Lan) {
        return;
    }
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
    submit_lan_command(&handle, enabled, &mixed_fields, &bind_fields, &text_fields);
}

pub(super) fn on_security_apply_activated(
    activate: On<Activate>,
    handle: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
    policy: RuntimePolicy,
    targets: LanSecurityControls,
) {
    let LanSecurityControls {
        apply_buttons,
        allowed_fields,
        disallowed_fields,
        skip_fields,
        auth_toggles,
        username_fields,
        password_fields,
        checkboxes,
        text_fields,
    } = targets;

    if !policy.available(RuntimeField::Security) {
        return;
    }
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

pub(super) fn apply_projection(
    update: On<SettingsProjectionUpdated>,
    mut observations: ResMut<LanFieldObservations>,
    mut commands: Commands,
    targets: LanProjectionTargets,
) {
    let LanProjectionTargets {
        lan_toggles,
        auth_toggles,
        checkboxes,
        mixed_fields,
        bind_fields,
        allowed_fields,
        disallowed_fields,
        skip_fields,
        username_fields,
        mut text_fields,
        focus,
        initial,
    } = targets;

    observations
        .0
        .retain(|entity, _| text_fields.contains(*entity));
    let projection = &update.0;
    for children in &lan_toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != (projection.allow_lan == Some(true))
            {
                if projection.allow_lan == Some(true) {
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
                && checked
                    != projection
                        .lan_security
                        .as_ref()
                        .is_some_and(|value| value.authentication_enabled)
            {
                if projection
                    .lan_security
                    .as_ref()
                    .is_some_and(|value| value.authentication_enabled)
                {
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
        &focus,
        &initial,
        &mut observations,
        &projection
            .mixed_port
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    restamp_field(
        &bind_fields,
        &mut text_fields,
        &focus,
        &initial,
        &mut observations,
        projection.lan_bind_address.as_deref().unwrap_or_default(),
    );
    restamp_field(
        &allowed_fields,
        &mut text_fields,
        &focus,
        &initial,
        &mut observations,
        &projection
            .lan_security
            .as_ref()
            .map(|value| value.allowed_ips.join(", "))
            .unwrap_or_default(),
    );
    restamp_field(
        &disallowed_fields,
        &mut text_fields,
        &focus,
        &initial,
        &mut observations,
        &projection
            .lan_security
            .as_ref()
            .map(|value| value.disallowed_ips.join(", "))
            .unwrap_or_default(),
    );
    restamp_field(
        &skip_fields,
        &mut text_fields,
        &focus,
        &initial,
        &mut observations,
        &projection
            .lan_security
            .as_ref()
            .map(|value| value.skip_auth_prefixes.join(", "))
            .unwrap_or_default(),
    );
    if let Some(username) = projection
        .lan_security
        .as_ref()
        .and_then(|value| value.authentication_username.as_deref())
    {
        restamp_field(
            &username_fields,
            &mut text_fields,
            &focus,
            &initial,
            &mut observations,
            username,
        );
    }
}

fn restamp_field<T: Component>(
    fields: &Query<&Children, With<T>>,
    text_fields: &mut Query<&mut TextField>,
    focus: &Query<&TextFieldFocused>,
    initial: &Query<&LanFieldBaseline>,
    observations: &mut LanFieldObservations,
    value: &str,
) {
    for children in fields.iter() {
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child) {
                let previous = observations
                    .0
                    .insert(*child, value.into())
                    .or_else(|| initial.get(*child).ok().map(|baseline| baseline.0.clone()));
                let unchanged_input = previous
                    .as_deref()
                    .is_some_and(|previous| field.0.text() == previous);
                let changed_fact = previous
                    .as_deref()
                    .is_some_and(|previous| previous != value);
                let editing =
                    focus.get(*child).is_ok_and(|focus| focus.0) || !field.0.preedit().is_empty();
                if changed_fact && unchanged_input && !editing {
                    field.0.apply(TextFieldInput::SetText(value.into()));
                }
            }
        }
    }
}

fn field_value<T>(
    fields: &Query<&Children, With<T>>,
    text_fields: &Query<&TextField>,
) -> Option<String>
where
    T: Component,
{
    fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| {
            text_fields
                .get(*child)
                .ok()
                .map(|field| field.0.text().to_owned())
        })
}

fn submit_lan_command(
    handle: &CommandSinkHandle,
    enabled: bool,
    mixed_fields: &Query<&Children, With<LanMixedPortField>>,
    bind_fields: &Query<&Children, With<LanBindAddressField>>,
    text_fields: &Query<&TextField>,
) {
    let Some(mixed_port) =
        field_value(mixed_fields, text_fields).and_then(|value| value.trim().parse::<u16>().ok())
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
