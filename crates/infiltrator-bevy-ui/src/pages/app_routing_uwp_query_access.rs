//! Scoped native component access for app routing uwp systems.

use super::{UwpPackageName, UwpPackageState, UwpStatusLine};
use bevy::ecs::query::{QueryFilter, With, Without};

#[derive(QueryFilter)]
pub struct ApplyProjectionStatusLinesFilter {
    with_uwp_status_line: With<UwpStatusLine>,
    without_uwp_package_name: Without<UwpPackageName>,
    without_uwp_package_state: Without<UwpPackageState>,
}

#[derive(QueryFilter)]
pub struct ApplyProjectionNamesFilter {
    without_uwp_status_line: Without<UwpStatusLine>,
    without_uwp_package_state: Without<UwpPackageState>,
}

#[derive(QueryFilter)]
pub struct ApplyProjectionStatesFilter {
    without_uwp_status_line: Without<UwpStatusLine>,
    without_uwp_package_name: Without<UwpPackageName>,
}
