//! Capture drives actual production observers and observes their visible surfaces.
use crate::command_palette::{CommandPaletteState, OpenCommandPalette};
use crate::pages::app_routing::AppRoutingPageRoot;
use crate::pages::connections::ConnectionsPageRoot;
use crate::pages::connections::{CloseAllConnectionsButton, ConnInspectButton};
use crate::pages::dns::DnsPageRoot;
use crate::pages::doctor::DoctorPageRoot;
use crate::pages::logs::LogsPageRoot;
use crate::pages::overview::OverviewPageRoot;
use crate::pages::overview_speedtest::OverviewSpeedtestDetailButton;
use crate::pages::profiles::ProfilesPageRoot;
use crate::pages::proxies::ProxiesPageRoot;
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_draft;
use crate::pages::settings::SettingsPageRoot;
use crate::pages::sync::SyncPageRoot;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::prelude::*;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::SystemParam;
use bevy::ui::UiSystems;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_contract::parity::FeatureId;
use std::env;

mod connections;
mod dns_cache;
mod dns_hosts;
mod dns_leak;
mod dns_query;
mod doctor;
mod filter;
mod geometry;
mod group_order;
mod host;
mod language;
mod logs;
mod logs_export;
pub mod observation;
mod probe_settings;
mod protocol;
mod proxies;
mod proxy_inspection;
mod proxy_mode;
mod rule_list;
mod rule_statistics;
mod rule_trace;
mod scroll;
mod search;
mod snapshot_restore;
mod telemetry;

#[derive(Resource, Clone, Copy, Debug)]
pub struct InteractionCapture {
    pub feature: FeatureId,
    pub activated: bool,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CaptureObservationSet;

#[derive(Resource, Default)]
pub struct ObservedInteraction(pub Option<[f32; 4]>);

impl ObservedInteraction {
    pub fn publish(&mut self, state: &mut InteractionCapture, bounds: Option<[f32; 4]>) {
        self.0 = bounds;
        state.activated = bounds.is_some();
    }
}

pub fn selected(
    state: &InteractionCapture,
    route: &ActiveRoute,
    feature: FeatureId,
    page: Route,
) -> bool {
    state.feature == feature && route.0 == Some(page)
}

pub fn supported(_feature: FeatureId) -> bool {
    true
}

impl InteractionCapture {
    pub fn install(app: &mut App, feature: FeatureId) {
        assert!(
            supported(feature),
            "interaction capture has no production activator"
        );
        app.init_resource::<geometry::GeometryRejections>();
        app.init_resource::<ObservedInteraction>();
        if feature == FeatureId::ProfilesSnapshotRestoreConfirm {
            snapshot_restore::install(app);
        }
        if feature == FeatureId::RuntimeTelemetryObservation {
            telemetry::install(app);
            app.add_systems(Update, telemetry::activate);
            app.add_systems(PostUpdate, telemetry::observe.in_set(CaptureObservationSet));
        }
        if matches!(
            feature,
            FeatureId::ShellProxyModeControl | FeatureId::ShellProxyModeAuthentication
        ) {
            proxy_mode::install(app, feature);
            app.add_systems(Update, proxy_mode::activate);
            app.add_systems(
                PostUpdate,
                proxy_mode::observe.in_set(CaptureObservationSet),
            );
        }
        if matches!(
            feature,
            FeatureId::ConnectionsGrouping | FeatureId::ConnectionsSearchHighlight
        ) {
            connections::install(app);
            app.add_systems(Update, connections::activate);
            app.add_systems(
                PostUpdate,
                connections::observe.in_set(CaptureObservationSet),
            );
        }
        if matches!(
            feature,
            FeatureId::LogsSearchHighlight
                | FeatureId::LogsScrollLock
                | FeatureId::LogsRedactedExport
        ) {
            logs::install(app);
            app.add_systems(Update, logs::activate);
            if feature == FeatureId::LogsRedactedExport {
                app.add_systems(Update, logs_export::activate.after(logs::activate));
                app.add_systems(
                    PostUpdate,
                    logs_export::observe.in_set(CaptureObservationSet),
                );
            } else {
                app.add_systems(PostUpdate, logs::observe.in_set(CaptureObservationSet));
            }
        }
        if feature == FeatureId::ProfilesFilterEditor {
            filter::install(app);
            app.add_systems(Update, filter::activate);
            app.add_systems(PostUpdate, filter::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::RulesStatisticsInspector {
            rule_statistics::install(app);
            app.add_systems(
                Update,
                rule_statistics::activate.before(rules_draft::observe),
            );
            app.add_systems(
                PostUpdate,
                rule_statistics::observe.in_set(CaptureObservationSet),
            );
        }
        if feature == FeatureId::RulesListEditor {
            rule_list::install(app);
            app.add_systems(Update, rule_list::activate.before(rules_draft::observe));
            app.add_systems(PostUpdate, rule_list::observe.in_set(CaptureObservationSet));
        }
        app.configure_sets(
            PostUpdate,
            CaptureObservationSet.after(UiSystems::PostLayout),
        );
        if matches!(
            feature,
            FeatureId::RulesTracerDrawer | FeatureId::RulesOverrideEditor
        ) {
            rule_trace::install(app);
            app.add_systems(Update, rule_trace::activate);
            app.add_systems(
                PostUpdate,
                rule_trace::observe.in_set(CaptureObservationSet),
            );
        }
        if feature == FeatureId::ProxiesGroupReorder {
            group_order::install(app);
            app.add_systems(Update, group_order::activate);
            app.add_systems(
                PostUpdate,
                group_order::observe.in_set(CaptureObservationSet),
            );
        }
        if feature == FeatureId::DnsQueryDetails {
            dns_query::install(app);
            app.add_systems(Update, dns_query::activate);
            app.add_systems(PostUpdate, dns_query::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::DnsFakeIpFlushConfirm {
            app.add_systems(Update, dns_cache::activate);
            app.add_systems(PostUpdate, dns_cache::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::DnsHostsEditor {
            dns_hosts::install(app);
            app.add_systems(Update, dns_hosts::activate);
            app.add_systems(PostUpdate, dns_hosts::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::DnsLeakAlert {
            dns_leak::install(app);
            app.add_systems(Update, dns_leak::activate);
            app.add_systems(PostUpdate, dns_leak::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::DoctorFailureRecovery {
            doctor::install(app);
            app.add_systems(Update, doctor::activate);
            app.add_systems(PostUpdate, doctor::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::SettingsLanguageChoice {
            language::install(app);
            app.add_systems(Update, language::activate);
            app.add_systems(PostUpdate, language::observe.in_set(CaptureObservationSet));
        }
        if matches!(
            feature,
            FeatureId::ProxiesCustomNodeModal | FeatureId::ProxiesUriImportPreview
        ) {
            protocol::install(app);
            app.add_systems(Update, protocol::activate);
        }
        if feature == FeatureId::ProxiesProbeSettings {
            app.add_systems(Update, probe_settings::activate);
            app.add_systems(
                PostUpdate,
                probe_settings::observe.in_set(CaptureObservationSet),
            );
        }
        if feature == FeatureId::ProxiesGroupExpanded {
            proxies::install(app);
            app.add_systems(Update, proxies::activate);
            app.add_systems(PostUpdate, proxies::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::ProxiesSearchHighlight {
            search::install(app);
            app.add_systems(Update, search::activate);
            app.add_systems(PostUpdate, search::observe.in_set(CaptureObservationSet));
        }
        if feature == FeatureId::ProxiesNodeDetailDrawer {
            proxy_inspection::install(app);
            app.add_systems(Update, proxy_inspection::activate);
        }
        app.insert_resource(Self {
            feature,
            activated: false,
        });
        match feature {
            FeatureId::ConnectionsCloseAllConfirm => {
                app.add_systems(Update, activate_close_all);
                app.add_systems(
                    PostUpdate,
                    observation::close_all_observation.in_set(CaptureObservationSet),
                );
            }
            FeatureId::ConnectionsDetailsDrawer => {
                app.add_systems(Update, activate_drawer);
                app.add_systems(
                    PostUpdate,
                    observation::drawer_observation.in_set(CaptureObservationSet),
                );
            }
            FeatureId::SpeedtestDetailsModal => {
                app.add_systems(Update, activate_speedtest);
                app.add_systems(
                    PostUpdate,
                    observation::speedtest_observation.in_set(CaptureObservationSet),
                );
            }
            FeatureId::ShellCommandPalette => {
                app.add_systems(Update, activate_palette);
                app.add_systems(
                    PostUpdate,
                    observation::palette_observation.in_set(CaptureObservationSet),
                );
            }
            FeatureId::ProxiesCustomNodeModal | FeatureId::ProxiesUriImportPreview => {
                app.add_systems(
                    PostUpdate,
                    observation::protocol_observation.in_set(CaptureObservationSet),
                );
            }
            FeatureId::ProxiesNodeDetailDrawer => {
                app.add_systems(
                    PostUpdate,
                    observation::inspection_observation.in_set(CaptureObservationSet),
                );
            }
            _ => {}
        }
        app.add_systems(
            PostUpdate,
            default_page_observation.in_set(CaptureObservationSet),
        );
    }
}

#[derive(SystemParam)]
pub struct DefaultPageObservation<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    overview: Query<'w, 's, Entity, With<OverviewPageRoot>>,
    proxies: Query<'w, 's, Entity, With<ProxiesPageRoot>>,
    rules: Query<'w, 's, Entity, With<RulesPageRoot>>,
    connections: Query<'w, 's, Entity, With<ConnectionsPageRoot>>,
    logs: Query<'w, 's, Entity, With<LogsPageRoot>>,
    dns: Query<'w, 's, Entity, With<DnsPageRoot>>,
    doctor: Query<'w, 's, Entity, With<DoctorPageRoot>>,
    app_routing: Query<'w, 's, Entity, With<AppRoutingPageRoot>>,
    sync: Query<'w, 's, Entity, With<SyncPageRoot>>,
    settings: Query<'w, 's, Entity, With<SettingsPageRoot>>,
    profiles: Query<'w, 's, Entity, With<ProfilesPageRoot>>,
}

fn has_specialized_activator(feature: FeatureId) -> bool {
    matches!(
        feature,
        FeatureId::ConnectionsCloseAllConfirm
            | FeatureId::RuntimeTelemetryObservation
            | FeatureId::ConnectionsGrouping
            | FeatureId::ConnectionsSearchHighlight
            | FeatureId::LogsSearchHighlight
            | FeatureId::LogsScrollLock
            | FeatureId::LogsRedactedExport
            | FeatureId::ConnectionsDetailsDrawer
            | FeatureId::SpeedtestDetailsModal
            | FeatureId::ShellCommandPalette
            | FeatureId::ShellProxyModeControl
            | FeatureId::ShellProxyModeAuthentication
            | FeatureId::ProxiesCustomNodeModal
            | FeatureId::ProxiesUriImportPreview
            | FeatureId::ProxiesGroupExpanded
            | FeatureId::ProxiesNodeDetailDrawer
            | FeatureId::ProxiesSearchHighlight
            | FeatureId::ProxiesProbeSettings
            | FeatureId::ProxiesGroupReorder
            | FeatureId::SettingsLanguageChoice
            | FeatureId::DoctorFailureRecovery
            | FeatureId::DnsLeakAlert
            | FeatureId::DnsHostsEditor
            | FeatureId::DnsQueryDetails
            | FeatureId::DnsFakeIpFlushConfirm
            | FeatureId::RulesTracerDrawer
            | FeatureId::RulesOverrideEditor
            | FeatureId::RulesStatisticsInspector
            | FeatureId::RulesListEditor
            | FeatureId::ProfilesFilterEditor
            | FeatureId::ProfilesSnapshotRestoreConfirm
    )
}

fn default_page_observation(
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    pages: DefaultPageObservation,
    geometry: geometry::CaptureGeometry,
) {
    if has_specialized_activator(state.feature) || state.activated || observation.0.is_some() {
        return;
    }
    let target = match state.feature.spec().bevy_page {
        "overview" => pages.overview.iter().next(),
        "proxies" => pages.proxies.iter().next(),
        "rules" | "rules-providers" => pages.rules.iter().next(),
        "connections" => pages.connections.iter().next(),
        "logs" => pages.logs.iter().next(),
        "dns" => pages.dns.iter().next(),
        "doctor" => pages.doctor.iter().next(),
        "app_routing" | "app-routing" => pages.app_routing.iter().next(),
        "sync" => pages.sync.iter().next(),
        "settings" => pages.settings.iter().next(),
        "profiles" | "editor" | "filter" => pages.profiles.iter().next(),
        _ => None,
    };
    if let Some(entity) = target
        && let Some(rect) = geometry.rect(entity)
    {
        let Ok(window) = pages.windows.single() else {
            return;
        };
        let win_w = window.physical_width() as f32;
        let win_h = window.physical_height() as f32;
        let x = rect[0].clamp(0.0, (win_w - 20.0).max(0.0));
        let y = rect[1].clamp(0.0, (win_h - 20.0).max(0.0));
        let w = rect[2].min(win_w - x).max(10.0);
        let h = rect[3].min(win_h - y).max(10.0);
        if x + w <= win_w && y + h <= win_h {
            observation.publish(&mut state, Some([x, y, w, h]));
        }
    }
}

fn activate_close_all(
    mut state: ResMut<InteractionCapture>,
    route: Res<ActiveRoute>,
    buttons: Query<Entity, With<CloseAllConnectionsButton>>,
    mut commands: Commands,
) {
    if !state.activated
        && selected(
            &state,
            &route,
            FeatureId::ConnectionsCloseAllConfirm,
            Route::Connections,
        )
        && let Some(entity) = buttons.iter().next()
    {
        commands.trigger(Activate { entity });
        state.activated = true;
    }
}
fn activate_drawer(
    mut state: ResMut<InteractionCapture>,
    route: Res<ActiveRoute>,
    buttons: Query<(Entity, &ConnInspectButton)>,
    mut commands: Commands,
) {
    if !state.activated
        && selected(
            &state,
            &route,
            FeatureId::ConnectionsDetailsDrawer,
            Route::Connections,
        )
        && let Some((entity, _)) = buttons.iter().find(|(_, button)| button.0 == 0)
    {
        commands.trigger(Activate { entity });
        state.activated = true;
    }
}
fn activate_speedtest(
    mut state: ResMut<InteractionCapture>,
    route: Res<ActiveRoute>,
    buttons: Query<Entity, With<OverviewSpeedtestDetailButton>>,
    mut commands: Commands,
) {
    if !state.activated
        && selected(
            &state,
            &route,
            FeatureId::SpeedtestDetailsModal,
            Route::Overview,
        )
        && let Some(entity) = buttons.iter().next()
    {
        commands.trigger(Activate { entity });
        state.activated = true;
    }
}
fn activate_palette(
    mut state: ResMut<InteractionCapture>,
    route: Res<ActiveRoute>,
    mut palette: ResMut<CommandPaletteState>,
    mut commands: Commands,
) {
    if state.activated
        || !selected(
            &state,
            &route,
            FeatureId::ShellCommandPalette,
            Route::Overview,
        )
    {
        return;
    }
    if !palette.is_open {
        commands.trigger(OpenCommandPalette);
    } else {
        palette.set_query("dns");
        state.activated = true;
    }
}

pub fn feature_from_environment() -> Option<FeatureId> {
    let raw = env::var("INFILTRATOR_SCENARIO").ok()?;
    if raw.is_empty() {
        return None;
    }
    FeatureId::ALL
        .iter()
        .copied()
        .find(|feature| feature.spec().id == raw && supported(*feature))
        .or_else(|| panic!("unknown or unimplemented interaction capture: {raw}"))
}
