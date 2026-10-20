//! Headless coverage for the typed stage guards of the widget layer:
//! sandbox instances, theme export, windowing, the circuit breaker, the
//! focus trap, the chaos runner, the reorder drag and the healing watchdog.

use super::*;

#[test]
fn test_sandbox_widget_instance() {
    let mut widget = SandboxedWidgetInstance::new("w_ping", "network_monitor");
    assert_eq!(widget.widget_id, "w_ping");
    assert_eq!(widget.plugin_name, "network_monitor");
    assert_eq!(widget.quota.max_memory_bytes, 4 * 1024 * 1024);

    assert!(widget.write_state("last_latency", "14ms"));
    assert_eq!(widget.read_state("last_latency"), Some("14ms"));
    assert_eq!(widget.read_state("non_existent"), None);

    // Update existing key
    assert!(widget.write_state("last_latency", "22ms"));
    assert_eq!(widget.read_state("last_latency"), Some("22ms"));
}

#[test]
fn test_theme_export_tokens() {
    let palette = UiPalette::new(&Theme::dark());
    let json_export = export_palette_tokens(&palette, ThemeExportFormat::Json);
    assert!(json_export.starts_with('{'));
    assert!(json_export.ends_with('}'));
    assert!(json_export.contains(r#""accent":"#));
    assert!(json_export.contains(r#""surface":"#));

    let css_export = export_palette_tokens(&palette, ThemeExportFormat::TailwindCss);
    assert!(css_export.starts_with(":root {"));
    assert!(css_export.contains("--color-accent:"));
    assert!(css_export.contains("--color-surface:"));

    let xml_export = export_palette_tokens(&palette, ThemeExportFormat::MaterialYouXml);
    assert!(xml_export.contains("<resources>"));
    assert!(xml_export.contains("md_theme_primary"));
}

#[test]
fn test_windowing_dock_and_pip_overlay() {
    let panel = DockPanel::new("p1", "Diagnostics", DockSlot::Right);
    assert_eq!(panel.id, "p1");
    assert_eq!(panel.title, "Diagnostics");
    assert_eq!(panel.current_slot, DockSlot::Right);
    assert!(!panel.is_floating);

    let mut pip = PipOverlayState::default();
    assert!(pip.is_pinned_top);

    // Top Right
    pip.snap_to_corner(Vec2::new(1920.0, 1080.0), true);
    assert_eq!(pip.position, Vec2::new(1920.0 - 180.0 - 20.0, 20.0));

    // Bottom Left
    pip.snap_to_corner(Vec2::new(1920.0, 1080.0), false);
    assert_eq!(pip.position, Vec2::new(20.0, 1080.0 - 64.0 - 20.0));

    // The session stage lives in exactly one place: the bound window handle.
    let mut registry = WindowRegistry::new();
    let mut session = PipOverlaySession::new(PipOverlayState::default());
    assert!(!session.is_active());
    let id = session.open(&mut registry, "Mini HUD").expect("opened");
    assert!(session.is_active());
    assert_eq!(session.window, Some(id));

    // A descriptor retired underneath the session leaves no "active but
    // windowless" overlay behind: the handle is the only stage there is.
    registry.remove(id).expect("descriptor retired");
    assert!(!session.move_to(&mut registry, Vec2::new(1.0, 2.0)));
    assert!(session.window.is_none());
    assert!(!session.is_active());

    // The window-manager attributes stay independent facts of the overlay.
    let mut overlay = PipOverlayState::default();
    overlay.toggle_pin();
    overlay.set_click_through(true);
    assert!(!overlay.is_pinned_top);
    assert!(overlay.is_click_through);
}

#[test]
fn test_node_circuit_breaker_and_healing_phase() {
    use infiltrator_bevy_widgets::auto_heal::{CircuitState, HealingPhase, NodeCircuitBreaker};
    use std::time::Duration;

    let mut breaker = NodeCircuitBreaker::new("SG-01");
    assert_eq!(breaker.state, CircuitState::Closed);
    assert!(breaker.is_healthy());

    for i in 1..=3 {
        breaker.record_failure(Duration::from_secs(i));
    }
    assert_eq!(breaker.state, CircuitState::Open);
    assert!(!breaker.can_attempt(Duration::from_secs(10)));

    // Test healing phase state
    let phase = HealingPhase::Scanning;
    assert_eq!(phase, HealingPhase::Scanning);
}

#[test]
fn test_advanced_ecosystem_round_two_capabilities() {
    use bevy::color::Color;
    use infiltrator_bevy_widgets::abi::{HostCapabilities, WidgetCapability};
    use infiltrator_bevy_widgets::chaos::{ChaosFaultScenario, ChaosSimulationRunner};
    use infiltrator_bevy_widgets::editor::{DiagnosticSeverity, validate_yaml_rule_syntax};
    use infiltrator_bevy_widgets::focus::GridFocusNavigator;
    use infiltrator_bevy_widgets::gesture::PinchZoomController;
    use infiltrator_bevy_widgets::shader_fx::{AnalyticalDropShadow, GlowSpec};

    // 1. Editor rule validation
    let rules = vec![
        "- DOMAIN-SUFFIX,example.com,DIRECT".to_string(),
        "- DOMAIN-KEYWORD,google".to_string(),
    ];
    let diags = validate_yaml_rule_syntax(&rules);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);

    // 2. Pinch zoom controller
    let mut pz = PinchZoomController::new();
    pz.apply_pinch(1.5, Vec2::new(200.0, 200.0));
    assert_eq!(pz.zoom_level, 1.5);

    // 3. Grid focus navigator
    let mut grid = GridFocusNavigator::new(2, 2, false);
    assert_eq!(grid.current_index(), 0);
    assert!(grid.move_right());
    assert_eq!(grid.current_index(), 1);

    // 4. Analytical drop shadow & glow
    let shadow = AnalyticalDropShadow::elevation_medium();
    assert_eq!(shadow.offset, Vec2::new(0.0, 6.0));
    assert!(shadow.falloff_alpha(0.0) > 0.9);

    let glow = GlowSpec::new(Color::srgb(0.0, 1.0, 0.0), 10.0, 2.0);
    assert!(glow.current_alpha(0.0) >= glow.min_alpha);

    // 5. Chaos simulation runner
    let mut runner = ChaosSimulationRunner::new();
    runner.inject(ChaosFaultScenario::tun_failure());
    assert!(runner.is_running());

    // 6. Host capabilities
    let mut caps = HostCapabilities::none();
    caps.enable(WidgetCapability::GpuShaders);
    assert!(caps.has(WidgetCapability::GpuShaders));
}

#[test]
fn test_focus_trap_stage_is_typed_and_refuses_a_second_engage() {
    use bevy::ecs::entity::Entity;
    use infiltrator_bevy_widgets::focus::{FocusTrapManager, FocusTrapStage};

    let root = Entity::from_raw_u32(30).unwrap();
    let other_root = Entity::from_raw_u32(31).unwrap();
    let focusable = Entity::from_raw_u32(32).unwrap();

    let mut trap = FocusTrapManager::new();
    assert_eq!(trap.stage(), &FocusTrapStage::Released);
    assert!(!trap.is_trapped());
    assert_eq!(trap.trap_root(), None);
    assert!(
        trap.cycle_next().is_none(),
        "a released trap has no focusable to move to"
    );

    trap.engage(root, vec![focusable], None)
        .expect("released trap engages");
    assert_eq!(trap.stage(), &FocusTrapStage::Trapped { root });
    assert!(trap.is_trapped());
    assert_eq!(trap.trap_root(), Some(root));
    assert_eq!(trap.cycle_next(), Some(focusable));

    // Engaging a second modal while the trap runs is refused; the running
    // stage comes back and the trap keeps its own root.
    let refusal = trap
        .engage(other_root, vec![focusable], None)
        .expect_err("a second engage is refused while a trap is engaged");
    assert_eq!(refusal.from, FocusTrapStage::Trapped { root });
    assert_eq!(trap.stage(), &FocusTrapStage::Trapped { root });
    assert_eq!(trap.trap_root(), Some(root));

    // Releasing returns to the single released stage and drops the root.
    assert_eq!(trap.release(), None);
    assert_eq!(trap.stage(), &FocusTrapStage::Released);
    assert!(!trap.is_trapped());
    assert_eq!(trap.trap_root(), None);
}

#[test]
fn test_chaos_runner_stage_is_typed_and_refuses_to_tick_when_idle() {
    use infiltrator_bevy_widgets::chaos::{
        ChaosFaultScenario, ChaosRunStage, ChaosSimulationRunner,
    };

    let mut runner = ChaosSimulationRunner::new();
    assert_eq!(runner.stage(), &ChaosRunStage::Idle);
    assert!(!runner.is_running());
    assert_eq!(runner.active_scenario(), None);
    assert!(
        runner.tick(1.0).is_none(),
        "an idle runner has no scenario to inject"
    );

    let scenario = ChaosFaultScenario::dns_pollution();
    runner.inject(scenario.clone());
    assert_eq!(
        runner.stage(),
        &ChaosRunStage::Running {
            scenario: scenario.clone()
        }
    );
    assert!(runner.is_running());
    assert_eq!(runner.active_scenario(), Some(&scenario));

    // The scenario lives inside the stage, so "running without a scenario" is
    // not a state that can be observed at all.
    assert_eq!(runner.tick(1.0), Some(DiagnosticAnomaly::DnsLeakDetected));
    assert!(runner.is_running());

    // Past the scenario duration the runner returns to the single idle stage.
    assert_eq!(runner.tick(20.0), None);
    assert_eq!(runner.stage(), &ChaosRunStage::Idle);
    assert!(!runner.is_running());
    assert_eq!(runner.active_scenario(), None);
}

#[test]
fn test_reorder_drag_stage_is_typed_and_refuses_a_drop_without_a_drag() {
    use infiltrator_bevy_widgets::reorderable::{ReorderDragStage, ReorderableListState};

    let mut list = ReorderableListState::new(vec!["A".into(), "B".into(), "C".into()]);
    assert_eq!(list.drag_stage(), ReorderDragStage::Idle);
    assert_eq!(list.dragged_item_index(), None);
    assert_eq!(list.target_drop_index(), None);

    // A drop position without a dragged item is not a state the list can hold.
    assert!(
        list.set_drop_target(2).is_err(),
        "a drop position needs a dragged item"
    );

    list.start_drag(0).expect("idle list starts a drag");
    assert_eq!(list.dragged_item_index(), Some(0));

    // A second drag while one runs is refused.
    assert!(list.start_drag(1).is_err());

    list.set_drop_target(2)
        .expect("running drag takes a drop position");
    assert_eq!(list.target_drop_index(), Some(2));
    assert_eq!(list.end_drag(), Some(2));
    assert_eq!(list.drag_stage(), ReorderDragStage::Idle);
    assert_eq!(list.target_drop_index(), None);
}

#[test]
fn test_healing_watchdog_stage_is_typed_and_refuses_double_arm() {
    use infiltrator_bevy_widgets::auto_heal::{HealingWatchdog, WatchdogStage};
    use std::time::Duration;

    let mut watchdog = HealingWatchdog::new(Duration::from_secs(2));

    // Idle: no repair runs and no trip is latched.
    assert_eq!(watchdog.stage(), &WatchdogStage::Idle);
    assert!(!watchdog.is_repairing());
    assert!(!watchdog.has_tripped());
    assert!(
        !watchdog.tick(Duration::from_secs(5)),
        "an idle watchdog has no running timer to step"
    );

    // Repairing: the running stage names the action under the watchdog.
    watchdog.arm("rebind_tun").expect("idle watchdog arms");
    assert_eq!(
        watchdog.stage(),
        &WatchdogStage::Repairing {
            action_name: "rebind_tun".to_string()
        }
    );

    // Arming again while a repair runs is refused; the running stage comes back.
    let refusal = watchdog
        .arm("other_action")
        .expect_err("a second arm is refused while a repair runs");
    assert_eq!(
        refusal.from,
        WatchdogStage::Repairing {
            action_name: "rebind_tun".to_string()
        }
    );
    assert_eq!(
        watchdog.stage(),
        &WatchdogStage::Repairing {
            action_name: "rebind_tun".to_string()
        },
        "a refused arm must not replace the running repair"
    );

    // The deadline trips the watchdog into the exclusive Tripped stage.
    assert!(watchdog.tick(Duration::from_secs(2)));
    assert_eq!(
        watchdog.stage(),
        &WatchdogStage::Tripped {
            action_name: "rebind_tun".to_string()
        }
    );
    assert!(watchdog.has_tripped());
    assert!(
        !watchdog.is_repairing(),
        "a tripped watchdog is not repairing"
    );

    // A tripped watchdog refuses further ticks and can be re-armed for the next action.
    assert!(
        !watchdog.tick(Duration::from_secs(10)),
        "a tripped watchdog refuses to step its timer"
    );
    watchdog
        .arm("next_action")
        .expect("a tripped watchdog re-arms for the next action");
    assert_eq!(
        watchdog.stage(),
        &WatchdogStage::Repairing {
            action_name: "next_action".to_string()
        }
    );

    // Disarming returns to the single idle stage.
    watchdog.disarm();
    assert_eq!(watchdog.stage(), &WatchdogStage::Idle);
    assert_eq!(watchdog.active_action_name(), None);
}
