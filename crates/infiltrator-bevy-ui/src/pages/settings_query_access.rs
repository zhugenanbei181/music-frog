//! Scoped native component access for settings systems.

use super::settings_core::{
    CoreLogLevelButton, SettingsLine, TunStackButton, TunStackButtonAvailability,
};
use super::{
    CloseToTrayToggle, CoreRollbackAvailability, CoreRollbackButton, CoreRollbackButtonLabel,
    PortConflictButton, PrepareTunPermissionButton, SaveSettingsButton, ServiceModeAvailability,
    ServiceModeButton, ServiceModeButtonLabel, SystemNotificationsToggle,
};
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{ParamSet, Query, SystemParam};
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::ButtonDisabled;

#[derive(SystemParam)]
pub struct SettingsActionControls<'w, 's> {
    pub(super) preference_disabled: Query<'w, 's, &'static ButtonDisabled, PreferenceButtonFilter>,
    pub(super) save_buttons: Query<'w, 's, (), With<SaveSettingsButton>>,
    pub(super) prepare_buttons: Query<'w, 's, (), With<PrepareTunPermissionButton>>,
    pub(super) tray_toggles: Query<'w, 's, (), With<CloseToTrayToggle>>,
    pub(super) notif_toggles: Query<'w, 's, (), With<SystemNotificationsToggle>>,
    pub(super) rollback_buttons: Query<'w, 's, (), With<CoreRollbackButton>>,
    pub(super) rollback_available:
        Query<'w, 's, &'static CoreRollbackAvailability, With<CoreRollbackButton>>,
    pub(super) log_level_buttons: Query<'w, 's, &'static CoreLogLevelButton>,
    pub(super) tun_stack_buttons: Query<'w, 's, &'static TunStackButton>,
    pub(super) tun_stack_available:
        Query<'w, 's, &'static TunStackButtonAvailability, With<TunStackButton>>,
    pub(super) service_buttons: Query<'w, 's, (), With<ServiceModeButton>>,
    pub(super) service_available:
        Query<'w, 's, &'static ServiceModeAvailability, With<ServiceModeButton>>,
    pub(super) port_buttons: Query<'w, 's, (), With<PortConflictButton>>,
}

#[derive(SystemParam)]
pub struct SettingsProjectionTargets<'w, 's> {
    pub(super) button_queries: ParamSet<
        'w,
        's,
        (
            Query<'w, 's, LogLevelControl>,
            Query<'w, 's, TunStackControl>,
        ),
    >,
    pub(super) lines: Query<'w, 's, SettingsTextOutput>,
    pub(super) rollback_buttons:
        Query<'w, 's, &'static mut CoreRollbackAvailability, With<CoreRollbackButton>>,
    pub(super) service_buttons:
        Query<'w, 's, &'static mut ServiceModeAvailability, With<ServiceModeButton>>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct SettingsTextOutput {
    pub(super) text: &'static mut Text,
    pub(super) line: Option<&'static SettingsLine>,
    pub(super) rollback_label: Option<&'static CoreRollbackButtonLabel>,
    pub(super) service_label: Option<&'static ServiceModeButtonLabel>,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct LogLevelControl {
    pub(super) background: &'static mut BackgroundColor,
    pub(super) button: &'static CoreLogLevelButton,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct TunStackControl {
    pub(super) background: &'static mut BackgroundColor,
    pub(super) availability: &'static mut TunStackButtonAvailability,
    pub(super) button: &'static TunStackButton,
}

#[derive(QueryFilter)]
pub struct PreferenceButtonFilter {
    native: Or<(With<CloseToTrayToggle>, With<SystemNotificationsToggle>)>,
}
