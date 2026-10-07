//! Scoped native editor activation and actual denied-save evidence; no business World access.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::dns_hosts::{
    DnsHostsDomainField, DnsHostsEditorField, DnsHostsEditorState, HostAction, HostRowIdentity,
    HostsModalCard, HostsRows,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::app::Update;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::dns_hosts_fixtures::{HostsCaptureStore, ORIGINAL_HOSTS};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface_snapshot::PageData;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
#[derive(Resource)]
pub struct HostsCapture {
    store: Arc<HostsCaptureStore>,
    configuration: ConfigurationApplication,
    initialized: bool,
    stage: u8,
}
pub fn install(app: &mut App) {
    let store = Arc::new(HostsCaptureStore::default());
    store.deny_save.store(true, Ordering::SeqCst);
    let configuration = ConfigurationApplication::new(store.clone());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_configuration(configuration.clone()),
        CaptureCapability::Hosts,
    ))));
    app.insert_resource(HostsCapture {
        store,
        configuration,
        initialized: false,
        stage: 0,
    });
    app.add_systems(Update, initialize.before(activate));
}
fn initialize(mut capture: ResMut<HostsCapture>, mut latest: ResMut<LatestSurfaceSnapshot>) {
    if capture.initialized {
        return;
    }
    let read = capture.configuration.clone();
    let observed = Arc::new(Mutex::new(None));
    let result = observed.clone();
    tokio_application_runtime()
        .expect("isolated capture runtime")
        .block_on(Box::pin(async move {
            *result.lock().expect("capture observation") = Some(read.load_hosts_profile().await);
        }));
    let hosts = observed
        .lock()
        .expect("capture observation")
        .take()
        .expect("read completed");
    latest.0.dns_hosts = PageData::ready(hosts.expect("actual initial Hosts read"));
    capture.initialized = true;
}
pub fn activate(
    feature: Res<InteractionCapture>,
    route: Res<ActiveRoute>,
    mut capture: ResMut<HostsCapture>,
    editor: Res<DnsHostsEditorState>,
    actions: Query<(Entity, &HostAction)>,
    mut commands: Commands,
) {
    if !selected(&feature, &route, FeatureId::DnsHostsEditor, Route::Dns) {
        return;
    }
    let wanted = match capture.stage {
        0 => HostAction::Open,
        1 if editor.editor.open => HostAction::ImportLegacy,
        2 if editor.editor.importing_legacy && editor.editor.can_apply() => HostAction::Apply,
        _ => return,
    };
    if let Some((entity, _)) = actions.iter().find(|(_, action)| **action == wanted) {
        commands.trigger(Activate { entity });
        capture.stage += 1;
    }
}
#[derive(SystemParam)]
pub struct HostsObservation<'w, 's> {
    capture: Res<'w, HostsCapture>,
    editor: Res<'w, DnsHostsEditorState>,
    cards: Query<'w, 's, Entity, With<HostsModalCard>>,
    rows: Query<'w, 's, Entity, With<HostRowIdentity>>,
    list: Query<'w, 's, Entity, With<HostsRows>>,
    address: Query<'w, 's, Entity, With<DnsHostsEditorField>>,
    domain: Query<'w, 's, Entity, With<DnsHostsDomainField>>,
    actions: Query<'w, 's, (Entity, &'static HostAction)>,
}
pub fn observe(
    probe: HostsObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        let editor = &probe.editor.editor;
        if probe.capture.stage != 3
            || !editor.open
            || editor.pending.is_some()
            || !editor.can_apply()
            || !editor
                .failure
                .as_ref()
                .is_some_and(|failure| failure.code == ErrorCode::Permission)
            || probe.capture.store.writes.load(Ordering::SeqCst) != 1
            || probe.capture.store.content() != ORIGINAL_HOSTS
            || editor.rows.len() != 3
            || probe.rows.iter().count() != 3
            || !editor.applied.as_ref().is_some_and(|profile| {
                profile.entries.len() == 2 && profile.legacy_entries.len() == 1
            })
        {
            return None;
        }
        for row in probe.rows.iter() {
            geometry.bounds(row, "Hosts draft row")?;
        }
        geometry.bounds(probe.list.iter().next()?, "Hosts row list")?;
        geometry.bounds(probe.address.iter().next()?, "Hosts address input")?;
        geometry.bounds(probe.domain.iter().next()?, "Hosts domain input")?;
        let apply = probe
            .actions
            .iter()
            .find(|(_, action)| matches!(action, HostAction::Apply))?
            .0;
        geometry.bounds(apply, "Hosts retry action")?;
        geometry.bounds(probe.cards.iter().next()?, "Hosts editor modal")
    })();
    observation.publish(&mut feature, bounds);
}
