//! Scoped actual-reader activation; independent confirmation pixels precede any write.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_draft::RulesDraftState;
use crate::pages::rules_statistics::{
    RulesStatisticsState, StatisticsCard, StatisticsConfirmationCard, StatisticsControl,
    StatisticsLine,
};
use crate::route::{ActiveRoute, Route, replay_surface_snapshot};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::rule_statistics_workbench::StatisticsTab;
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[derive(Resource)]
pub struct StatisticsCapture {
    store: Arc<RuleTraceStore>,
    snapshot: Option<SurfaceSnapshot>,
    stage: u8,
}
pub fn install(app: &mut App) {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated statistics runtime"),
    ));
    let reader = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_rule_tracer(owner.clone())
        .with_configuration(ConfigurationApplication::new(store.clone()));
    let observed = Arc::new(Mutex::new(None));
    let result = observed.clone();
    tokio_application_runtime()
        .expect("isolated statistics read")
        .block_on(Box::pin(async move {
            owner
                .simulate(
                    RuleTraceOperationId(1),
                    RuleTraceRequest {
                        query: "special.com".into(),
                        context: TrafficContextSnapshot {
                            src_ip: Some("192.0.2.1".into()),
                            ..Default::default()
                        },
                    },
                    None,
                )
                .await
                .expect("actual statistics");
            *result.lock().expect("capture reader") =
                Some(reader.read().await.expect("actual statistics page"));
        }));
    app.insert_resource(StatisticsCapture {
        store,
        snapshot: observed.lock().expect("capture read").take(),
        stage: 0,
    });
}
#[derive(SystemParam)]
pub struct StatisticsActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, StatisticsCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    statistics: Res<'w, RulesStatisticsState>,
    draft: Res<'w, RulesDraftState>,
    controls: Query<
        'w,
        's,
        (
            Entity,
            &'static StatisticsControl,
            &'static ButtonDisabled,
            Has<ModalScrim>,
        ),
    >,
    cards: Query<'w, 's, Entity, With<StatisticsCard>>,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
}
pub fn activate(
    mut probe: StatisticsActivation,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &probe.feature,
        &probe.route,
        FeatureId::RulesStatisticsInspector,
        Route::Rules,
    ) {
        return;
    }
    if let Some(mut snapshot) = probe.capture.snapshot.take() {
        snapshot.origin = SurfaceOrigin::Demo;
        snapshot.revision = probe.latest.0.revision + 1;
        probe.latest.0 = snapshot;
        replay_surface_snapshot(probe.latest.0.clone(), &mut commands);
        return;
    }
    if !probe.statistics.model.current() || !probe.draft.model.editable() {
        return;
    }
    let wanted = match probe.capture.stage {
        0 => StatisticsControl::Tab(StatisticsTab::Inactive),
        1 if probe.statistics.model.tab == StatisticsTab::Inactive => StatisticsControl::Inspect,
        2 if probe.statistics.model.inspected => StatisticsControl::PrepareCleanup,
        _ => return,
    };
    let Some((entity, _, _, _)) = probe.controls.iter().find(|(_, action, disabled, scrim)| {
        !disabled.0
            && !*scrim
            && match (*action, wanted) {
                (StatisticsControl::Tab(a), StatisticsControl::Tab(b)) => *a == b,
                (StatisticsControl::Inspect, StatisticsControl::Inspect)
                | (StatisticsControl::PrepareCleanup, StatisticsControl::PrepareCleanup) => true,
                _ => false,
            }
    }) else {
        return;
    };
    let (Ok(card), Ok(page)) = (probe.cards.single(), probe.pages.single()) else {
        return;
    };
    if let (Some(bounds), Some(viewport)) = (geometry.rect(card), geometry.rect(page)) {
        request_scroll(&mut commands, page, bounds, viewport);
    }
    if geometry
        .bounds(entity, "statistics-native-action")
        .is_some()
    {
        commands.trigger(Activate { entity });
        probe.capture.stage += 1;
    }
}
#[derive(SystemParam)]
pub struct StatisticsObservation<'w, 's> {
    capture: Res<'w, StatisticsCapture>,
    statistics: Res<'w, RulesStatisticsState>,
    draft: Res<'w, RulesDraftState>,
    cards: Query<'w, 's, Entity, With<StatisticsConfirmationCard>>,
    copy: Query<'w, 's, (Entity, &'static StatisticsLine, &'static Text)>,
    controls: Query<
        'w,
        's,
        (
            Entity,
            &'static StatisticsControl,
            &'static ButtonDisabled,
            Has<ModalScrim>,
        ),
    >,
}
pub fn observe(
    probe: StatisticsObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if feature.feature != FeatureId::RulesStatisticsInspector
        || probe.capture.stage != 3
        || probe.draft.model.dirty()
        || !probe
            .statistics
            .model
            .can_confirm_cleanup(&probe.draft.model)
        || probe.capture.store.writes.load(Ordering::SeqCst) != 0
    {
        return;
    }
    let Ok(card) = probe.cards.single() else {
        return;
    };
    let Some(bounds) = geometry.bounds(card, "statistics-cleanup-review") else {
        return;
    };
    let Some((entity, _, copy)) = probe
        .copy
        .iter()
        .find(|(_, line, _)| matches!(line, StatisticsLine::Confirmation))
    else {
        return;
    };
    if !copy.0.contains("Profile: trace.yaml · 3 rows")
        || !copy.0.contains("DOMAIN-SUFFIX,google.com,PROXY")
        || geometry
            .bounds(entity, "statistics-qualified-targets")
            .is_none()
    {
        return;
    }
    for wanted in [
        StatisticsControl::CancelCleanup,
        StatisticsControl::ConfirmCleanup,
    ] {
        let Some((entity, _, _, _)) = probe.controls.iter().find(|(_, action, disabled, scrim)| {
            !disabled.0
                && !*scrim
                && matches!(
                    (*action, wanted),
                    (
                        StatisticsControl::CancelCleanup,
                        StatisticsControl::CancelCleanup
                    ) | (
                        StatisticsControl::ConfirmCleanup,
                        StatisticsControl::ConfirmCleanup
                    )
                )
        }) else {
            return;
        };
        if geometry
            .bounds(entity, "statistics-review-action")
            .is_none()
        {
            return;
        }
    }
    feature.activated = true;
    observed.0 = Some(bounds);
}
