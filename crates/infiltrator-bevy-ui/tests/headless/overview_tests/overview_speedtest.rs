//! Typed speedtest commands and readback on Overview.
//! test-intent: behavior

use super::*;
use crate::native_input::click_entity;
use bevy::ecs::query::With;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::pages::overview::OverviewProjectionUpdated;
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestButton, OverviewSpeedtestConcurrencyStep, OverviewSpeedtestConcurrencyText,
    OverviewSpeedtestDeadText, OverviewSpeedtestMetricsText, OverviewSpeedtestText,
    SpeedtestButtonPhase,
};
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::speedtest::{
    SpeedtestConcurrency, SpeedtestProgress, SpeedtestRunId, SpeedtestRunState, SpeedtestSnapshot,
};
use std::sync::Arc;

struct EmptySpeedtestSource;

impl OverviewSource for EmptySpeedtestSource {
    fn current(&self) -> OverviewProjection {
        let mut projection = DemoOverviewSource::running().current();
        projection.speedtest = SpeedtestSnapshot::default();
        projection
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }
}

fn button_entity(app: &mut App, phase: SpeedtestButtonPhase) -> Entity {
    let world = app.world_mut();
    let mut buttons = world.query::<(Entity, &OverviewSpeedtestButton)>();
    buttons
        .iter(world)
        .find(|(_, button)| button.phase == phase)
        .map(|(entity, _)| entity)
        .expect("speedtest button mounted in requested phase")
}

fn set_running_speedtest(
    projection: &mut OverviewProjection,
    completed_targets: usize,
    total_targets: usize,
) -> SpeedtestRunId {
    let snapshot = SpeedtestSnapshot::demo_fixture();
    let run_id = snapshot.state.run_id().expect("demo run ID");
    let target = snapshot.state.target().expect("demo target").clone();
    projection.speedtest = SpeedtestSnapshot {
        state: SpeedtestRunState::Probing {
            run_id,
            target,
            progress: SpeedtestProgress {
                completed_targets,
                total_targets,
            },
        },
        ..snapshot
    };
    run_id
}

fn marked_text<T: Component>(app: &mut App) -> String {
    let world = app.world_mut();
    let mut texts = world.query_filtered::<&Text, With<T>>();
    texts
        .single(world)
        .expect("marked speedtest text mounted")
        .0
        .clone()
}

#[test]
fn overview_start_intent_carries_core_scope_inventory_revision_and_typed_scope() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_default();
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.add_plugins(ButtonPlugin);
    app.update();

    let observed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let inventory = observed
        .outbound_inventory
        .data
        .as_ref()
        .expect("demo surface has a typed outbound inventory");
    let button = button_entity(&mut app, SpeedtestButtonPhase::Ready);
    click_entity(&mut app, button);

    let submitted = sink.submitted();
    let [UiCommand::StartSpeedtest { target }] = submitted.as_slice() else {
        panic!("expected a typed speedtest target, got {submitted:?}");
    };
    assert_eq!(target.core_scope(), &observed.core.scope().unwrap());
    assert_eq!(target.expected_inventory_revision(), inventory.revision());
    assert_eq!(target.outbound_scope(), &OutboundProbeBatchScope::All);
    assert_eq!(target.rounds().get(), 5);
}

#[test]
fn overview_progress_metrics_and_timeout_archive_follow_the_shared_snapshot() {
    let mut app = mounted_app_with(EmptySpeedtestSource);
    assert_eq!(marked_text::<OverviewSpeedtestMetricsText>(&mut app), "—");
    assert_eq!(marked_text::<OverviewSpeedtestDeadText>(&mut app), "—");

    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::demo_fixture();
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    let metrics = marked_text::<OverviewSpeedtestMetricsText>(&mut app);
    assert!(metrics.contains("Jitter"), "metrics={metrics}");
    assert!(
        !metrics.contains("Mbps"),
        "speedtest no longer claims bandwidth"
    );
    let dead = marked_text::<OverviewSpeedtestDeadText>(&mut app);
    assert!(dead.contains("超时归档 1"), "dead={dead}");
    assert!(dead.contains("超时不可用节点"), "dead={dead}");

    set_running_speedtest(&mut projection, 2, 3);
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    assert_eq!(
        app.world()
            .query::<&OverviewSpeedtestButton>()
            .single(app.world())
            .unwrap()
            .phase,
        SpeedtestButtonPhase::Running
    );
    let caption = marked_text::<OverviewSpeedtestText>(&mut app);
    assert!(caption.contains("2") && caption.contains("3"));
}

#[test]
fn running_overview_button_submits_shared_cancel_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.add_plugins(ButtonPlugin);
    app.update();

    let mut projection = DemoOverviewSource::running().current();
    let run_id = set_running_speedtest(&mut projection, 1, 4);
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let button = button_entity(&mut app, SpeedtestButtonPhase::Running);
    click_entity(&mut app, button);
    assert!(
        sink.submitted()
            .contains(&UiCommand::CancelSpeedtestRun { run_id })
    );
}

#[test]
fn concurrency_stepper_submits_a_bounded_typed_value() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.add_plugins(ButtonPlugin);
    app.update();

    let up = {
        let world = app.world_mut();
        let mut steps = world.query::<(Entity, &OverviewSpeedtestConcurrencyStep)>();
        steps
            .iter(world)
            .find(|(_, step)| step.0 == 5)
            .expect("increase step mounted")
            .0
    };
    click_entity(&mut app, up);
    assert!(
        sink.submitted()
            .contains(&UiCommand::SetSpeedtestConcurrency {
                concurrency: SpeedtestConcurrency::new(30).unwrap(),
            })
    );

    app.world_mut()
        .resource_mut::<LatestSurfaceSnapshot>()
        .0
        .speedtest
        .concurrency = SpeedtestConcurrency::new(2).unwrap();
    let down = {
        let world = app.world_mut();
        let mut steps = world.query::<(Entity, &OverviewSpeedtestConcurrencyStep)>();
        steps
            .iter(world)
            .find(|(_, step)| step.0 == -5)
            .expect("decrease step mounted")
            .0
    };
    click_entity(&mut app, down);
    assert!(
        sink.submitted()
            .contains(&UiCommand::SetSpeedtestConcurrency {
                concurrency: SpeedtestConcurrency::new(1).unwrap(),
            })
    );

    let _ = marked_text::<OverviewSpeedtestConcurrencyText>(&mut app);
}
