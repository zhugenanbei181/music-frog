//! Read real telemetry through its injected port, then observe native stale-zero texts and geometry.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::overview::{
    OverviewLine, OverviewLineKind, OverviewPageRoot, OverviewTrafficCard,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::LogCaptureProcess;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::telemetry_observation_fixtures::{
    TelemetryObservationReader, stale_zero_snapshot,
};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use std::sync::{Arc, Mutex};

#[derive(Resource)]
struct TelemetryCapture {
    core: Arc<CoreApplication>,
    reader: Arc<ApplicationSurfaceReader>,
    telemetry: Arc<TelemetryObservationReader>,
    runtime: Arc<dyn ApplicationRuntime>,
    initialized: bool,
}
pub fn install(app: &mut App) {
    let runtime = tokio_application_runtime().expect("isolated telemetry executor");
    let process = Arc::new(LogCaptureProcess::default());
    let telemetry = Arc::new(TelemetryObservationReader::default());
    let core = Arc::new(CoreApplication::new_with_overview(
        process.clone(),
        process,
        telemetry.clone(),
        runtime.clone(),
    ));
    let reader = Arc::new(ApplicationSurfaceReader::new(
        core.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    ));
    app.insert_resource(TelemetryCapture {
        core,
        reader,
        telemetry,
        runtime,
        initialized: false,
    });
}
#[derive(SystemParam)]
pub struct TelemetryControls<'w, 's> {
    capture: ResMut<'w, TelemetryCapture>,
    route: Res<'w, ActiveRoute>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    lines: Query<'w, 's, (Entity, &'static OverviewLine, &'static Text)>,
    cards: Query<'w, 's, Entity, With<OverviewTrafficCard>>,
    scroll: Query<'w, 's, Entity, With<OverviewPageRoot>>,
}
pub fn activate(
    mut controls: TelemetryControls,
    feature: Res<InteractionCapture>,
    geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::RuntimeTelemetryObservation,
        Route::Overview,
    ) {
        return;
    }
    if !controls.capture.initialized {
        let core = controls.capture.core.clone();
        let reader = controls.capture.reader.clone();
        let telemetry = controls.capture.telemetry.clone();
        let result = Arc::new(Mutex::new(None));
        let output = result.clone();
        controls.capture.runtime.block_on(Box::pin(async move {
            *output.lock().unwrap() = Some(
                stale_zero_snapshot(&core, &reader, &telemetry)
                    .await
                    .expect("actual telemetry transitions"),
            );
        }));
        let mut snapshot = result
            .lock()
            .unwrap()
            .take()
            .expect("actual telemetry facts");
        snapshot.revision = controls.latest.0.revision + 1;
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
        controls.capture.initialized = true;
    }
    if let (Ok(card), Ok(scroll)) = (controls.cards.single(), controls.scroll.single())
        && let (Some(bounds), Some(viewport)) = (geometry.rect(card), geometry.rect(scroll))
    {
        request_scroll(&mut commands, scroll, bounds, viewport);
    }
}
pub fn observe(
    controls: TelemetryControls,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::RuntimeTelemetryObservation,
        Route::Overview,
    ) {
        return;
    }
    let bounds = (|| {
        for (kind, expected) in [
            (OverviewLineKind::Upload, "↑ 0 B/s (stale)"),
            (OverviewLineKind::Download, "↓ 0 B/s (stale)"),
        ] {
            let (entity, _, text) = controls.lines.iter().find(|(_, line, _)| line.0 == kind)?;
            if text.0 != expected {
                return None;
            }
            geometry.bounds(entity, "telemetry-stale-zero-rate")?;
        }
        let (entity, _, text) = controls
            .lines
            .iter()
            .find(|(_, line, _)| line.0 == OverviewLineKind::TelemetryFailure)?;
        if !text.0.contains("isolated telemetry read denied") {
            return None;
        }
        geometry.bounds(entity, "telemetry-read-failure")?;
        geometry.bounds(controls.cards.single().ok()?, "telemetry-observation-card")
    })();
    observed.publish(&mut feature, bounds);
}
