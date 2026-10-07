//! Headless integration tests for Page Matrix B (DNS, Doctor, Settings, Sync, AppRouting):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - WebDAV conflict resolution, DNS cache clearing, Doctor repair actions, and AppRouting rules.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::With;
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ValueChange};
use infiltrator_bevy_ui::app::{
    ShellPlugin, SidebarSystemProxyToggle, SidebarToggleProjection, SidebarTunToggle,
};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::app_routing::*;
use infiltrator_bevy_ui::pages::app_routing_uwp::{UwpAction, UwpActionButton};
use infiltrator_bevy_ui::pages::dns::*;
use infiltrator_bevy_ui::pages::dns_edit::{
    DnsEditApplyButton, DnsEditField, DnsEditGeoipToggle, DnsEditStatusLine, DnsEditTemplate,
};
use infiltrator_bevy_ui::pages::dns_fakeip::DnsFakeIpSearchField;
use infiltrator_bevy_ui::pages::dns_hosts::{DnsHostsEditorField, DnsHostsStatusLine};
use infiltrator_bevy_ui::pages::doctor::*;
use infiltrator_bevy_ui::pages::settings::settings_core::{
    CoreLogLevelButton, ProbeTunMtuButton, SettingsProjection, TunEnableToggle, TunRouteToggle,
    TunRouteToggleKind, TunStackButton, TunStackButtonAvailability,
};
use infiltrator_bevy_ui::pages::settings::settings_ipv6::Ipv6RoutingToggle;
use infiltrator_bevy_ui::pages::settings::settings_lan::{
    LanAllowedIpsField, LanAuthPasswordField, LanAuthUsernameField, LanAuthenticationToggle,
    LanBindAddressField, LanDisallowedIpsField, LanMixedPortField, LanSecurityApplyButton,
    LanSharingApplyButton, LanSharingToggle, LanSkipAuthPrefixesField,
};
use infiltrator_bevy_ui::pages::settings::settings_network_roaming::{
    NetworkRoamingRefreshButton, NetworkRoamingRepairButton,
};
use infiltrator_bevy_ui::pages::settings::settings_pac::{PacApplyButton, PacBypassField};
use infiltrator_bevy_ui::pages::settings::settings_privileged_network::PrivilegedNetworkRunButton;
use infiltrator_bevy_ui::pages::settings::settings_system::SystemProxyToggle;
use infiltrator_bevy_ui::pages::settings::settings_vpn::{VpnStartButton, VpnStopButton};
use infiltrator_bevy_ui::pages::settings::{
    CloseToTrayToggle, CoreRollbackButton, PortConflictButton, PrepareTunPermissionButton,
    SaveSettingsButton, ServiceModeButton, SettingsProjectionUpdated, SystemNotificationsToggle,
};
use infiltrator_bevy_ui::pages::sync::*;
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{DemoSurfaceSource, SurfaceSnapshotUpdated, SurfaceSource};
use infiltrator_bevy_widgets::button::{ControlVisual, PillLabel};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode, DnsSwitchField};
use infiltrator_contract::dns_form::DnsFormField;
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::{LanCredentials, LanSecuritySnapshot};
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, PhysicalMtuSnapshot};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingSnapshot, NetworkRoamingStatus,
};
use infiltrator_contract::offline_startup::{LocalAssetStatus, OfflineStartupSnapshot};
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::system_proxy::{
    SystemProxyDesiredState, SystemProxyObservation, SystemProxyRecoveryStatus,
};
use infiltrator_contract::tun::TunStack;
use infiltrator_contract::version::CoreRollbackSnapshot;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use std::sync::Arc;

use crate::support::*;

pub(crate) fn setup_matrix_b_app(sink: Arc<DemoCommandSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    app
}

fn navigate_to(app: &mut App, route: Route) -> (Entity, Entity) {
    app.world_mut().commands().trigger(RouteChanged(route));
    app.update();
    let slot = content_slot(app.world_mut());
    let (root, mounted_route) = page_root(app.world_mut());
    assert_eq!(mounted_route, route);
    let parent = app
        .world()
        .get::<ChildOf>(root)
        .expect("page root parent")
        .0;
    assert_eq!(parent, slot, "page root is parented under ContentSlot");
    (root, slot)
}
// 1. DNS Page Tests
// 1b. DNS Workbench Form Parity Tests (DUAL-14-04 / 14-05 / 14-07 / 14-14)

fn set_dns_field(app: &mut App, field: DnsFormField, value: &str) {
    let parent = app
        .world_mut()
        .query::<(Entity, &DnsEditField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == field)
        .map(|(entity, _)| entity)
        .expect("edit field row");
    let child = *app
        .world()
        .get::<Children>(parent)
        .expect("edit field children")
        .iter()
        .next()
        .expect("text field child");
    app.world_mut()
        .entity_mut(child)
        .get_mut::<TextField>()
        .expect("text field component")
        .0
        .apply(TextFieldInput::SetText(value.to_owned()));
}

fn trigger_dns_edit_apply(app: &mut App) {
    let button = app
        .world_mut()
        .query_filtered::<Entity, With<DnsEditApplyButton>>()
        .single(app.world())
        .expect("dns edit apply button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
}

/// The rendered text of one typed DNS line kind.
fn dns_line_text(app: &mut App, kind: DnsLineKind) -> String {
    let world = app.world_mut();
    let mut query = world.query::<(&Text, &DnsLine)>();
    query
        .iter(world)
        .find(|(_, line)| line.0 == kind)
        .map(|(text, _)| text.0.clone())
        .expect("dns line")
}
// 1c. DNS Fake-IP Pool / Latency / Hosts Editor (DUAL-14-06 / 14-10 / 14-11)
// 2. Doctor Page Tests
// 3. Settings Page Tests
// 4. Sync Page Tests
// 5. AppRouting Page Tests

#[path = "pages_matrix_b_tests/app.rs"]
mod app;
#[path = "pages_matrix_b_tests/dns_cache.rs"]
mod dns_cache;
#[path = "pages_matrix_b_tests/dns_clear.rs"]
mod dns_clear;
#[path = "pages_matrix_b_tests/dns_edit.rs"]
mod dns_edit;
#[path = "pages_matrix_b_tests/dns_enhanced.rs"]
mod dns_enhanced;
#[path = "pages_matrix_b_tests/dns_fake.rs"]
mod dns_fake;
#[path = "pages_matrix_b_tests/dns_fallback.rs"]
mod dns_fallback;
#[path = "pages_matrix_b_tests/dns_filter.rs"]
mod dns_filter;
#[path = "pages_matrix_b_tests/dns_form.rs"]
mod dns_form;
#[path = "pages_matrix_b_tests/dns_hosts.rs"]
mod dns_hosts;
#[path = "pages_matrix_b_tests/dns_latency.rs"]
mod dns_latency;
#[path = "pages_matrix_b_tests/dns_leak.rs"]
mod dns_leak;
#[path = "pages_matrix_b_tests/dns_page.rs"]
mod dns_page;
#[path = "pages_matrix_b_tests/dns_projection.rs"]
mod dns_projection;
#[path = "pages_matrix_b_tests/dns_quick.rs"]
mod dns_quick;
#[path = "pages_matrix_b_tests/dns_self.rs"]
mod dns_self;
#[path = "pages_matrix_b_tests/dns_stun.rs"]
mod dns_stun;
#[path = "pages_matrix_b_tests/dns_switch.rs"]
mod dns_switch;
#[path = "pages_matrix_b_tests/dns_test.rs"]
mod dns_test;
#[path = "pages_matrix_b_tests/dns_upstream.rs"]
mod dns_upstream;
#[path = "pages_matrix_b_tests/doctor.rs"]
mod doctor;
#[path = "pages_matrix_b_tests/privileged.rs"]
mod privileged;
#[path = "pages_matrix_b_tests/settings_core.rs"]
mod settings_core;
#[path = "pages_matrix_b_tests/settings_ipv6.rs"]
mod settings_ipv6;
#[path = "pages_matrix_b_tests/settings_lan.rs"]
mod settings_lan;
#[path = "pages_matrix_b_tests/settings_mtu.rs"]
mod settings_mtu;
#[path = "pages_matrix_b_tests/settings_network.rs"]
mod settings_network;
#[path = "pages_matrix_b_tests/settings_pac.rs"]
mod settings_pac;
#[path = "pages_matrix_b_tests/settings_page.rs"]
mod settings_page;
#[path = "pages_matrix_b_tests/settings_prepare.rs"]
mod settings_prepare;
#[path = "pages_matrix_b_tests/settings_projection.rs"]
mod settings_projection;
#[path = "pages_matrix_b_tests/settings_save.rs"]
mod settings_save;
#[path = "pages_matrix_b_tests/settings_system.rs"]
mod settings_system;
#[path = "pages_matrix_b_tests/settings_tun.rs"]
mod settings_tun;
#[path = "pages_matrix_b_tests/settings_vpn.rs"]
mod settings_vpn;
#[path = "pages_matrix_b_tests/sidebar.rs"]
mod sidebar;
#[path = "pages_matrix_b_tests/sync.rs"]
mod sync;
