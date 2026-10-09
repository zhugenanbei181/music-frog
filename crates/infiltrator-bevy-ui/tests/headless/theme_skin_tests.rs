//! Headless tests for the shared appearance contract on the Bevy surface
//! (DUAL-15-09): the widget-layer skin mirror never drifts from
//! `infiltrator_contract::theme`, and the shell repaints when the OS
//! appearance changes while the preference follows the system.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use bevy::ui::prelude::BackgroundColor;
use bevy::window::{Window, WindowTheme};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::appearance::{
    SystemAppearance, ThemeMode, TonalLadderStopSwatch, ladder_preview_for_skin, resolved_skin,
    skin_from_contract,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::palette::{UiPalette, theme_color};
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin, TokenColor};
use infiltrator_bevy_widgets::theme_export::{
    DEFAULT_LADDER_STEPS, LadderSource, theme_tonal_ladders,
};
use infiltrator_contract::theme;
use infiltrator_contract::theme::ThemePreference;

fn mounted_shell(preference: ThemePreference) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(preference));
    app.update();
    app
}

#[test]
fn widget_skin_mirror_matches_the_shared_contract() {
    for skin in theme::ThemeSkin::ALL {
        let mirrored = skin_from_contract(skin);
        assert_eq!(
            mirrored.as_setting(),
            skin.as_setting(),
            "setting names must never drift"
        );
        assert_eq!(mirrored.to_index(), skin.to_index());
        assert_eq!(mirrored.is_dark(), skin.is_dark());
        assert_eq!(
            ThemeSkin::from_index(skin.to_index()).as_setting(),
            skin.as_setting()
        );
    }
    assert_eq!(ThemeSkin::ALL.len(), theme::ThemeSkin::ALL.len());
}

#[test]
fn system_preference_resolves_against_the_os_appearance() {
    assert_eq!(
        resolved_skin(ThemePreference::System, Some(true)),
        ThemeSkin::Dark
    );
    assert_eq!(
        resolved_skin(ThemePreference::System, Some(false)),
        ThemeSkin::Light
    );
    // Unknown OS appearance: the documented dark cold start.
    assert_eq!(
        resolved_skin(ThemePreference::System, None),
        ThemeSkin::Dark
    );
    // A pinned skin ignores the OS.
    assert_eq!(
        resolved_skin(
            ThemePreference::Fixed(theme::ThemeSkin::Forest),
            Some(false)
        ),
        ThemeSkin::Forest
    );
}

#[test]
fn the_shell_follows_the_os_appearance_while_preference_is_system() {
    let mut app = mounted_shell(ThemePreference::System);
    assert_eq!(app.world().resource::<SystemAppearance>().0, None);

    let window = app
        .world_mut()
        .spawn(Window {
            window_theme: Some(WindowTheme::Light),
            ..Window::default()
        })
        .id();
    app.update();
    assert_eq!(app.world().resource::<SystemAppearance>().0, Some(false));
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &UiPalette::new(&Theme::light()),
        "the mounted palette follows the OS light appearance"
    );

    app.world_mut().entity_mut(window).insert(Window {
        window_theme: Some(WindowTheme::Dark),
        ..Window::default()
    });
    app.update();
    assert_eq!(app.world().resource::<SystemAppearance>().0, Some(true));
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &UiPalette::new(&Theme::dark())
    );
}

#[test]
fn a_pinned_skin_ignores_os_appearance_changes() {
    let mut app = mounted_shell(ThemePreference::Fixed(theme::ThemeSkin::Amoled));
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &UiPalette::new(&Theme::amoled())
    );

    app.world_mut().spawn(Window {
        window_theme: Some(WindowTheme::Light),
        ..Window::default()
    });
    app.update();
    assert_eq!(app.world().resource::<SystemAppearance>().0, Some(false));
    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::Fixed(theme::ThemeSkin::Amoled)
    );
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &UiPalette::new(&Theme::amoled()),
        "a pinned skin must not be repainted by the OS"
    );
}

/// UI-04-06: Verify AMOLED pitch-black HDR contrast characteristics and
/// translucent window backdrop adaptation for Win11 Mica and macOS Vibrancy.
#[test]
fn amoled_pitch_black_contrast_and_translucent_backdrop_adaptation() {
    let amoled_theme = Theme::amoled();
    let palette = UiPalette::new(&amoled_theme);

    // 1. Amoled background is true pitch black #000000 (OLED zero-power emit)
    let clear_srgba = palette.window_clear.to_srgba();
    assert_eq!(clear_srgba.red, 0.0);
    assert_eq!(clear_srgba.green, 0.0);
    assert_eq!(clear_srgba.blue, 0.0);
    assert_eq!(clear_srgba.alpha, 1.0);

    // 2. High-contrast border against pitch black
    let border_srgba = palette.border.to_srgba();
    assert!(border_srgba.alpha >= 0.10);

    // 3. Desktop translucent backdrop adaptation (Windows 11 Mica 0.78, macOS Vibrancy 0.75, Wayland 0.82)
    let mica_palette = palette.with_translucent_window_clear(0.78);
    let mica_srgba = mica_palette.window_clear.to_srgba();
    assert!((mica_srgba.alpha - 0.78).abs() < 1e-4);
    assert_eq!(mica_srgba.red, 0.0);
    assert_eq!(mica_srgba.green, 0.0);
    assert_eq!(mica_srgba.blue, 0.0);

    let vibrancy_palette = palette.with_translucent_window_clear(0.75);
    let vibrancy_srgba = vibrancy_palette.window_clear.to_srgba();
    assert!((vibrancy_srgba.alpha - 0.75).abs() < 1e-4);

    let wayland_palette = palette.with_translucent_window_clear(0.82);
    let wayland_srgba = wayland_palette.window_clear.to_srgba();
    assert!((wayland_srgba.alpha - 0.82).abs() < 1e-4);
}

/// BEVY-034: the preview derives from the active skin's real accent token and
/// always carries the explicit sRGB fallback; token names/values stay put.
#[test]
fn tonal_ladder_preview_derives_from_real_tokens_and_the_srgb_fallback() {
    for skin in ThemeSkin::ALL {
        let theme = Theme::for_mode(skin);
        let expected = theme_tonal_ladders(&theme).accent;
        let preview = ladder_preview_for_skin(skin);
        assert_eq!(preview.perceptual, expected);
        assert_eq!(preview.perceptual.source, LadderSource::Oklch);
        assert_eq!(preview.fallback.source, LadderSource::SrgbFallback);
        assert_eq!(preview.perceptual.stops.len(), DEFAULT_LADDER_STEPS);
        assert_eq!(preview.fallback.stops.len(), DEFAULT_LADDER_STEPS);
        assert_ne!(
            preview.perceptual, preview.fallback,
            "the fallback is a distinct generator, never a silent alias"
        );
    }
    // Existing token semantics are untouched: the hand-picked seed still holds.
    assert_eq!(Theme::dark().accent, TokenColor::rgb(0.12, 0.56, 0.96));
    assert_eq!(Theme::light().accent, TokenColor::rgb(0.04, 0.44, 0.88));
}

/// BEVY-034: the mounted Settings preview paints the active skin's ladder.
#[test]
fn mounted_settings_preview_paints_the_active_skin_ladder() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(ThemePreference::Fixed(
        theme::ThemeSkin::Forest,
    )));
    app.add_plugins(PagesPlugin::default());
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    app.update();

    let ladder = theme_tonal_ladders(&Theme::for_mode(ThemeSkin::Forest)).accent;
    let mut query = app
        .world_mut()
        .query::<(&TonalLadderStopSwatch, &BackgroundColor)>();
    let mut painted = 0;
    for (swatch, fill) in query.iter(app.world()) {
        assert_eq!(fill.0, theme_color(ladder.stops[swatch.0]));
        painted += 1;
    }
    assert_eq!(painted, DEFAULT_LADDER_STEPS);
}
