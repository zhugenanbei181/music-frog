use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::math::Vec2;

use infiltrator_bevy_widgets::cadence::FramePacingMode;
use infiltrator_bevy_widgets::desktop::{FramelessWindowConfig, TrayBadgeState, WindowHitZone};
use infiltrator_bevy_widgets::editor::{CodeEditorState, SyntaxTokenKind, tokenize_yaml_line};
use infiltrator_bevy_widgets::focus::{FocusDirection, find_spatial_neighbor};
use infiltrator_bevy_widgets::gesture::{
    GestureOutcome, GestureRecognizer, PullToRefreshState, SafeAreaInsets, SwipeToActionItem,
    TouchPhase,
};
use infiltrator_bevy_widgets::i18n::{
    Locale, LocaleKey, TranslationRepo, format_bytes, format_duration_secs, format_rate,
};
use infiltrator_bevy_widgets::motion::{Easing, Spring, lerp_color, lerp_f32};

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

    let repo = TranslationRepo::new(Locale::ZhCn);
    assert_eq!(repo.translate(LocaleKey::Overview), "核心概览");
    assert_eq!(repo.translate(LocaleKey::Settings), "系统设置");
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
        1000
    );
    assert_eq!(FramePacingMode::Suspended.target_frame_time_ms(), 0);
    assert!(FramePacingMode::HighRefresh.is_active());
    assert!(!FramePacingMode::Suspended.is_active());
}

use bevy::app::Startup;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::CommandsSceneExt;
use bevy::ui::prelude::Node;
use infiltrator_bevy_widgets::cadence::CadenceGovernor;
use infiltrator_bevy_widgets::editor::code_editor_scene;
use infiltrator_bevy_widgets::focus::{FocusRingStyle, focus_ring_scene};
use infiltrator_bevy_widgets::gesture::{PullToRefreshIndicator, pull_to_refresh_scene};
use infiltrator_bevy_widgets::palette::UiPalette;

use crate::support::headless_app;

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
    assert!(matches!(node.height, bevy::ui::Val::Px(h) if h > 0.0));
}

#[test]
fn test_focus_ring_and_code_editor_scenes() {
    let mut app = headless_app();
    let dark_palette = UiPalette::new(&infiltrator_bevy_widgets::theme::Theme::dark());
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
    let mut rings = world.query::<(&FocusRingStyle, &bevy::ui::BorderColor)>();
    let (style, border) = rings.iter(world).next().expect("focus ring mounted");
    assert_eq!(style.width_px, 2.0);
    assert_eq!(style.offset_px, 2.0);
    assert_eq!(border.top, dark_palette.accent);
    assert_eq!(border.bottom, dark_palette.accent);

    let mut gutters = world.query::<(&infiltrator_bevy_widgets::editor::CodeEditorGutter, &Node)>();
    let (_, gutter_node) = gutters.iter(world).next().expect("gutter mounted");
    assert_eq!(gutter_node.width, bevy::ui::Val::Px(48.0));
}
