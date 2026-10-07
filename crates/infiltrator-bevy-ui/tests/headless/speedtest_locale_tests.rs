//! test-intent: behavior
//! Native locale replay must preserve the result, active modal and input owner.
use crate::native_input::{click_entity, press, replace_text};
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
    OverviewSpeedtestMetricsText, OverviewSpeedtestUrlField,
};
use infiltrator_bevy_ui::projection::{DemoOverviewSource, OverviewSource};
use infiltrator_bevy_ui::route::PagesPlugin;
use infiltrator_bevy_widgets::adaptive_modal::ModalState;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
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
fn locale_replay_keeps_speedtest_results_partial_facts_modal_and_native_draft() {
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
    let mut node = projection.speedtest.fastest_node().unwrap().clone();
    node.node_name = "node {stars}/中文🙂".into();
    node.bandwidth_mbps = Some(0.0);
    node.jitter = None;
    projection.speedtest.node_results.clear();
    projection
        .speedtest
        .node_results
        .insert(node.node_name.clone(), node);
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    let metrics = entity::<OverviewSpeedtestMetricsText>(&mut app);
    let history = entity::<OverviewSpeedtestHistoryText>(&mut app);
    let detail = entity::<OverviewSpeedtestDetailBodyText>(&mut app);
    let detail_button = entity::<OverviewSpeedtestDetailButton>(&mut app);
    click_entity(&mut app, detail_button);
    assert!(app.world().resource::<ModalState>().is_open);
    let proxy = entity::<SidebarSystemProxyToggle>(&mut app);
    let tun = entity::<SidebarTunToggle>(&mut app);
    let proxy_label = *app.world().get::<Children>(proxy).unwrap().first().unwrap();
    let tun_label = *app.world().get::<Children>(tun).unwrap().first().unwrap();
    assert_eq!(text(&app, proxy_label), "开");
    assert_eq!(text(&app, tun_label), "开");
    assert!(text(&app, metrics).contains("带宽 0.0 Mbps · 抖动 — · 丢包 — · — · —"));
    let url_owner = entity::<OverviewSpeedtestUrlField>(&mut app);
    let field = app
        .world()
        .get::<Children>(url_owner)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap();
    press(&mut app, field);
    let draft = "https://test.example/{locale}/中文🙂";
    replace_text(&mut app, draft);
    let mut expected = app.world().get::<TextField>(field).unwrap().0.clone();
    expected.set_placeholder("Speedtest URL (leave blank to use the default)");
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
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), draft);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert!(app.world().resource::<ModalState>().is_open);
    assert!(text(&app, metrics).starts_with("node {stars}/中文🙂 · Bandwidth 0.0 Mbps"));
    assert!(text(&app, metrics).contains("Jitter — · Loss — · — · —"));
    assert!(text(&app, history).starts_with("Recent tests:"));
    assert!(text(&app, detail).contains("Stars —"));
    assert!(!text(&app, detail).contains('★'));
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
