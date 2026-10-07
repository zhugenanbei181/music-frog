//! Behavior cases for activating.
//! test-intent: behavior

use super::*;
use infiltrator_contract::theme::ThemeSkin;

#[test]
fn activating_the_pill_cycles_every_shared_skin() {
    let mut app = mounted_shell();
    let pill = theme_pill_entity(app.world_mut());
    let slot = {
        let world = app.world_mut();
        let mut slots = world.query::<(Entity, &ContentSlot)>();
        slots.single(world).expect("content slot").0
    };
    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());
    let forest = UiPalette::new(&Theme::forest());
    let amoled = UiPalette::new(&Theme::amoled());
    // Cold start: the preference follows the OS and the shell paints its
    // documented dark default until a window reports its appearance.
    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::System
    );
    assert_eq!(app.world().resource::<UiPalette>(), &dark);

    // A non-pill activation is a no-op for the mode.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: slot });
    app.update();
    assert_eq!(
        app.world().resource::<ThemeMode>().0,
        ThemePreference::System
    );
    assert_eq!(app.world().resource::<UiPalette>(), &dark);

    let expectations = [
        (ThemePreference::Fixed(ThemeSkin::Dark), dark),
        (ThemePreference::Fixed(ThemeSkin::Light), light),
        (ThemePreference::Fixed(ThemeSkin::Forest), forest),
        (ThemePreference::Fixed(ThemeSkin::Amoled), amoled),
        (ThemePreference::Fixed(ThemeSkin::Dark), dark),
    ];
    for (expected, palette) in expectations {
        app.world_mut()
            .commands()
            .trigger(Activate { entity: pill });
        app.update();
        assert_eq!(app.world().resource::<ThemeMode>().0, expected);
        assert_eq!(
            app.world().resource::<UiPalette>(),
            &palette,
            "palette is Color-exact for {expected:?}"
        );
    }
}

#[test]
fn activating_density_pill_toggles_density() {
    let mut app = mounted_shell();
    let density_pill = density_pill_entity(app.world_mut());

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Comfortable
    );

    app.world_mut().commands().trigger(Activate {
        entity: density_pill,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Compact
    );
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Compact
    );

    app.world_mut().commands().trigger(Activate {
        entity: density_pill,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ShellLayoutState>().density,
        Density::Comfortable
    );
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Comfortable
    );
}
