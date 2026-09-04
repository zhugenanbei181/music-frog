use bevy::app::Startup;
use bevy::color::Color;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::CommandsSceneExt;

use infiltrator_bevy_widgets::desktop::ClipboardPayload;
use infiltrator_bevy_widgets::motion::{SpringAnimator, StaggeredEnterAnimation};
use infiltrator_bevy_widgets::palette::{
    UiPalette, calculate_contrast_ratio, calculate_relative_luminance, satisfies_wcag_aa,
};
use infiltrator_bevy_widgets::shader_fx::{ShimmerWaveSpec, skeleton_card_scene};
use infiltrator_bevy_widgets::text::{RichTextSpan, Role, rich_text_line_scene};

use crate::support::headless_app;

#[test]
fn test_staggered_enter_animation_and_spring_animator() {
    let anim = StaggeredEnterAnimation::new(40.0, 200.0, 24.0);

    // Item 0 at 0.0s
    let (op0, ty0) = anim.evaluate_item(0, 0.0);
    assert_eq!(op0, 0.0);
    assert_eq!(ty0, 24.0);

    // Item 0 at 0.2s (fully entered)
    let (op0_end, ty0_end) = anim.evaluate_item(0, 0.2);
    assert_eq!(op0_end, 1.0);
    assert_eq!(ty0_end, 0.0);

    // Item 2 (starts at 0.08s) evaluated at 0.05s (still hidden)
    let (op2_pre, _) = anim.evaluate_item(2, 0.05);
    assert_eq!(op2_pre, 0.0);

    // SpringAnimator tracking
    let mut spring_anim = SpringAnimator::new(0.0, 100.0, 15.0);
    spring_anim.set_target(10.0);
    assert!(spring_anim.is_running);

    for _ in 0..120 {
        spring_anim.update(0.016);
    }
    assert!(!spring_anim.is_running);
    assert_eq!(spring_anim.spring.value, 10.0);
}

#[test]
fn test_shimmer_wave_math_and_skeleton_scene() {
    let shimmer = ShimmerWaveSpec::default();
    let pos_0 = shimmer.wave_position(0.0);
    assert_eq!(pos_0, 0.0);

    let boost_center = shimmer.brightness_boost_at(0.0, 0.0);
    assert!(boost_center > 0.0);

    let boost_far = shimmer.brightness_boost_at(0.8, 0.0);
    assert_eq!(boost_far, 0.0);

    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(skeleton_card_scene(120.0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut nodes = world.query::<&bevy::ui::prelude::Node>();
    assert!(nodes.iter(world).count() >= 4); // Root + 3 placeholder rows
}

#[test]
fn test_wcag_luminance_and_contrast_ratios() {
    let black = Color::srgb(0.0, 0.0, 0.0);
    let white = Color::srgb(1.0, 1.0, 1.0);

    assert_eq!(calculate_relative_luminance(black), 0.0);
    assert!((calculate_relative_luminance(white) - 1.0).abs() < 1e-3);

    let contrast_bw = calculate_contrast_ratio(black, white);
    assert!((contrast_bw - 21.0).abs() < 1e-1);
    assert!(satisfies_wcag_aa(black, white));

    let gray = Color::srgb(0.5, 0.5, 0.5);
    let contrast_gw = calculate_contrast_ratio(gray, white);
    assert!(contrast_gw > 3.0);
}

#[test]
fn test_rich_text_spans_and_scene_mounting() {
    let mut app = headless_app();
    let green_badge = Color::srgb(0.0, 0.8, 0.2);

    app.add_systems(
        Startup,
        move |mut commands: Commands, palette: Res<UiPalette>| {
            let spans = vec![
                RichTextSpan::new("协议: ", Role::Body),
                RichTextSpan::badge("Shadowsocks", green_badge),
                RichTextSpan::new(" 状态正常", Role::Body),
            ];
            commands.spawn_scene(rich_text_line_scene(spans, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<&bevy::ui::widget::Text>();
    assert_eq!(texts.iter(world).count(), 3);
}

#[test]
fn test_clipboard_payload_sanitization() {
    let raw_url = "https://sub.example.com/api?token=secret_jwt_token_123456&mode=clash";
    let sanitized_url = ClipboardPayload::sanitize_text(raw_url);
    assert_eq!(
        sanitized_url,
        "https://sub.example.com/api?token=REDACTED&mode=clash"
    );

    let raw_yaml = "port: 7890\nsecret: my_super_secret_key_999\nallow-lan: false";
    let sanitized_yaml = ClipboardPayload::sanitize_text(raw_yaml);
    assert!(sanitized_yaml.contains("secret: REDACTED"));
    assert!(!sanitized_yaml.contains("my_super_secret_key_999"));
}
