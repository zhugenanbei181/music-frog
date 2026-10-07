//! Scoped native component access for proxies systems.

use super::{
    FilterAliveToggle, NodePinButton, ProxyGroupFoldButton, ProxyNodeButton, ProxySortPill,
    TestAllProxiesButton, TestProxyGroupButton, ToggleViewModeButton,
};
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, SystemParam};

#[derive(SystemParam)]
pub struct ProxyActionControls<'w, 's> {
    pub(super) test_all_buttons: Query<'w, 's, (), With<TestAllProxiesButton>>,
    pub(super) test_group_buttons: Query<'w, 's, &'static TestProxyGroupButton>,
    pub(super) fold_buttons: Query<'w, 's, &'static ProxyGroupFoldButton>,
    pub(super) node_buttons: Query<'w, 's, &'static ProxyNodeButton>,
    pub(super) filter_alive_toggles: Query<'w, 's, (), With<FilterAliveToggle>>,
    pub(super) sort_pills: Query<'w, 's, &'static ProxySortPill>,
    pub(super) pin_buttons: Query<'w, 's, &'static NodePinButton>,
    pub(super) toggle_view_buttons: Query<'w, 's, (), With<ToggleViewModeButton>>,
}
