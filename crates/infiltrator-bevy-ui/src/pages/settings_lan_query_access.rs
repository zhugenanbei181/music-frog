//! Scoped native component access for settings lan systems.

use super::{
    LanAllowedIpsField, LanAuthPasswordField, LanAuthUsernameField, LanAuthenticationToggle,
    LanBindAddressField, LanDisallowedIpsField, LanFieldBaseline, LanMixedPortField,
    LanSecurityApplyButton, LanSharingToggle, LanSkipAuthPrefixesField,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::Checked;
use bevy::ui_widgets::Checkbox;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};

#[derive(SystemParam)]
pub struct LanSecurityControls<'w, 's> {
    pub(super) apply_buttons: Query<'w, 's, (), With<LanSecurityApplyButton>>,
    pub(super) allowed_fields: Query<'w, 's, &'static Children, With<LanAllowedIpsField>>,
    pub(super) disallowed_fields: Query<'w, 's, &'static Children, With<LanDisallowedIpsField>>,
    pub(super) skip_fields: Query<'w, 's, &'static Children, With<LanSkipAuthPrefixesField>>,
    pub(super) auth_toggles: Query<'w, 's, &'static Children, With<LanAuthenticationToggle>>,
    pub(super) username_fields: Query<'w, 's, &'static Children, With<LanAuthUsernameField>>,
    pub(super) password_fields: Query<'w, 's, &'static Children, With<LanAuthPasswordField>>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
}

#[derive(SystemParam)]
pub struct LanProjectionTargets<'w, 's> {
    pub(super) lan_toggles: Query<'w, 's, &'static Children, With<LanSharingToggle>>,
    pub(super) auth_toggles: Query<'w, 's, &'static Children, With<LanAuthenticationToggle>>,
    pub(super) checkboxes: Query<'w, 's, (Entity, Has<Checked>), With<Checkbox>>,
    pub(super) mixed_fields: Query<'w, 's, &'static Children, With<LanMixedPortField>>,
    pub(super) bind_fields: Query<'w, 's, &'static Children, With<LanBindAddressField>>,
    pub(super) allowed_fields: Query<'w, 's, &'static Children, With<LanAllowedIpsField>>,
    pub(super) disallowed_fields: Query<'w, 's, &'static Children, With<LanDisallowedIpsField>>,
    pub(super) skip_fields: Query<'w, 's, &'static Children, With<LanSkipAuthPrefixesField>>,
    pub(super) username_fields: Query<'w, 's, &'static Children, With<LanAuthUsernameField>>,
    pub(super) text_fields: Query<'w, 's, &'static mut TextField>,
    pub(super) focus: Query<'w, 's, &'static TextFieldFocused>,
    pub(super) initial: Query<'w, 's, &'static LanFieldBaseline>,
}
