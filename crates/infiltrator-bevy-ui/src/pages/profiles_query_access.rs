//! Scoped native component access for profiles systems.

use super::{
    ActivateProfileButton, DeleteProfileButton, ProfileNameText, ProfileScheduleText,
    ProfileStatusText, ProfileTimeText, ProfileTrafficText, ProfilesLine,
};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::ControlVisual;

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionLinesFilter {
    with_profiles_line: With<ProfilesLine>,
    without_profile_name_text: Without<ProfileNameText>,
    without_profile_time_text: Without<ProfileTimeText>,
    without_profile_traffic_text: Without<ProfileTrafficText>,
    without_profile_status_text: Without<ProfileStatusText>,
}

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionNamesFilter {
    with_profile_name_text: With<ProfileNameText>,
    without_profiles_line: Without<ProfilesLine>,
    without_profile_time_text: Without<ProfileTimeText>,
    without_profile_traffic_text: Without<ProfileTrafficText>,
    without_profile_status_text: Without<ProfileStatusText>,
}

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionTimesFilter {
    with_profile_time_text: With<ProfileTimeText>,
    without_profiles_line: Without<ProfilesLine>,
    without_profile_name_text: Without<ProfileNameText>,
    without_profile_traffic_text: Without<ProfileTrafficText>,
    without_profile_status_text: Without<ProfileStatusText>,
}

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionTrafficsFilter {
    with_profile_traffic_text: With<ProfileTrafficText>,
    without_profiles_line: Without<ProfilesLine>,
    without_profile_name_text: Without<ProfileNameText>,
    without_profile_time_text: Without<ProfileTimeText>,
    without_profile_status_text: Without<ProfileStatusText>,
}

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionStatusesFilter {
    with_profile_status_text: With<ProfileStatusText>,
    without_profiles_line: Without<ProfilesLine>,
    without_profile_name_text: Without<ProfileNameText>,
    without_profile_time_text: Without<ProfileTimeText>,
    without_profile_traffic_text: Without<ProfileTrafficText>,
    without_profile_schedule_text: Without<ProfileScheduleText>,
}

#[derive(QueryFilter)]
pub struct ApplyProfilesProjectionSchedulesFilter {
    with_profile_schedule_text: With<ProfileScheduleText>,
    without_profiles_line: Without<ProfilesLine>,
    without_profile_name_text: Without<ProfileNameText>,
    without_profile_time_text: Without<ProfileTimeText>,
    without_profile_traffic_text: Without<ProfileTrafficText>,
    without_profile_status_text: Without<ProfileStatusText>,
}

#[derive(SystemParam)]
pub struct ProfileProjectionTargets<'w, 's> {
    pub(super) lines: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfilesLine),
        ApplyProfilesProjectionLinesFilter,
    >,
    pub(super) names: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfileNameText),
        ApplyProfilesProjectionNamesFilter,
    >,
    pub(super) times: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfileTimeText),
        ApplyProfilesProjectionTimesFilter,
    >,
    pub(super) traffics: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfileTrafficText),
        ApplyProfilesProjectionTrafficsFilter,
    >,
    pub(super) statuses: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfileStatusText),
        ApplyProfilesProjectionStatusesFilter,
    >,
    pub(super) schedules: Query<
        'w,
        's,
        (&'static mut Text, &'static ProfileScheduleText),
        ApplyProfilesProjectionSchedulesFilter,
    >,
    pub(super) buttons: Query<
        'w,
        's,
        (
            &'static mut BackgroundColor,
            &'static mut ControlVisual,
            &'static mut ActivateProfileButton,
        ),
        Without<DeleteProfileButton>,
    >,
    pub(super) delete_buttons: Query<
        'w,
        's,
        (&'static mut DeleteProfileButton, &'static mut ControlVisual),
        Without<ActivateProfileButton>,
    >,
}
