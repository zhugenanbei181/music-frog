//! Scoped replay of runtime control availability on the Settings page.
use super::LastSettingsProjection;
use super::settings_core::{
    CoreLogLevelButton, TunEnableToggle, TunRouteToggle, TunRouteToggleKind, TunStackButton,
};
use super::settings_ipv6::Ipv6RoutingToggle;
use super::settings_lan::{
    LanAllowedIpsField, LanAuthPasswordField, LanAuthUsernameField, LanAuthenticationToggle,
    LanBindAddressField, LanDisallowedIpsField, LanMixedPortField, LanSecurityApplyButton,
    LanSharingApplyButton, LanSharingToggle, LanSkipAuthPrefixesField,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface_snapshot::PageStatus;

#[derive(Clone, Copy)]
pub enum RuntimeField {
    Tun,
    AutoRoute,
    StrictRoute,
    Stack,
    Ipv6,
    Lan,
    Security,
    LogLevel,
}

#[derive(SystemParam)]
pub struct RuntimePolicy<'w> {
    last: Option<Res<'w, LastSettingsProjection>>,
}
impl RuntimePolicy<'_> {
    pub fn preference(&self, key: &str) -> Option<bool> {
        let value = self.last.as_ref()?.0.as_ref()?;
        if value.preference_status != PageStatus::Ready {
            return None;
        }
        match key {
            "close_to_tray" => value.close_to_tray,
            "notifications_enabled" => value.notifications_enabled,
            _ => None,
        }
    }

    pub fn available(&self, field: RuntimeField) -> bool {
        let Some(value) = self.last.as_ref().and_then(|last| last.0.as_ref()) else {
            return false;
        };
        if value.runtime_status != RuntimeControlStatus::Ready {
            return false;
        }
        match field {
            RuntimeField::Tun => value.tun_enabled.is_some(),
            RuntimeField::AutoRoute => {
                value.tun_auto_route.is_some() && value.tun_strict_route.is_some()
            }
            RuntimeField::StrictRoute => {
                value.tun_auto_route.is_some() && value.tun_strict_route.is_some()
            }
            RuntimeField::Stack => value.tun_stack.is_some(),
            RuntimeField::Ipv6 => value.ipv6_routing.is_some(),
            RuntimeField::Lan => {
                value.allow_lan.is_some()
                    && value.mixed_port.is_some()
                    && value.lan_bind_address.is_some()
            }
            RuntimeField::Security => value.lan_security.is_some(),
            RuntimeField::LogLevel => value.log_level.is_some(),
        }
    }
}

#[derive(QueryData)]
pub struct RuntimeGroup {
    children: &'static Children,
    tun: Option<&'static TunEnableToggle>,
    route: Option<&'static TunRouteToggle>,
    ipv6: Option<&'static Ipv6RoutingToggle>,
    lan: Option<&'static LanSharingToggle>,
    auth: Option<&'static LanAuthenticationToggle>,
    mixed: Option<&'static LanMixedPortField>,
    bind: Option<&'static LanBindAddressField>,
    allowed: Option<&'static LanAllowedIpsField>,
    disallowed: Option<&'static LanDisallowedIpsField>,
    skip: Option<&'static LanSkipAuthPrefixesField>,
    username: Option<&'static LanAuthUsernameField>,
    password: Option<&'static LanAuthPasswordField>,
}

#[derive(QueryData)]
pub struct RuntimeButton {
    entity: Entity,
    disabled: Option<&'static ButtonDisabled>,
    stack: Option<&'static TunStackButton>,
    level: Option<&'static CoreLogLevelButton>,
    lan: Option<&'static LanSharingApplyButton>,
    security: Option<&'static LanSecurityApplyButton>,
}

pub fn sync(
    policy: RuntimePolicy,
    groups: Query<RuntimeGroup>,
    buttons: Query<RuntimeButton>,
    flags: Query<&ButtonDisabled>,
    mut fields: Query<&mut TextField>,
    mut commands: Commands,
) {
    for group in &groups {
        let field = if group.tun.is_some() {
            Some(RuntimeField::Tun)
        } else if let Some(route) = group.route {
            Some(match route.0 {
                TunRouteToggleKind::AutoRoute => RuntimeField::AutoRoute,
                TunRouteToggleKind::StrictRoute => RuntimeField::StrictRoute,
            })
        } else if group.ipv6.is_some() {
            Some(RuntimeField::Ipv6)
        } else if group.lan.is_some() || group.mixed.is_some() || group.bind.is_some() {
            Some(RuntimeField::Lan)
        } else if group.auth.is_some()
            || group.allowed.is_some()
            || group.disallowed.is_some()
            || group.skip.is_some()
            || group.username.is_some()
            || group.password.is_some()
        {
            Some(RuntimeField::Security)
        } else {
            None
        };
        let Some(field) = field else {
            continue;
        };
        let disabled = !policy.available(field);
        for child in group.children.iter() {
            if let Ok(mut text) = fields.get_mut(*child) {
                text.0.set_disabled(disabled);
            }
            if flags.get(*child).is_ok_and(|flag| flag.0 == disabled) {
                continue;
            }
            commands.entity(*child).insert(ButtonDisabled(disabled));
        }
    }
    for button in &buttons {
        let field = if button.stack.is_some() {
            Some(RuntimeField::Stack)
        } else if button.level.is_some() {
            Some(RuntimeField::LogLevel)
        } else if button.lan.is_some() {
            Some(RuntimeField::Lan)
        } else if button.security.is_some() {
            Some(RuntimeField::Security)
        } else {
            None
        };
        let Some(field) = field else {
            continue;
        };
        let disabled = !policy.available(field)
            || button
                .stack
                .is_some_and(|button| !button.stack.is_live_supported());
        if button.disabled.is_none_or(|flag| flag.0 != disabled) {
            commands
                .entity(button.entity)
                .insert(ButtonDisabled(disabled));
        }
    }
}
