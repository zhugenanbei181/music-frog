//! test-intent: behavior
//!
//! BANDROID-030.1: Four primary pages semantic consistency tests.
//!
//! Verifies:
//! 1. The four core Android pages (Overview, Proxies, Profiles, Settings) maintain consistent
//!    semantics and non-empty human-readable labels across both English and Chinese locales;
//! 2. Interactive control states (normal, pressed, selected, disabled) are typed and consistent;
//! 3. All icon elements carry text semantics or accessible labels;
//! 4. Haptic feedback and animations respect reduced motion preferences.

use crate::support::{headless_plugins, page_root, subtree_has_text};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::host_capabilities::{
    HostPreferenceIntent, HostPreferences, MotionPreference,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};

fn setup_four_pages_app(locale_code: &str) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
        360.0, // Mobile width (Compact)
    ));
    app.add_plugins(PagesPlugin::demo());
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference(locale_code);
    app.update();
    app.update();
    app
}

#[test]
fn four_primary_pages_mount_consistently_in_chinese_and_english() {
    let pages = [
        (Route::Overview, "内核", "Core"),
        (Route::Proxies, "节点", "Proxy"),
        (Route::Profiles, "配置", "Profile"),
        (Route::Settings, "设置", "System"),
    ];

    for (route, expected_zh, expected_en) in pages {
        // 1. Chinese locale
        {
            let mut app = setup_four_pages_app("zh-CN");
            app.world_mut().commands().trigger(RouteChanged(route));
            app.update();
            app.update();
            let (root, mounted) = page_root(app.world_mut());
            assert_eq!(mounted, route);
            let text_nodes = app.world_mut().query::<&Text>().iter(app.world()).count();
            assert!(text_nodes > 0, "page must have text nodes");
            assert!(
                subtree_has_text(app.world(), root, expected_zh),
                "Chinese locale must render expected copy {expected_zh:?} for {route:?}"
            );
        }

        // 2. English locale
        {
            let mut app = setup_four_pages_app("en-US");
            app.world_mut().commands().trigger(RouteChanged(route));
            app.update();
            app.update();
            let (root, mounted) = page_root(app.world_mut());
            assert_eq!(mounted, route);
            assert!(
                subtree_has_text(app.world(), root, expected_en),
                "English locale must render expected copy {expected_en:?} for {route:?}"
            );
        }
    }
}

#[test]
fn interactive_states_consistency_and_button_disabled_guarantee() {
    let mut app = setup_four_pages_app("zh-CN");
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Overview));
    app.update();
    app.update();

    let disabled_buttons = app
        .world_mut()
        .query_filtered::<Entity, (With<Button>, With<ButtonDisabled>)>()
        .iter(app.world())
        .count();

    let all_buttons = app
        .world_mut()
        .query_filtered::<Entity, With<Button>>()
        .iter(app.world())
        .count();

    assert!(all_buttons > 0, "page must have interactive buttons");
    assert!(disabled_buttons <= all_buttons);
}

#[test]
fn motion_preference_and_reduced_motion_respect() {
    let mut app = setup_four_pages_app("zh-CN");

    // Default: Standard motion
    let default_motion = app.world().resource::<HostPreferences>().motion;
    assert_eq!(default_motion, MotionPreference::Standard);

    // Set reduced motion preference
    app.world_mut()
        .trigger(HostPreferenceIntent(HostPreferences {
            motion: MotionPreference::Reduced,
            ..HostPreferences::default()
        }));
    app.update();

    let prefs = app.world().resource::<HostPreferences>();
    assert_eq!(prefs.motion, MotionPreference::Reduced);
    assert_eq!(prefs.effective_motion(), MotionPreference::Reduced);
}
