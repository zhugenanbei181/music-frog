//! Headless tests for the shared appearance contract on the Bevy surface
//! (DUAL-15-09): the widget-layer skin mirror never drifts from
//! `infiltrator_contract::theme`, and the shell repaints when the OS
//! appearance changes while the preference follows the system.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use bevy::window::{Window, WindowTheme};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::appearance::{
    SystemAppearance, ThemeMode, resolved_skin, skin_from_contract,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};
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
    for skin in infiltrator_contract::theme::ThemeSkin::ALL {
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
    assert_eq!(
        ThemeSkin::ALL.len(),
        infiltrator_contract::theme::ThemeSkin::ALL.len()
    );
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
            ThemePreference::Fixed(infiltrator_contract::theme::ThemeSkin::Forest),
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
    let mut app = mounted_shell(ThemePreference::Fixed(
        infiltrator_contract::theme::ThemeSkin::Amoled,
    ));
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
        ThemePreference::Fixed(infiltrator_contract::theme::ThemeSkin::Amoled)
    );
    assert_eq!(
        app.world().resource::<UiPalette>(),
        &UiPalette::new(&Theme::amoled()),
        "a pinned skin must not be repainted by the OS"
    );
}
