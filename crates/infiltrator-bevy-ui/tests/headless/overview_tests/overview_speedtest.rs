//! Behavior cases for overview speedtest.
//! test-intent: behavior

use super::*;
use crate::native_input::{click_entity, type_text};
use bevy::ecs::query::With;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::pages::overview::OverviewProjectionUpdated;
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestButton, OverviewSpeedtestConcurrencyStep, OverviewSpeedtestConcurrencyText,
    OverviewSpeedtestDeadText, OverviewSpeedtestDetailBodyText, OverviewSpeedtestDetailButton,
    OverviewSpeedtestEgressText, OverviewSpeedtestHistoryText, OverviewSpeedtestMetricsText,
    OverviewSpeedtestText,
};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::adaptive_modal::ModalState;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::speedtest::{SpeedtestPhase, SpeedtestSnapshot};
use infiltrator_contract::speedtest_matrix::SpeedtestRegressionMatrixReport;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

/// Explicit empty engine input, independent of the populated demo fixture.
struct EmptySpeedtestSource;
impl OverviewSource for EmptySpeedtestSource {
    fn current(&self) -> OverviewProjection {
        let mut projection = DemoOverviewSource::running().current();
        projection.speedtest = SpeedtestSnapshot::default();
        projection
    }
}

#[test]
fn overview_speedtest_button_submits_test_all_proxy_groups() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewSpeedtestButton)>();
        buttons
            .iter(world)
            .find(|(_, btn)| !btn.testing)
            .expect("speedtest button mounted")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    assert!(sink.submitted().contains(&UiCommand::TestAllProxyGroups));
}

#[test]
fn overview_speedtest_button_reflects_shared_engine_phase() {
    // The button and its caption must follow the shared engine snapshot, so a
    // running batch reads "测速中 n/m" on both surfaces instead of a static label.
    let mut app = mounted_default();

    let read_caption = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest caption mounted")
    };
    let read_testing = |app: &mut App| -> bool {
        let world = app.world_mut();
        let mut buttons = world.query::<&OverviewSpeedtestButton>();
        buttons
            .iter(world)
            .next()
            .map(|b| b.testing)
            .unwrap_or(false)
    };

    // Idle: default caption, not testing.
    assert_eq!(read_caption(&mut app), "一键测速");
    assert!(!read_testing(&mut app));

    // Running snapshot with progress must flip both label and marker.
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest.phase = SpeedtestPhase::ProbingLatency;
    projection.speedtest.progress.completed_nodes = 12;
    projection.speedtest.progress.total_nodes = 30;
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    assert_eq!(
        read_caption(&mut app),
        interpolate(
            Lang("zh-CN").tr("speedtest_cancel_progress").as_ref(),
            &[("done", "12"), ("total", "30")]
        )
    );
    assert!(read_testing(&mut app));
}

#[test]
fn overview_speedtest_metrics_follow_shared_engine() {
    // Jitter / packet-loss / star / bandwidth of the fastest node must come
    // from the same shared snapshot Iced renders, never a Bevy-local value.
    let mut app = mounted_app_with(EmptySpeedtestSource);

    let read_metrics = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestMetricsText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest metrics caption mounted")
    };
    let read_dead = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestDeadText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest dead archive caption mounted")
    };

    // Idle: honest placeholder, no fabricated numbers.
    assert_eq!(read_metrics(&mut app), "—");
    assert_eq!(read_dead(&mut app), "—");

    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::demo_fixture();
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let metrics = read_metrics(&mut app);
    assert!(metrics.contains("抖动"), "metrics={metrics}");
    assert!(metrics.contains("Mbps"), "metrics={metrics}");
    assert!(metrics.contains('★'), "metrics={metrics}");

    // The demo fixture carries exactly one dead node; it must be archived
    // honestly rather than hidden.
    let dead = read_dead(&mut app);
    assert!(dead.contains("超时归档 1"), "dead={dead}");
    assert!(dead.contains("超时不可用节点"), "dead={dead}");
}

#[test]
fn overview_speedtest_history_follows_shared_engine() {
    // The persisted run history must render from the same shared snapshot
    // Iced reads (`recent_history`), never a Bevy-local record.
    let mut app = mounted_app_with(EmptySpeedtestSource);

    let read_history = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestHistoryText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest history caption mounted")
    };

    // Idle: honest placeholder, no fabricated run.
    assert_eq!(read_history(&mut app), "—");

    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::demo_fixture();
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let history = read_history(&mut app);
    assert!(history.contains("最近测速"), "history={history}");
    assert!(history.contains("全部节点"), "history={history}");
    assert!(history.contains("平均带宽 154.8 Mbps"), "history={history}");
    assert!(history.contains("★★★★★"), "history={history}");
}

#[test]
fn overview_speedtest_running_button_submits_cancel() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest.phase = SpeedtestPhase::ProbingLatency;
    projection.speedtest.progress.completed_nodes = 1;
    projection.speedtest.progress.total_nodes = 4;
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewSpeedtestButton)>();
        buttons
            .iter(world)
            .find(|(_, btn)| btn.testing)
            .expect("running speedtest button mounted")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(sink.submitted().contains(&UiCommand::CancelSpeedtest));
}

#[test]
fn overview_speedtest_typed_url_reaches_the_shared_intent() {
    // DUAL-06-03: the URL typed in the Overview field must ride into the
    // shared `TestDelay { url: Some(..) }` intent, not a Bevy-local target.
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

    let custom_url = "https://cp.cloudflare.com/generate_204";
    let field = native_url_field(app.world_mut());
    click_entity(&mut app, field);
    type_text(&mut app, custom_url);
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        custom_url
    );
    assert!(
        sink.submitted().is_empty(),
        "editing does not submit a measurement"
    );

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewSpeedtestButton)>();
        buttons
            .iter(world)
            .find(|(_, btn)| !btn.testing)
            .expect("speedtest button mounted")
            .0
    };
    click_entity(&mut app, button);

    assert!(
        sink.submitted()
            .contains(&UiCommand::TestAllProxyGroupsWithUrl {
                url: custom_url.to_owned(),
            }),
        "submitted={:?}",
        sink.submitted()
    );
    // The typed URL reaches the shared delay intent verbatim.
    assert_eq!(
        UiCommand::TestAllProxyGroupsWithUrl {
            url: custom_url.to_owned(),
        }
        .to_intent(),
        Some(CommandIntent::TestDelay {
            group: None,
            url: Some(custom_url.to_owned()),
            timeout_ms: None,
        })
    );
}

#[test]
fn overview_speedtest_concurrency_stepper_submits_shared_intent() {
    // DUAL-06-01: the +/- stepper reads the live bound from the shared
    // snapshot and submits the shared concurrency intent; the UI owns no
    // concurrency fact of its own.
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let step_entity = |app: &mut App, delta: i64| -> Entity {
        let world = app.world_mut();
        let mut steps = world.query::<(Entity, &OverviewSpeedtestConcurrencyStep)>();
        steps
            .iter(world)
            .find(|(_, step)| step.0 == delta)
            .expect("concurrency step mounted")
            .0
    };

    // The demo fixture bound is 30; +5 -> 35.
    let up = step_entity(&mut app, 5);
    app.world_mut().commands().trigger(Activate { entity: up });
    app.update();
    assert!(
        sink.submitted()
            .contains(&UiCommand::SetSpeedtestConcurrency { limit: 35 }),
        "submitted={:?}",
        sink.submitted()
    );

    // Clamp: a shared bound of 2 stepped down by 5 stays at 1 (never zero).
    app.world_mut()
        .resource_mut::<LatestSurfaceSnapshot>()
        .0
        .speedtest
        .config
        .concurrency = 2;
    let down = step_entity(&mut app, -5);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: down });
    app.update();
    assert!(
        sink.submitted()
            .contains(&UiCommand::SetSpeedtestConcurrency { limit: 1 }),
        "submitted={:?}",
        sink.submitted()
    );
}

#[test]
fn overview_speedtest_concurrency_text_follows_shared_engine() {
    // The concurrency caption is restamped from the shared snapshot's
    // `config.concurrency`, never a Bevy-local constant.
    let mut app = mounted_default();

    let read = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestConcurrencyText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("concurrency caption mounted")
    };

    assert_eq!(read(&mut app), "30");

    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest.config.concurrency = 12;
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    assert_eq!(read(&mut app), "12");
}

#[test]
fn overview_speedtest_egress_and_detail_modal_follow_shared_engine() {
    // DUAL-06-12/13/14: the egress comparison caption and the detail modal
    // must both restamp from the one shared snapshot Iced reads.
    let mut app = mounted_default();

    let read_egress = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestEgressText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest egress caption mounted")
    };
    let read_body = |app: &mut App| -> String {
        let world = app.world_mut();
        let mut texts = world.query_filtered::<&Text, With<OverviewSpeedtestDetailBodyText>>();
        texts
            .iter(world)
            .next()
            .map(|t| t.0.clone())
            .expect("speedtest detail body mounted")
    };

    // Honest empty state: no fabricated node, no fabricated egress.
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::default();
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    assert_eq!(read_egress(&mut app), "—");
    assert!(read_body(&mut app).contains("暂无测速结果"));

    // A HK-labelled node reporting a US egress is an honest mismatch.
    let mut projection = DemoOverviewSource::running().current();
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    if let Some(node) = snapshot.node_results.get_mut("💀 超时不可用节点 01") {
        node.is_alive = true;
        node.delay_ms = Some(20);
        node.star_rating = 2;
        node.label_country = Some("HK".to_owned());
        node.outbound_ip = Some("45.32.1.9".to_owned());
        node.outbound_country = Some("US".to_owned());
    }
    projection.speedtest = snapshot;
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();

    let egress = read_egress(&mut app);
    assert!(egress.contains("45.32.1.9 (US)"), "egress={egress}");
    assert!(egress.contains("归属不一致"), "egress={egress}");

    let body = read_body(&mut app);
    assert!(body.contains("出口"), "body={body}");
    assert!(body.contains("归属不一致"), "body={body}");
    assert!(body.contains("20 ms"), "body={body}");

    // The modal is closed by default and opens from its shared-intent button.
    assert!(!app.world().resource::<ModalState>().is_open);
    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewSpeedtestDetailButton)>();
        buttons
            .iter(world)
            .next()
            .expect("speedtest detail button mounted")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    assert!(app.world().resource::<ModalState>().is_open);

    // DUAL-06-14: both surfaces are driven by the one shared matrix report.
    let report = SpeedtestRegressionMatrixReport::run_deterministic_matrix();
    assert!(report.is_all_passed());
    assert_eq!(report.total_scenarios, 15);
}
