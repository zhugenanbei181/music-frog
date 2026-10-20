//! Headless coverage for the platform foundations of the widget layer:
//! ABI negotiation, the auto-heal wizard, bidi mirroring, font variation,
//! boot cache, chaos injection, gamepad scrolling and mobile view slots.

use super::*;

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
        wizard.pending_actions[0].title("zh-CN"),
        "轮换控制器端口 (占用: 9090)"
    );
    assert!(!wizard.pending_actions[0].is_destructive);

    assert_eq!(
        wizard.pending_actions[1].title("zh-CN"),
        "强制启用 Strict Route 阻断直连 DNS"
    );
    assert!(!wizard.pending_actions[1].is_destructive);

    assert_eq!(wizard.pending_actions[2].title("zh-CN"), "清理僵尸核心进程");
    assert!(wizard.pending_actions[2].is_destructive);

    assert_eq!(
        wizard.pending_actions[0].title("en-US"),
        "Rotate controller port (occupied: 9090)"
    );
    assert_eq!(
        wizard.pending_actions[0].description("en-US"),
        "Find a free high port and restart the core interface"
    );
    assert_eq!(
        wizard.pending_actions[2].title("en-US"),
        "Clean up orphaned core processes"
    );
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
fn test_font_variation_axis_clamp_and_interpolation() {
    let over = FontVariationAxes {
        weight: 2000.0,
        slant: 40.0,
        width: 500.0,
    };
    let clamped = over.clamped();
    assert_eq!(clamped.weight, 900.0);
    assert_eq!(clamped.slant, 0.0);
    assert_eq!(clamped.width, 125.0);

    let under = FontVariationAxes {
        weight: 0.0,
        slant: -100.0,
        width: 0.0,
    };
    let clamped = under.clamped();
    assert_eq!(clamped.weight, 100.0);
    assert_eq!(clamped.slant, -10.0);
    assert_eq!(clamped.width, 75.0);

    // Weight interpolation clamps both `t` and the result domain.
    assert_eq!(
        FontVariationAxes::interpolate_weight(100.0, 900.0, 0.5),
        500.0
    );
    assert_eq!(
        FontVariationAxes::interpolate_weight(100.0, 900.0, 2.0),
        900.0
    );
    assert_eq!(
        FontVariationAxes::interpolate_weight(100.0, 900.0, -1.0),
        100.0
    );
    assert_eq!(FontVariationAxes::weight_at_ratio(0.0), 100.0);
    assert_eq!(FontVariationAxes::weight_at_ratio(1.0), 900.0);

    // An out-of-domain target is clamped, not overshot.
    let from = FontVariationAxes::default();
    let to = FontVariationAxes {
        weight: 5000.0,
        slant: -50.0,
        width: 100.0,
    };
    let end = from.lerp(&to, 1.0);
    assert_eq!(end.weight, 900.0);
    assert_eq!(end.slant, -10.0);
}

#[test]
fn test_rtl_mirror_symmetry_and_alignment() {
    let rtl = LayoutDirection::Rtl;
    let ltr = LayoutDirection::Ltr;

    // mirror_x keeps the opposite-edge gap and is an involution.
    let x = 24.0;
    let container = 320.0;
    let item = 80.0;
    let mirrored = rtl.mirror_x(x, container, item);
    assert_eq!(mirrored, container - x - item);
    assert_eq!(rtl.mirror_x(mirrored, container, item), x);
    assert_eq!(ltr.mirror_x(x, container, item), x);

    // mirror_rect is an involution and LTR is untouched.
    let rect = UiRect::new(Val::Px(8.0), Val::Px(16.0), Val::Px(4.0), Val::Px(2.0));
    assert_eq!(rtl.mirror_rect(rtl.mirror_rect(rect)), rect);
    assert_eq!(ltr.mirror_rect(rect), rect);

    // Alignment mirrors and stays symmetric; LTR is unaffected.
    assert_eq!(
        rtl.mirror_align_items(AlignItems::FlexStart),
        AlignItems::FlexEnd
    );
    assert_eq!(
        rtl.mirror_align_items(rtl.mirror_align_items(AlignItems::Start)),
        AlignItems::Start
    );
    assert_eq!(
        rtl.mirror_justify_content(JustifyContent::Start),
        JustifyContent::End
    );
    assert_eq!(
        ltr.mirror_align_items(AlignItems::FlexStart),
        AlignItems::FlexStart
    );
    assert_eq!(
        ltr.mirror_justify_content(JustifyContent::End),
        JustifyContent::End
    );

    // Absolute text alignment mirrors; direction-relative variants do not.
    assert_eq!(rtl.mirror_justify(Justify::Left), Justify::Right);
    assert_eq!(rtl.mirror_justify(Justify::Start), Justify::Start);
    assert_eq!(ltr.mirror_justify(Justify::Left), Justify::Left);
    assert_eq!(
        rtl.mirror_layout(FlexDirection::Row, Justify::Left),
        (FlexDirection::RowReverse, Justify::Right)
    );
    assert_eq!(
        ltr.mirror_layout(FlexDirection::Row, Justify::Left),
        (FlexDirection::Row, Justify::Left)
    );
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
