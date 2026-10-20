//! test-intent: behavior
//! Native locale replay must preserve the result, active modal and input owner.
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::app::{ShellPlugin, SidebarSystemProxyToggle, SidebarTunToggle};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink};
use infiltrator_bevy_ui::pages::overview::{LastOverviewProjection, OverviewProjectionUpdated};
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestDetailBodyText, OverviewSpeedtestDetailButton, OverviewSpeedtestHistoryText,
    OverviewSpeedtestMetricsText,
};
use infiltrator_bevy_ui::projection::{DemoOverviewSource, OverviewSource};
use infiltrator_bevy_ui::route::PagesPlugin;
use infiltrator_bevy_widgets::adaptive_modal::ModalState;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalStage;
use infiltrator_contract::speedtest::SpeedtestSampleOutcome;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use std::sync::Arc;

fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}
fn text(app: &App, entity: Entity) -> &str {
    &app.world().get::<Text>(entity).unwrap().0
}

#[test]
fn locale_replay_keeps_speedtest_results_partial_facts_and_modal() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
        CommandPumpPlugin::new(sink.clone()),
    ));
    app.update();
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::demo_fixture();
    let mut result = projection.speedtest.results.remove(0);
    result.display_name = Some("node {stars}/中文🙂".into());
    result.samples = vec![SpeedtestSampleOutcome::Measured { delay_ms: 0 }];
    projection.speedtest.results = vec![result];
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    let metrics = entity::<OverviewSpeedtestMetricsText>(&mut app);
    let history = entity::<OverviewSpeedtestHistoryText>(&mut app);
    let detail = entity::<OverviewSpeedtestDetailBodyText>(&mut app);
    let detail_button = entity::<OverviewSpeedtestDetailButton>(&mut app);
    click_entity(&mut app, detail_button);
    assert_eq!(
        app.world().resource::<ModalState>().stage,
        ModalStage::Open
    );
    let proxy = entity::<SidebarSystemProxyToggle>(&mut app);
    let tun = entity::<SidebarTunToggle>(&mut app);
    let proxy_label = *app.world().get::<Children>(proxy).unwrap().first().unwrap();
    let tun_label = *app.world().get::<Children>(tun).unwrap().first().unwrap();
    assert_eq!(text(&app, proxy_label), "开");
    assert_eq!(text(&app, tun_label), "开");
    assert!(text(&app, metrics).contains("抖动 — · 丢包 0.0%"));
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(entity::<SidebarSystemProxyToggle>(&mut app), proxy);
    assert_eq!(entity::<SidebarTunToggle>(&mut app), tun);
    assert_eq!(text(&app, proxy_label), "On");
    assert_eq!(text(&app, tun_label), "On");
    assert_eq!(entity::<OverviewSpeedtestMetricsText>(&mut app), metrics);
    assert_eq!(entity::<OverviewSpeedtestDetailBodyText>(&mut app), detail);
    assert_eq!(
        app.world().resource::<ModalState>().stage,
        ModalStage::Open
    );
    assert!(text(&app, metrics).starts_with("node {stars}/中文🙂"));
    assert!(text(&app, metrics).contains("Jitter — · Loss 0.0%"));
    assert!(text(&app, history).starts_with("Recent tests:"));
    assert!(text(&app, detail).contains("node {stars}/中文🙂 · Shadowsocks"));
    assert!(text(&app, detail).contains("0 ms"));
    assert_eq!(
        app.world()
            .resource::<LastOverviewProjection>()
            .0
            .as_ref()
            .unwrap()
            .speedtest,
        projection.speedtest
    );
    assert!(sink.submitted().is_empty());
}
