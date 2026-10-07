//! Scoped native component access for app systems.

use super::{BottomNavBar, ShellRoot, SidebarPanel};
use crate::chrome::ChromeDragBar;
use bevy::ecs::query::{QueryFilter, With, Without};

#[derive(QueryFilter)]
pub struct SyncResponsiveShellSidebarsFilter {
    with_sidebar_panel: With<SidebarPanel>,
    without_bottom_nav_bar: Without<BottomNavBar>,
}

#[derive(QueryFilter)]
pub struct SyncResponsiveShellBottomNavsFilter {
    with_bottom_nav_bar: With<BottomNavBar>,
    without_sidebar_panel: Without<SidebarPanel>,
}

#[derive(QueryFilter)]
pub struct SyncSafeAreaInsetsShellRootsFilter {
    with_shell_root: With<ShellRoot>,
    without_bottom_nav_bar: Without<BottomNavBar>,
    without_chrome_drag_bar: Without<ChromeDragBar>,
}

#[derive(QueryFilter)]
pub struct SyncSafeAreaInsetsBottomNavsFilter {
    with_bottom_nav_bar: With<BottomNavBar>,
    without_shell_root: Without<ShellRoot>,
    without_chrome_drag_bar: Without<ChromeDragBar>,
}

#[derive(QueryFilter)]
pub struct SyncSafeAreaInsetsChromeBarsFilter {
    with_chrome_drag_bar: With<ChromeDragBar>,
    without_shell_root: Without<ShellRoot>,
    without_bottom_nav_bar: Without<BottomNavBar>,
}
