//! test-intent: behavior
//! Actual native service controls follow the shared observations, including unavailable values.
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::pages::settings::SettingsProjectionUpdated;
use infiltrator_bevy_ui::pages::settings::settings_core::SettingsProjection;
use infiltrator_bevy_ui::pages::settings::settings_privileged_network::{
    PrivilegedNetworkRunButton, PrivilegedNetworkStatusLine,
};
use infiltrator_bevy_ui::pages::settings::settings_vpn::{
    VpnStartButton, VpnStatusLine, VpnStopButton,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::privileged_network::PrivilegedNetworkState;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use std::sync::Arc;

fn read_text<T: Component>(app: &mut App) -> String {
    app.world_mut()
        .query::<(&T, &Text)>()
        .single(app.world())
        .unwrap()
        .1
        .0
        .clone()
}
#[test]
fn native_host_status_keeps_unknown_mtu_and_busy_controls_and_replays_opaque_failures_on_locale_change()
 {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
        CommandPumpPlugin::new(sink.clone()),
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    let start = app
        .world_mut()
        .query::<(Entity, &VpnStartButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let stop = app
        .world_mut()
        .query::<(Entity, &VpnStopButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let run = app
        .world_mut()
        .query::<(Entity, &PrivilegedNetworkRunButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let mut projection = SettingsProjection::demo();
    projection.vpn = VpnSessionSnapshot {
        state: VpnSessionState::Running,
        ..Default::default()
    };
    projection.privileged_network.state = PrivilegedNetworkState::RollingBack;
    app.world_mut()
        .trigger(SettingsProjectionUpdated(projection.clone()));
    app.update();
    assert!(read_text::<VpnStatusLine>(&mut app).contains("MTU=未观测"));
    for entity in [start, run] {
        assert!(app.world().get::<ButtonDisabled>(entity).unwrap().0);
        assert!(app.world().get::<InteractionDisabled>(entity).is_some());
        assert!(
            app.world()
                .get::<AccessibilityNode>(entity)
                .unwrap()
                .is_disabled()
        );
        click_entity(&mut app, entity);
    }
    assert!(sink.submitted().is_empty());
    click_entity(&mut app, stop);
    assert_eq!(sink.submitted(), vec![UiCommand::StopVpn]);
    sink.clear();
    projection.vpn.state = VpnSessionState::Unsupported {
        reason: "no bridge {state} / 中文🙂".into(),
    };
    app.world_mut()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let text = read_text::<VpnStatusLine>(&mut app);
    assert!(text.starts_with("Host unsupported · no bridge {state} / 中文🙂"));
    assert!(text.contains("MTU=Not observed"));
    assert!(read_text::<PrivilegedNetworkStatusLine>(&mut app).starts_with("Rolling back"));
    for entity in [start, stop] {
        click_entity(&mut app, entity);
    }
    assert!(sink.submitted().is_empty());
}
