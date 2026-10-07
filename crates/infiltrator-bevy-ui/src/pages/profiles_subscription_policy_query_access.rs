//! Scoped native component access for profiles subscription policy systems.

use super::{
    SaveSubscriptionPolicyButton, SubscriptionAutoReloadStatus, SubscriptionAutoReloadToggle,
    SubscriptionAutoUpdateToggle, SubscriptionPolicyCronField, SubscriptionPolicyIntervalField,
    SubscriptionPolicyStatus, SubscriptionPolicyUrlField,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Checkbox;
use infiltrator_bevy_widgets::text_input::TextField;

#[derive(QueryFilter)]
pub struct SyncSubscriptionPolicyControlsStatusLinesFilter {
    with_subscription_policy_status: With<SubscriptionPolicyStatus>,
    without_subscription_auto_reload_status: Without<SubscriptionAutoReloadStatus>,
}

#[derive(QueryFilter)]
pub struct SyncSubscriptionPolicyControlsReloadLinesFilter {
    with_subscription_auto_reload_status: With<SubscriptionAutoReloadStatus>,
    without_subscription_policy_status: Without<SubscriptionPolicyStatus>,
}

#[derive(SystemParam)]
pub struct SubscriptionPolicyTargets<'w, 's> {
    pub(super) urls: Query<'w, 's, &'static Children, With<SubscriptionPolicyUrlField>>,
    pub(super) intervals: Query<'w, 's, &'static Children, With<SubscriptionPolicyIntervalField>>,
    pub(super) crons: Query<'w, 's, &'static Children, With<SubscriptionPolicyCronField>>,
    pub(super) auto_update_toggles:
        Query<'w, 's, &'static Children, With<SubscriptionAutoUpdateToggle>>,
    pub(super) auto_reload_toggles:
        Query<'w, 's, &'static Children, With<SubscriptionAutoReloadToggle>>,
    pub(super) checkboxes: Query<'w, 's, (Entity, Has<Checked>), With<Checkbox>>,
    pub(super) text_fields: Query<'w, 's, &'static mut TextField>,
    pub(super) status_lines:
        Query<'w, 's, &'static mut Text, SyncSubscriptionPolicyControlsStatusLinesFilter>,
    pub(super) reload_lines:
        Query<'w, 's, &'static mut Text, SyncSubscriptionPolicyControlsReloadLinesFilter>,
}

#[derive(SystemParam)]
pub struct SubscriptionPolicyControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<SaveSubscriptionPolicyButton>>,
    pub(super) urls: Query<'w, 's, &'static Children, With<SubscriptionPolicyUrlField>>,
    pub(super) intervals: Query<'w, 's, &'static Children, With<SubscriptionPolicyIntervalField>>,
    pub(super) crons: Query<'w, 's, &'static Children, With<SubscriptionPolicyCronField>>,
    pub(super) auto_update_toggles:
        Query<'w, 's, &'static Children, With<SubscriptionAutoUpdateToggle>>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
}
