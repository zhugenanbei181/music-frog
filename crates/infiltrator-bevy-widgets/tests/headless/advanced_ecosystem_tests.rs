use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::math::Vec2;
use bevy::ui::{BorderColor, Val};
use infiltrator_bevy_widgets::cadence::FramePacingMode;
use infiltrator_bevy_widgets::desktop::{FramelessWindowConfig, TrayBadgeState, WindowHitZone};
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::editor::{CodeEditorGutter, SyntaxTokenKind, tokenize_yaml_line};
use infiltrator_bevy_widgets::focus::{FocusDirection, find_spatial_neighbor};
use infiltrator_bevy_widgets::gesture::{
    GestureOutcome, GestureRecognizer, PullToRefreshState, SafeAreaInsets, SwipeToActionItem,
    TouchPhase,
};
use infiltrator_bevy_widgets::i18n::{
    Locale, LocaleKey, TranslationRepo, format_bytes, format_duration_secs, format_rate,
};
use infiltrator_bevy_widgets::mobile_view::{
    CameraPixelFormat, CameraScanReject, CameraScanResponse, CameraScanSession, CameraScanState,
    CameraScanTexture, CameraScanTransitionError, PlatformViewLifecycleHost, PlatformViewState,
    PlatformViewTransition, PlatformViewTransitionError,
};
use infiltrator_bevy_widgets::motion::{Easing, Spring, lerp_color, lerp_f32};
use infiltrator_bevy_widgets::text_input::native::{
    NoScreenReader, ScreenReaderAction, ScreenReaderBridge, ScreenReaderCapability,
    ScreenReaderGate, ScreenReaderOutcome, SemanticNodeId,
};
use infiltrator_bevy_widgets::theme::Theme;

#[test]
fn test_touch_gesture_recognizer_tap_long_press_swipe() {
    let mut rec = GestureRecognizer::new();

    // 1. Quick tap
    rec.handle_touch(TouchPhase::Start(Vec2::new(10.0, 10.0)), 100);
    let outcome = rec.handle_touch(TouchPhase::End(Vec2::new(10.0, 10.0)), 150);
    assert_eq!(outcome, Some(GestureOutcome::Tap(Vec2::new(10.0, 10.0))));

    // 2. Long press
    rec.handle_touch(TouchPhase::Start(Vec2::new(50.0, 50.0)), 1000);
    let outcome = rec.handle_touch(TouchPhase::End(Vec2::new(50.0, 50.0)), 1600);
    assert_eq!(
        outcome,
        Some(GestureOutcome::LongPress(Vec2::new(50.0, 50.0)))
    );

    // 3. Pan and Swipe
    rec.handle_touch(TouchPhase::Start(Vec2::new(0.0, 0.0)), 2000);
    let pan_outcome = rec.handle_touch(TouchPhase::Move(Vec2::new(50.0, 0.0)), 2050);
    assert!(matches!(pan_outcome, Some(GestureOutcome::Pan { .. })));

    let swipe_outcome = rec.handle_touch(TouchPhase::End(Vec2::new(200.0, 0.0)), 2100);
    assert!(matches!(swipe_outcome, Some(GestureOutcome::Swipe { .. })));
}

#[test]
fn test_safe_area_insets_and_pull_to_refresh() {
    let insets = SafeAreaInsets::new(44.0, 16.0, 34.0, 16.0);
    assert_eq!(insets.vertical(), 78.0);
    assert_eq!(insets.horizontal(), 32.0);

    let mut ptr = PullToRefreshState::new(80.0);
    assert_eq!(ptr.fraction(), 0.0);
    ptr.pull(40.0);
    assert!(ptr.pull_offset > 0.0);
    assert!(!ptr.release());

    ptr.pull(150.0);
    assert!(ptr.release());
    assert!(ptr.is_refreshing);
    ptr.finish_refresh();
    assert!(!ptr.is_refreshing);
}

#[test]
fn test_swipe_to_action_item_dynamics() {
    let mut swipe = SwipeToActionItem::new(80.0);
    swipe.apply_drag(-50.0);
    assert_eq!(swipe.offset_x, -50.0);
    swipe.settle();
    assert_eq!(swipe.offset_x, -80.0);

    swipe.apply_drag(60.0);
    swipe.settle();
    assert_eq!(swipe.offset_x, 0.0);
}

#[test]
fn test_spatial_focus_navigation() {
    let c1 = (Entity::from_raw_u32(1).unwrap(), Vec2::new(100.0, 50.0));
    let c2 = (Entity::from_raw_u32(2).unwrap(), Vec2::new(100.0, 150.0));
    let c3 = (Entity::from_raw_u32(3).unwrap(), Vec2::new(200.0, 100.0));
    let candidates = [c1, c2, c3];

    let current = Vec2::new(100.0, 100.0);
    assert_eq!(
        find_spatial_neighbor(current, FocusDirection::Up, &candidates),
        Some(c1.0)
    );
    assert_eq!(
        find_spatial_neighbor(current, FocusDirection::Down, &candidates),
        Some(c2.0)
    );
    assert_eq!(
        find_spatial_neighbor(current, FocusDirection::Right, &candidates),
        Some(c3.0)
    );
}

#[test]
fn test_motion_easing_spring_and_color_lerp() {
    assert_eq!(Easing::Linear.evaluate(0.5), 0.5);
    assert_eq!(lerp_f32(0.0, 10.0, 0.5), 5.0);
    assert!((Easing::EaseOutQuad.evaluate(0.5) - 0.75).abs() < 1e-4);

    let red = Color::srgb(1.0, 0.0, 0.0);
    let blue = Color::srgb(0.0, 0.0, 1.0);
    let mid = lerp_color(red, blue, 0.5);
    let srgb = mid.to_srgba();
    assert!((srgb.red - 0.5).abs() < 1e-3);
    assert!((srgb.blue - 0.5).abs() < 1e-3);

    let mut spring = Spring::new(0.0, 100.0, 10.0);
    spring.target = 100.0;
    for _ in 0..120 {
        spring.step(0.016);
    }
    assert!(spring.is_settled(1.0));
}

#[test]
fn test_i18n_formatting_and_locales() {
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert_eq!(format_bytes(1024 * 1024 * 50), "50.00 MB");
    assert_eq!(format_rate(2048.0), "2.00 KB/s");
    assert_eq!(format_duration_secs(3665), "01:01:05");

    let mut repo = TranslationRepo::new(Locale::ZhCn);
    assert_eq!(repo.translate(LocaleKey::Overview), "核心概览");
    assert_eq!(repo.translate(LocaleKey::Settings), "系统设置");
    repo.current_locale = Locale::EnUs;
    assert_eq!(repo.translate(LocaleKey::Overview), "Overview");
    assert_eq!(repo.translate(LocaleKey::Settings), "Settings");
    assert_eq!(repo.translate(LocaleKey::ActionCancel), "Cancel");
}

#[test]
fn test_code_editor_state_and_yaml_tokenization() {
    let mut editor = CodeEditorState::new(
        "proxies:
  - name: direct
    type: direct",
    );
    assert_eq!(editor.line_count(), 3);
    editor.insert_char('!');
    assert!(editor.undo());
    assert!(editor.redo());

    let tokens = tokenize_yaml_line("port: 7890 # socks5");
    assert_eq!(tokens[0].kind, SyntaxTokenKind::Keyword);
    assert_eq!(tokens[0].text, "port");
    assert_eq!(tokens[1].kind, SyntaxTokenKind::Punctuation);
}

#[test]
fn test_frameless_window_hit_testing_and_tray_badge() {
    let config = FramelessWindowConfig::default();
    assert_eq!(
        config.hit_test(Vec2::new(100.0, 2.0)),
        WindowHitZone::ResizeBorderNorth
    );
    assert_eq!(
        config.hit_test(Vec2::new(1160.0, 15.0)),
        WindowHitZone::CloseButton
    );
    assert_eq!(
        config.hit_test(Vec2::new(200.0, 20.0)),
        WindowHitZone::TitlebarDrag
    );
    assert_eq!(
        config.hit_test(Vec2::new(500.0, 400.0)),
        WindowHitZone::Content
    );

    let mut badge = TrayBadgeState::default();
    badge.update(true, "12 KB/s", "1.5 MB/s");
    assert!(badge.is_running);
    assert_eq!(badge.download_rate_str, "1.5 MB/s");
}

#[test]
fn test_frame_pacing_cadence() {
    assert_eq!(FramePacingMode::HighRefresh.target_frame_time_ms(), 16);
    assert_eq!(FramePacingMode::PowerSaver.target_frame_time_ms(), 100);
    assert_eq!(
        FramePacingMode::BackgroundThrottled.target_frame_time_ms(),
        500
    );
    assert_eq!(FramePacingMode::Suspended.target_frame_time_ms(), 0);
    assert!(FramePacingMode::HighRefresh.is_active());
    assert!(!FramePacingMode::Suspended.is_active());
}

use crate::support::headless_app;
use bevy::app::Startup;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::CommandsSceneExt;
use bevy::ui::prelude::Node;
use infiltrator_bevy_widgets::cadence::CadenceGovernor;
use infiltrator_bevy_widgets::editor::code_editor_scene;
use infiltrator_bevy_widgets::focus::{FocusRingStyle, focus_ring_scene};
use infiltrator_bevy_widgets::gesture::{PullToRefreshIndicator, pull_to_refresh_scene};
use infiltrator_bevy_widgets::palette::UiPalette;

#[test]
fn test_cadence_governor_lifecycle_and_decay() {
    let mut gov = CadenceGovernor::new();
    assert_eq!(gov.current_mode, FramePacingMode::PowerSaver);

    gov.request_high_refresh(5);
    assert_eq!(gov.current_mode, FramePacingMode::HighRefresh);

    for _ in 0..5 {
        gov.tick();
    }
    assert_eq!(gov.current_mode, FramePacingMode::PowerSaver);

    // Unfocused / Invisible decay
    gov.update_window_state(false, true);
    assert_eq!(gov.current_mode, FramePacingMode::BackgroundThrottled);
}

#[test]
fn test_pull_to_refresh_and_swipe_action_scenes() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let mut ptr = PullToRefreshState::new(80.0);
            ptr.pull(60.0);
            commands.spawn_scene(pull_to_refresh_scene(&ptr, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut indicators = world.query::<(&PullToRefreshIndicator, &Node)>();
    let (_, node) = indicators.iter(world).next().expect("indicator mounted");
    assert!(matches!(node.height, Val::Px(h) if h > 0.0));
}

#[test]
fn test_focus_ring_and_code_editor_scenes() {
    let mut app = headless_app();
    let dark_palette = UiPalette::new(&Theme::dark());
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(focus_ring_scene(&palette));
            let state = CodeEditorState::new("mode: rule\nport: 7890");
            commands.spawn_scene(code_editor_scene(&state, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut rings = world.query::<(&FocusRingStyle, &BorderColor)>();
    let (style, border) = rings.iter(world).next().expect("focus ring mounted");
    assert_eq!(style.width_px, 2.0);
    assert_eq!(style.offset_px, 2.0);
    assert_eq!(border.top, dark_palette.focus_ring);
    assert_eq!(border.bottom, dark_palette.focus_ring);

    let mut gutters = world.query::<(&CodeEditorGutter, &Node)>();
    let (_, gutter_node) = gutters.iter(world).next().expect("gutter mounted");
    assert_eq!(gutter_node.width, Val::Px(48.0));
}

/// A host bridge stand-in: only activation is handled, everything else is the
/// typed unsupported answer, so the gate's pass-through is observable.
struct FakeTalkBack;

impl ScreenReaderBridge for FakeTalkBack {
    fn capability(&self) -> ScreenReaderCapability {
        ScreenReaderCapability::TalkBack
    }

    fn dispatch(&self, action: ScreenReaderAction) -> ScreenReaderOutcome {
        match action {
            ScreenReaderAction::Activate(_) => ScreenReaderOutcome::Dispatched,
            _ => ScreenReaderOutcome::Unsupported,
        }
    }
}

#[test]
fn test_screen_reader_bridge_noop_and_capability_gate() {
    let node = SemanticNodeId::new(7);
    assert_eq!(node.raw(), 7);

    // No host bridge: every semantic action is typed unsupported.
    let absent = ScreenReaderGate::absent();
    assert_eq!(absent.capability(), ScreenReaderCapability::Unsupported);
    assert!(!absent.is_supported());
    assert_eq!(
        absent.dispatch(ScreenReaderAction::Focus(node)),
        ScreenReaderOutcome::Unsupported
    );
    assert_eq!(
        absent.dispatch(ScreenReaderAction::Activate(node)),
        ScreenReaderOutcome::Unsupported
    );
    assert_eq!(
        absent.dispatch(ScreenReaderAction::SetValue {
            node,
            value: "sub.lan".to_string(),
        }),
        ScreenReaderOutcome::Unsupported
    );

    // The explicit no-op bridge answers the same typed unsupported.
    let noop = ScreenReaderGate::install(NoScreenReader);
    assert_eq!(noop.capability(), ScreenReaderCapability::Unsupported);
    assert!(!noop.is_supported());
    assert_eq!(
        noop.dispatch(ScreenReaderAction::Activate(node)),
        ScreenReaderOutcome::Unsupported
    );

    // A supported host bridge advertises its backend and passes actions through.
    let gate = ScreenReaderGate::install(FakeTalkBack);
    assert_eq!(gate.capability(), ScreenReaderCapability::TalkBack);
    assert!(gate.is_supported());
    assert_eq!(
        gate.dispatch(ScreenReaderAction::Activate(node)),
        ScreenReaderOutcome::Dispatched
    );
    assert_eq!(
        gate.dispatch(ScreenReaderAction::Focus(node)),
        ScreenReaderOutcome::Unsupported
    );
}

#[test]
fn test_camera_scan_state_machine_grant_deny_cancel_invalid() {
    // Zero-area frames are a typed invalid texture.
    assert_eq!(
        CameraScanTexture::new(1, 0, 720, CameraPixelFormat::Rgba8),
        Err(CameraScanReject::InvalidTexture)
    );
    let texture =
        CameraScanTexture::new(42, 1280, 720, CameraPixelFormat::Nv21).expect("valid texture");
    assert_eq!(texture.texture_id(), 42);
    assert_eq!(texture.width(), 1280);
    assert_eq!(texture.height(), 720);
    assert_eq!(texture.format(), CameraPixelFormat::Nv21);

    // Grant path: Idle → AwaitingPermission → Scanning → Detected.
    let mut session = CameraScanSession::new();
    assert_eq!(session.state(), CameraScanState::Idle);
    let request = session.request(9, texture).expect("idle request");
    assert_eq!(request.request_id, 9);
    assert_eq!(request.texture, texture);
    assert_eq!(session.request_id(), Some(9));
    assert_eq!(session.state(), CameraScanState::AwaitingPermission);
    assert!(session.grant_permission().is_ok());
    assert_eq!(session.state(), CameraScanState::Scanning);
    assert!(
        session
            .detect("clash://install-config?url=https://sub.lan/c.yaml")
            .is_ok()
    );
    assert_eq!(session.state(), CameraScanState::Detected);
    assert_eq!(
        session.response(),
        Some(&CameraScanResponse::Granted(
            "clash://install-config?url=https://sub.lan/c.yaml".to_string()
        ))
    );
    // Illegal transitions stay typed, never silent.
    assert_eq!(
        session.grant_permission(),
        Err(CameraScanTransitionError {
            from: CameraScanState::Detected
        })
    );
    assert!(session.reset().is_ok());
    assert_eq!(session.state(), CameraScanState::Idle);
    assert_eq!(session.response(), None);

    // Deny path.
    let mut denied = CameraScanSession::new();
    denied.request(1, texture).expect("deny request");
    assert!(denied.deny_permission().is_ok());
    assert_eq!(denied.state(), CameraScanState::Denied);
    assert_eq!(denied.response(), Some(&CameraScanResponse::Denied));

    // Cancel path.
    let mut cancelled = CameraScanSession::new();
    cancelled.request(2, texture).expect("cancel request");
    assert!(cancelled.cancel().is_ok());
    assert_eq!(cancelled.state(), CameraScanState::Cancelled);
    assert_eq!(cancelled.response(), Some(&CameraScanResponse::Cancelled));

    // Invalid path.
    let mut invalid = CameraScanSession::new();
    invalid.request(3, texture).expect("invalid request");
    invalid.grant_permission().expect("permission granted");
    assert!(invalid.invalidate(CameraScanReject::DecodeFailed).is_ok());
    assert_eq!(invalid.state(), CameraScanState::Invalid);
    assert_eq!(
        invalid.response(),
        Some(&CameraScanResponse::Invalid(CameraScanReject::DecodeFailed))
    );
}

#[test]
fn test_platform_view_lifecycle_transitions_and_no_leak() {
    let mut host = PlatformViewLifecycleHost::new("native-map");
    assert_eq!(host.state(), PlatformViewState::Unmounted);
    assert_eq!(host.unmount(), Err(PlatformViewTransitionError::NotMounted));

    assert_eq!(host.mount(77), Ok(PlatformViewTransition::Mounted));
    assert_eq!(host.state(), PlatformViewState::Mounted);
    assert_eq!(host.native_handle_id, Some(77));
    assert_eq!(
        host.mount(78),
        Err(PlatformViewTransitionError::AlreadyMounted)
    );

    assert_eq!(host.pause(), Ok(PlatformViewTransition::Paused));
    assert_eq!(host.state(), PlatformViewState::Paused);
    assert!(host.is_attached, "a paused view keeps its native handle");
    assert_eq!(
        host.pause(),
        Err(PlatformViewTransitionError::AlreadyPaused)
    );

    assert_eq!(host.resume(), Ok(PlatformViewTransition::Resumed));
    assert_eq!(host.state(), PlatformViewState::Mounted);
    assert_eq!(host.resume(), Err(PlatformViewTransitionError::NotPaused));

    assert_eq!(host.unmount(), Ok(PlatformViewTransition::Unmounted));
    assert_eq!(host.state(), PlatformViewState::Unmounted);
    assert_eq!(host.native_handle_id, None, "unmount releases the handle");
    assert!(!host.is_attached);
    assert_eq!(host.unmount(), Err(PlatformViewTransitionError::NotMounted));
}
