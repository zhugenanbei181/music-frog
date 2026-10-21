//! Behavior cases for sidebar.
//! test-intent: behavior

use super::*;
use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::query::With;
use bevy::ui::prelude::BackgroundColor;
use infiltrator_bevy_ui::app::SidebarSystemProxyCard;
use infiltrator_bevy_widgets::switch::{SwitchKnob, SwitchPosition};
use infiltrator_contract::system_proxy::{SystemProxyObservation, SystemProxySnapshot};
use infiltrator_contract::system_toggle::SystemToggleState;

/// The switch's published state: the knob's typed rest position and the
/// track's fill.
///
/// The sidebar's toggles are the shared switch control, so the switch states
/// its own truth: the owner's projection drives the knob and the track, and
/// the card carries no second on/off flag of its own.
fn switch_state(app: &App, toggle: Entity) -> (SwitchPosition, Color) {
    let knob = app
        .world()
        .get::<Children>(toggle)
        .expect("the switch track owns its knob")
        .iter()
        .find_map(|child| app.world().get::<SwitchKnob>(*child).copied())
        .expect("the switch track owns a knob");
    let fill = app
        .world()
        .get::<BackgroundColor>(toggle)
        .expect("the switch track owns its fill")
        .0;
    (knob.0, fill)
}

/// The switch's published accessibility state.
fn switch_a11y(app: &App, toggle: Entity) -> Option<accesskit::Toggled> {
    app.world()
        .get::<AccessibilityNode>(toggle)
        .expect("the switch publishes its state")
        .0
        .toggled()
}

#[test]
fn test_sidebar_system_toggles_use_shared_projection_and_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));

    let proxy = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarSystemProxyToggle>>()
        .single(app.world())
        .expect("sidebar system proxy toggle");
    let tun = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarTunToggle>>()
        .single(app.world())
        .expect("sidebar TUN toggle");

    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Enabled
    ));

    // The switch publishes the owner's state: the knob rests on and the
    // accessibility row publishes the same truth.
    let (proxy_position, proxy_on_fill) = switch_state(&app, proxy);
    assert_eq!(proxy_position, SwitchPosition::On);
    assert_eq!(switch_a11y(&app, proxy), Some(accesskit::Toggled::True));

    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSystemProxy { enabled: false }]
    );
    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Pending { desired: false }
    ));

    // A second activation while the application command is in flight is
    // fenced instead of producing a duplicate command.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(sink.submitted().len(), 1);

    let mut next = DemoSurfaceSource::running().surface_snapshot();
    next.revision = 2;
    next.system_proxy = SystemProxySnapshot::from_observation(
        2,
        SystemProxyObservation {
            enabled: false,
            endpoint: None,
            bypass: None,
        },
    );
    next.runtime_control.tun_enabled = Some(false);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(next));
    app.update();

    // The owner's readback drives the switch: the knob rests off, the track
    // drops the accent and the accessibility row publishes the off state.
    let (proxy_position, proxy_off_fill) = switch_state(&app, proxy);
    assert_eq!(proxy_position, SwitchPosition::Off);
    assert_ne!(proxy_off_fill, proxy_on_fill);
    assert_eq!(switch_a11y(&app, proxy), Some(accesskit::Toggled::False));
    let (tun_position, tun_fill) = switch_state(&app, tun);
    assert_eq!(tun_position, SwitchPosition::Off);
    assert_ne!(tun_fill, proxy_on_fill);
    // The card names its own toggle and spends no pill on the word "On".
    let card = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarSystemProxyCard>>()
        .single(app.world())
        .expect("the system proxy toggle card");
    assert!(subtree_has_text(app.world(), card, "系统代理"));
    assert!(
        !subtree_has_text(app.world(), card, "关"),
        "the card states the state with the switch, not with a word"
    );
    assert_eq!(
        app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Disabled
    );

    app.world_mut().commands().trigger(Activate { entity: tun });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetSystemProxy { enabled: false },
            UiCommand::ToggleTun { enabled: true },
        ]
    );
}
