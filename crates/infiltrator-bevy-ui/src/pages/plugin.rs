//! Product assembly owns page resources and observers; route entities carry no wiring hooks.
use crate::pages::app_routing::AppRoutingPagePlugin;
use crate::pages::connections::ConnectionsPagePlugin;
use crate::pages::dns::DnsPagePlugin;
use crate::pages::doctor::DoctorPagePlugin;
use crate::pages::logs::LogsPagePlugin;
use crate::pages::overview::OverviewPagePlugin;
use crate::pages::profiles::ProfilesPagePlugin;
use crate::pages::proxies::ProxiesPagePlugin;
use crate::pages::rules::RulesPagePlugin;
use crate::pages::settings::SettingsPagePlugin;
use crate::pages::sync::SyncPagePlugin;
use bevy::app::{App, Plugin};

#[derive(Default)]
pub struct PageBindingsPlugin;

impl Plugin for PageBindingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            OverviewPagePlugin,
            ProxiesPagePlugin,
            ProfilesPagePlugin,
            RulesPagePlugin,
            ConnectionsPagePlugin,
            LogsPagePlugin,
            DnsPagePlugin,
            DoctorPagePlugin,
            AppRoutingPagePlugin,
            SyncPagePlugin,
            SettingsPagePlugin,
        ));
    }
}
