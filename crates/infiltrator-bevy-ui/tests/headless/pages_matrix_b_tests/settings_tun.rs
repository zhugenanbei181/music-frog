//! Behavior cases for settings tun.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::Children;

#[test]
fn test_settings_tun_route_checkboxes_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut toggles = app.world_mut().query::<(&TunRouteToggle, &Children)>();
    let sources: Vec<(TunRouteToggleKind, Entity)> = toggles
        .iter(app.world())
        .map(|(toggle, children)| {
            let source = children.iter().next().expect("route toggle checkbox");
            (toggle.0, *source)
        })
        .collect();

    for (kind, source) in sources {
        let value = match kind {
            TunRouteToggleKind::AutoRoute => false,
            TunRouteToggleKind::StrictRoute => true,
        };
        app.world_mut().commands().trigger(ValueChange {
            source,
            value,
            is_final: true,
        });
        app.update();
    }

    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetTunAutoRoute(false),
            UiCommand::SetTunStrictRoute(true)
        ]
    );
}

#[test]
fn test_settings_tun_enable_checkbox_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let source = {
        let mut toggles = app.world_mut().query::<(&TunEnableToggle, &Children)>();
        *toggles
            .single(app.world())
            .expect("TUN enable toggle")
            .1
            .iter()
            .next()
            .expect("TUN enable checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: false,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ToggleTun { enabled: false }]
    );
}

#[test]
fn test_settings_tun_stack_catalog_has_three_live_values_and_safe_lwip() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut buttons = app
        .world_mut()
        .query::<(Entity, &TunStackButton, &TunStackButtonAvailability)>();
    let entries: Vec<(Entity, TunStack, bool)> = buttons
        .iter(app.world())
        .map(|(entity, button, availability)| (entity, button.stack, availability.0))
        .collect();
    assert_eq!(entries.len(), TunStack::ALL.len());
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::Gvisor && *available)
    );
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::System && *available)
    );
    assert!(
        entries
            .iter()
            .any(|(_, stack, available)| *stack == TunStack::Mixed && *available)
    );
    let (lwip_entity, _, lwip_availability) = entries
        .iter()
        .find(|(_, stack, _)| *stack == TunStack::Lwip)
        .expect("LWIP reference entry");
    assert!(!lwip_availability);

    app.world_mut().commands().trigger(Activate {
        entity: *lwip_entity,
    });
    app.update();
    assert!(sink.submitted().is_empty(), "reference-only LWIP is inert");

    let (mixed_entity, _, mixed_availability) = entries
        .iter()
        .find(|(_, stack, _)| *stack == TunStack::Mixed)
        .expect("Mixed button");
    assert!(*mixed_availability);
    app.world_mut().commands().trigger(Activate {
        entity: *mixed_entity,
    });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetTunStack(TunStack::Mixed)]
    );
}
