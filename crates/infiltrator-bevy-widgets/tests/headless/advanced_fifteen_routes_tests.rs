use bevy::math::Vec2;
use bevy::ui::prelude::{FlexDirection, UiRect, Val};

use bevy::color::Color;
use infiltrator_bevy_widgets::abi::{
    DesktopRunnerHost, UniversalRunnerHost, WIDGET_ABI_VERSION, is_abi_compatible,
};
use infiltrator_bevy_widgets::auto_heal::{AutoHealWizardState, DiagnosticAnomaly};
use infiltrator_bevy_widgets::bidi::{FontVariationAxes, LayoutDirection};
use infiltrator_bevy_widgets::boot_cache::{BootPipelineCache, StaticByteTable};
use infiltrator_bevy_widgets::chaos::{ChaosFaultConfig, MonkeyExplorerBot};
use infiltrator_bevy_widgets::gamepad_ui::GamepadScrollState;
use infiltrator_bevy_widgets::mobile_view::{
    CameraPixelFormat, CameraTextureFeed, NativePlatformViewSlot,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::sandbox::SandboxedWidgetInstance;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_bevy_widgets::theme_export::{ThemeExportFormat, export_palette_tokens};
use infiltrator_bevy_widgets::windowing::{DockPanel, DockSlot, PipOverlayState};

#[test]
fn test_abi_version_and_runner_host() {
    assert_eq!(WIDGET_ABI_VERSION, (0, 30, 0));
    assert!(is_abi_compatible((0, 30, 0)));
    assert!(is_abi_compatible((0, 30, 5)));
    assert!(!is_abi_compatible((1, 0, 0)));
    assert!(!is_abi_compatible((0, 29, 0)));

    let host = DesktopRunnerHost;
    assert_eq!(host.abi_version(), (0, 30, 0));
    assert!(!host.is_headless());
    assert_eq!(host.host_name(), "MusicFrog Infiltrator Desktop Runner");
}

#[test]
fn test_auto_heal_wizard_anomalies_and_actions() {
    let mut wizard = AutoHealWizardState::new();
    wizard.push_anomaly(DiagnosticAnomaly::ControllerPortConflict(9090));
    wizard.push_anomaly(DiagnosticAnomaly::DnsLeakDetected);
    wizard.push_anomaly(DiagnosticAnomaly::ZombieProcessDetected);

    assert_eq!(wizard.active_anomalies.len(), 3);
    assert_eq!(wizard.pending_actions.len(), 3);

    assert_eq!(
        wizard.pending_actions[0].title,
        "轮换控制器端口 (占用: 9090)"
    );
    assert!(!wizard.pending_actions[0].is_destructive);

    assert_eq!(
        wizard.pending_actions[1].title,
        "强制启用 Strict Route 阻断直连 DNS"
    );
    assert!(!wizard.pending_actions[1].is_destructive);

    assert_eq!(wizard.pending_actions[2].title, "清理僵尸核心进程");
    assert!(wizard.pending_actions[2].is_destructive);

    // Idempotent push
    wizard.push_anomaly(DiagnosticAnomaly::ControllerPortConflict(9090));
    assert_eq!(wizard.active_anomalies.len(), 3);

    wizard.clear();
    assert!(wizard.active_anomalies.is_empty());
    assert!(wizard.pending_actions.is_empty());
    assert!(!wizard.is_repairing);
}

#[test]
fn test_bidi_mirroring_and_font_variations() {
    let rtl = LayoutDirection::Rtl;
    assert!(rtl.is_rtl());

    let rect = UiRect::new(Val::Px(10.0), Val::Px(20.0), Val::Px(5.0), Val::Px(5.0));
    let mirrored = rtl.mirror_rect(rect);
    assert_eq!(mirrored.left, Val::Px(20.0));
    assert_eq!(mirrored.right, Val::Px(10.0));
    assert_eq!(mirrored.top, Val::Px(5.0));
    assert_eq!(mirrored.bottom, Val::Px(5.0));

    assert_eq!(
        rtl.mirror_flex_direction(FlexDirection::Row),
        FlexDirection::RowReverse
    );
    assert_eq!(
        rtl.mirror_flex_direction(FlexDirection::RowReverse),
        FlexDirection::Row
    );
    assert_eq!(
        rtl.mirror_flex_direction(FlexDirection::Column),
        FlexDirection::Column
    );

    let ltr = LayoutDirection::Ltr;
    assert_eq!(ltr.mirror_rect(rect), rect);

    let axes1 = FontVariationAxes {
        weight: 400.0,
        slant: 0.0,
        width: 100.0,
    };
    let axes2 = FontVariationAxes {
        weight: 700.0,
        slant: -5.0,
        width: 100.0,
    };
    let mid = axes1.lerp(&axes2, 0.5);
    assert_eq!(mid.weight, 550.0);
    assert_eq!(mid.slant, -2.5);
    assert_eq!(mid.width, 100.0);
}

#[test]
fn test_boot_cache_and_static_byte_table() {
    let mut cache = BootPipelineCache::new();
    assert!(!cache.is_warmed_up);
    cache.mark_warmed(12, 128, 45);
    assert!(cache.is_warmed_up);
    assert_eq!(cache.cached_shader_count, 12);
    assert_eq!(cache.cached_font_glyphs, 128);
    assert_eq!(cache.boot_duration_ms, 45);

    static RAW_BYTES: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let table = StaticByteTable::new(&RAW_BYTES, 4);
    assert_eq!(table.entry_count(), 2);
    assert_eq!(table.get_entry(0), Some(&[1, 2, 3, 4][..]));
    assert_eq!(table.get_entry(1), Some(&[5, 6, 7, 8][..]));
    assert_eq!(table.get_entry(2), None);

    let empty_table = StaticByteTable::new(&[], 4);
    assert_eq!(empty_table.entry_count(), 0);
}

#[test]
fn test_chaos_fault_config_and_monkey_bot() {
    let chaos = ChaosFaultConfig {
        is_enabled: true,
        packet_drop_rate: 0.3,
        latency_jitter_ms: 25.0,
        ..Default::default()
    };
    assert!(chaos.should_drop_packet(0.2));
    assert!(!chaos.should_drop_packet(0.5));
    assert_eq!(chaos.latency_jitter_ms, 25.0);

    let mut monkey = MonkeyExplorerBot::new();
    monkey.record_action("Overview");
    monkey.record_action("Proxies");
    monkey.record_exception();

    assert_eq!(monkey.actions_executed, 2);
    assert_eq!(monkey.routes_visited, vec!["Overview", "Proxies"]);
    assert_eq!(monkey.exceptions_caught, 1);
}

#[test]
fn test_gamepad_scrolling_physics() {
    let mut state = GamepadScrollState::new();

    // 1. Inputs below deadzone produce 0 delta
    state.update_sticks(Vec2::ZERO, Vec2::new(0.0, 0.05));
    assert_eq!(state.right_stick, Vec2::ZERO);
    let delta = state.step(0.016);
    assert_eq!(delta, Vec2::ZERO);

    // 2. Inputs above deadzone produce proportional velocity
    state.update_sticks(Vec2::ZERO, Vec2::new(0.0, 1.0));
    assert_eq!(state.right_stick, Vec2::new(0.0, 1.0));
    let delta1 = state.step(0.016);
    assert!(delta1.y > 0.0);

    let delta2 = state.step(0.016);
    assert!(delta2.y >= delta1.y); // Acceleration towards target
}

#[test]
fn test_mobile_view_camera_feed_and_slot() {
    let mut feed = CameraTextureFeed::default();
    feed.start_stream(1920, 1080, CameraPixelFormat::Rgba8);
    assert!(feed.is_streaming);
    assert_eq!(feed.frame_byte_size(), 1920 * 1080 * 4);

    feed.on_qr_detected("clash://install-config?url=https://sub.lan/clash.yaml");
    assert_eq!(
        feed.parse_qr_config_url(),
        Some("https://sub.lan/clash.yaml".to_string())
    );

    feed.stop_stream();
    assert!(!feed.is_streaming);
    assert_eq!(feed.parse_qr_config_url(), None);

    let slot = NativePlatformViewSlot::new("map_view", Vec2::new(300.0, 200.0));
    assert_eq!(slot.view_type_id, "map_view");
    assert_eq!(slot.bounds_size, Vec2::new(300.0, 200.0));
    assert!(slot.is_visible);
}

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
    assert!(runner.is_running);

    // 6. Host capabilities
    let mut caps = HostCapabilities::none();
    caps.enable(WidgetCapability::GpuShaders);
    assert!(caps.has(WidgetCapability::GpuShaders));
}

#[test]
fn test_advanced_ecosystem_round_three_capabilities() {
    use infiltrator_bevy_widgets::auto_heal::MultiNodeCircuitRegistry;
    use infiltrator_bevy_widgets::chart::quantiles::{LatencyQuantiles, compute_empirical_cdf};
    use infiltrator_bevy_widgets::context_menu::{
        ContextMenuItem, ContextMenuPlacement, ContextMenuState,
    };
    use infiltrator_bevy_widgets::fluid_grid::compute_ideal_column_layout;
    use infiltrator_bevy_widgets::sandbox::{WidgetManifest, WidgetPermission};
    use infiltrator_bevy_widgets::signal_dag::ReactiveDag;

    // 1. Context Menu boundary flipping
    let origin = ContextMenuPlacement::calculate_origin(
        Vec2::new(980.0, 780.0),
        Vec2::new(180.0, 150.0),
        Vec2::new(1000.0, 800.0),
    );
    assert_eq!(origin, Vec2::new(980.0 - 180.0, 780.0 - 150.0));

    let mut menu_state = ContextMenuState::new();
    menu_state.open_at(
        Vec2::new(100.0, 100.0),
        vec![ContextMenuItem::new("act1", "Ping Action")],
        Vec2::new(180.0, 50.0),
        Vec2::new(800.0, 600.0),
    );
    assert!(menu_state.is_open);

    // 2. Fluid grid ideal column layout
    let layout = compute_ideal_column_layout(1000.0, 240.0, 16.0, 4);
    assert_eq!(layout.columns, 3);
    assert!(layout.item_width_px >= 240.0);

    // 3. Latency Quantiles & CDF
    let latencies = [30.0, 50.0, 80.0, 120.0, 300.0];
    let q = LatencyQuantiles::compute(&latencies).unwrap();
    assert_eq!(q.count, 5);
    assert!(q.p50 >= 50.0);

    let cdf = compute_empirical_cdf(&latencies, 3);
    assert_eq!(cdf.len(), 3);

    // 4. Reactive DAG
    let mut dag = ReactiveDag::new();
    let a = dag.create_signal(1);
    let b = dag.create_derived(&[a], 2);
    assert!(!dag.has_cycle());
    assert_eq!(dag.update_signal(a, 5), vec![b]);

    // 5. Multi-node Circuit Registry
    let mut reg = MultiNodeCircuitRegistry::new();
    let nodes = ["Node1", "Node2"];
    assert_eq!(
        reg.select_first_healthy(&nodes, std::time::Duration::ZERO),
        Some("Node1")
    );

    // 6. Widget Manifest
    let manifest = WidgetManifest::new("w1", "Plugin Widget")
        .with_permission(WidgetPermission::ReadTrafficStats);
    assert!(manifest.validate().is_ok());
    assert!(manifest.has_permission(WidgetPermission::ReadTrafficStats));
}

#[test]
fn test_advanced_ecosystem_round_four_capabilities() {
    use infiltrator_bevy_widgets::auto_heal::ExponentialBackoffPolicy;
    use infiltrator_bevy_widgets::chart::bezier::{compute_crest_factor, find_waveform_extrema};
    use infiltrator_bevy_widgets::clipboard_sanitizer::{
        mask_sensitive_token, sanitize_pasted_text,
    };
    use infiltrator_bevy_widgets::splitter::{DEFAULT_SNAP_ANCHORS, apply_snap_anchors};
    use infiltrator_bevy_widgets::theme::{TokenColor, contrast};
    use infiltrator_bevy_widgets::tsdb::{DeltaCompressedSeries, TelemetrySample};

    // 1. Clipboard Sanitizer
    let cleaned = sanitize_pasted_text("link\u{200B}\r\n");
    assert_eq!(cleaned, "link\n");
    assert_eq!(mask_sensitive_token("1234567890"), "1234...7890");

    // 2. Splitter Snap Anchors
    let (snapped, ok) = apply_snap_anchors(0.495, DEFAULT_SNAP_ANCHORS, 0.02);
    assert!(ok);
    assert_eq!(snapped, 0.50);

    // 3. Telemetry Extrema & Crest Factor
    let samples = [5.0, 30.0, 10.0, 45.0, 5.0];
    let extrema = find_waveform_extrema(&samples, 5.0);
    assert_eq!(extrema.len(), 3);
    assert!(compute_crest_factor(&samples) > 1.2);

    // 4. TSDB Delta Compression
    let telemetry = [
        TelemetrySample {
            timestamp_sec: 100,
            upload_bytes: 10,
            download_bytes: 20,
            active_connections: 1,
            latency_ms: 10.0,
        },
        TelemetrySample {
            timestamp_sec: 101,
            upload_bytes: 15,
            download_bytes: 25,
            active_connections: 1,
            latency_ms: 10.0,
        },
    ];
    let comp = DeltaCompressedSeries::compress(&telemetry).unwrap();
    assert_eq!(comp.decompress().len(), 2);

    // 5. Exponential Backoff
    let backoff = ExponentialBackoffPolicy::default();
    assert_eq!(
        backoff.delay_for_attempt(1),
        std::time::Duration::from_millis(1000)
    );

    // 6. WCAG AAA Contrast
    let black = TokenColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let white = TokenColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    assert!(contrast::is_wcag_aaa(contrast::contrast_ratio(
        white, black
    )));
}

#[test]
fn test_advanced_ecosystem_round_five_capabilities() {
    use infiltrator_bevy_widgets::auto_heal::AutoRollbackTransaction;
    use infiltrator_bevy_widgets::boot_cache::ZeroAllocBudgetMeter;
    use infiltrator_bevy_widgets::chart::bezier::CatmullRomSpline;
    use infiltrator_bevy_widgets::list::scroll_core::{DynamicHeightIndex, ScrollAnchorBookmark};
    use std::time::Duration;

    // 1. Scroll Anchor Bookmark
    let index = DynamicHeightIndex::new(10, 40.0);
    let bookmark = ScrollAnchorBookmark::create(105.0, &index);
    assert_eq!(bookmark.item_index, 2);
    assert_eq!(bookmark.restore_scroll_offset(&index), 105.0);

    // 2. Catmull-Rom Spline
    let interp = CatmullRomSpline::interpolate_sequence(&[10.0, 20.0, 30.0], 2);
    assert_eq!(interp.len(), 5);
    assert_eq!(interp[0], 10.0);
    assert_eq!(interp[4], 30.0);

    // 3. Auto-Rollback Transaction
    let mut tx = AutoRollbackTransaction::new("old_dns".to_string(), Duration::from_secs(2));
    tx.apply_mutation("new_dns".to_string());
    assert_eq!(tx.current_state, "new_dns");
    // Timeout triggers rollback
    assert!(tx.tick(Duration::from_secs(3)));
    assert_eq!(tx.current_state, "old_dns");

    // 4. Zero-Alloc Budget Meter
    let mut meter = ZeroAllocBudgetMeter::new();
    meter.record_frame(0, 0);
    assert!(meter.is_budget_compliant());
}

#[test]
fn test_advanced_ecosystem_round_six_capabilities() {
    use infiltrator_bevy_widgets::accordion::{AccordionMode, AccordionState};
    use infiltrator_bevy_widgets::auto_heal::FailoverRoutingGraph;
    use infiltrator_bevy_widgets::chart::log_scale::LogarithmicScaleMapper;
    use infiltrator_bevy_widgets::palette::{ThemeTokenPatch, UiPalette};
    use infiltrator_bevy_widgets::reorderable::{ReorderAction, ReorderableListState};
    use infiltrator_bevy_widgets::theme::Theme;

    // 1. Reorderable list
    let mut reorder = ReorderableListState::new(vec!["A".into(), "B".into(), "C".into()]);
    assert!(reorder.apply_action(ReorderAction::MoveDown(0)));
    assert_eq!(reorder.items, vec!["B", "A", "C"]);

    // 2. Accordion batch controls
    let mut acc = AccordionState::new(
        vec![("Section 1".into(), false), ("Section 2".into(), false)],
        AccordionMode::Single,
    );
    acc.expand_all();
    assert!(acc.is_expanded(0));
    assert!(acc.is_expanded(1));

    // 3. Logarithmic scale mapper
    let mapper = LogarithmicScaleMapper::new(10_000_000.0, 1.0);
    assert_eq!(mapper.map_to_normalized(0.0), 0.0);
    assert_eq!(mapper.map_to_normalized(10_000_000.0), 1.0);

    // 4. Failover routing graph
    let mut graph = FailoverRoutingGraph::new();
    graph.link_fallback("US-01", "US-02");
    assert_eq!(graph.resolve_active_outbound("US-01", |_| false), "US-01");
    assert_eq!(
        graph.resolve_active_outbound("US-01", |n| n == "US-01"),
        "US-02"
    );

    // 5. Palette scoped token patch
    let palette = UiPalette::new(&Theme::dark());
    let patch = ThemeTokenPatch {
        accent: Some(Color::srgb(1.0, 0.0, 0.0)),
        ..Default::default()
    };
    let patched = palette.with_patch(&patch);
    assert_eq!(patched.accent, Color::srgb(1.0, 0.0, 0.0));
}

#[test]
fn test_advanced_ecosystem_round_seven_capabilities() {
    use infiltrator_bevy_widgets::auto_heal::RootCauseInferenceEngine;
    use infiltrator_bevy_widgets::cadence::FrameTimingProbe;
    use infiltrator_bevy_widgets::chart::nice_scale::NiceScale;
    use infiltrator_bevy_widgets::datagrid::ColumnResizeState;

    // 1. Column resize state
    let mut resize = ColumnResizeState::new();
    let mut widths = vec![120.0, 180.0];
    resize.start_drag(0, 100.0, 120.0);
    assert!(resize.apply_drag(140.0, &mut widths));
    assert_eq!(widths[0], 160.0);

    // 2. Nice scale tick generator
    let scale = NiceScale::compute(0.0, 48.0, 5);
    assert_eq!(scale.min, 0.0);
    assert_eq!(scale.max, 50.0);
    assert_eq!(scale.tick_spacing, 10.0);

    // 3. Root cause inference engine
    let anomalies = vec![
        DiagnosticAnomaly::HighPacketLoss,
        DiagnosticAnomaly::TunInterfaceMissing,
    ];
    let primary = RootCauseInferenceEngine::identify_primary(&anomalies);
    assert_eq!(primary, Some(DiagnosticAnomaly::TunInterfaceMissing));

    // 4. Frame timing probe
    let mut probe = FrameTimingProbe::new(30, 16.67);
    probe.record_frame(8.33); // 120 fps
    assert!(probe.is_stutter_free());
    assert!(probe.average_fps() > 100.0);
}

#[test]
fn test_advanced_ecosystem_round_eight_capabilities() {
    use bevy::ecs::entity::Entity;
    use infiltrator_bevy_widgets::auto_heal::HealingWatchdog;
    use infiltrator_bevy_widgets::chart::ring_buffer::RollingWindowAggregator;
    use infiltrator_bevy_widgets::focus::FocusTrapManager;
    use infiltrator_bevy_widgets::mobile_view::PlatformViewLifecycleHost;
    use infiltrator_bevy_widgets::motion::CardParallaxTilt;
    use std::time::Duration;

    // 1. Focus trap manager
    let mut trap = FocusTrapManager::new();
    let e1 = Entity::from_raw_u32(10).unwrap();
    let e2 = Entity::from_raw_u32(20).unwrap();
    trap.engage(e1, vec![e1, e2], None);
    assert!(trap.is_trapped);
    assert_eq!(trap.cycle_next(), Some(e2));

    // 2. Card parallax tilt
    let tilt_calc = CardParallaxTilt::new(Vec2::new(100.0, 100.0), Vec2::new(200.0, 200.0), 10.0);
    let (angles, highlight) = tilt_calc.evaluate(Vec2::new(100.0, 100.0));
    assert_eq!(angles, Vec2::ZERO);
    assert!(highlight > 0.9);

    // 3. Rolling window aggregator
    let mut agg = RollingWindowAggregator::<3>::new();
    agg.push(10.0);
    agg.push(20.0);
    assert_eq!(agg.moving_average(), 15.0);

    // 4. Healing watchdog
    let mut watchdog = HealingWatchdog::new(Duration::from_secs(3));
    watchdog.arm("heal_test");
    assert!(!watchdog.tick(Duration::from_secs(1)));
    assert!(watchdog.tick(Duration::from_secs(3)));
    assert!(watchdog.has_tripped);

    // 5. Platform view lifecycle host
    let mut pv = PlatformViewLifecycleHost::new("view1");
    pv.attach(999);
    assert!(pv.is_attached);
    assert_eq!(pv.detach(), Some(999));
}
