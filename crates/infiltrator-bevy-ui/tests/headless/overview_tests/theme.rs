//! Behavior cases for theme.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::pages::overview_lifecycle::CoreControlCaption;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::theme::{TokenColor, contrast};

fn over(foreground: Color, background: Color) -> Color {
    let foreground = foreground.to_linear();
    let background = background.to_linear();
    let remainder = 1.0 - foreground.alpha;
    Color::linear_rgba(
        foreground.red * foreground.alpha + background.red * remainder,
        foreground.green * foreground.alpha + background.green * remainder,
        foreground.blue * foreground.alpha + background.blue * remainder,
        1.0,
    )
}

fn contrast_on(foreground: Color, background: Color) -> f32 {
    let foreground = over(foreground, background).to_srgba();
    let background = background.to_srgba();
    contrast::contrast_ratio(
        TokenColor::rgb(foreground.red, foreground.green, foreground.blue),
        TokenColor::rgb(background.red, background.green, background.blue),
    )
}

#[test]
fn unavailable_core_caption_and_banner_note_remain_readable_after_theme_changes() {
    let mut app = mounted_default();
    let caption = app
        .world_mut()
        .query::<(Entity, &CoreControlCaption)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    for skin in [ThemeSkin::Light, ThemeSkin::Dark] {
        app.world_mut().commands().trigger(ThemeSwitch(skin));
        app.update();
        assert!(app.world().get::<CoreControlCaption>(caption).is_some());
        let banner = app
            .world_mut()
            .query::<(&OverviewStatusCard, &BackgroundColor)>()
            .iter(app.world())
            .next()
            .unwrap()
            .1
            .0;
        let (fill, disabled) = app
            .world_mut()
            .query::<(&CoreControlButton, &BackgroundColor, &ButtonDisabled)>()
            .iter(app.world())
            .map(|(_, fill, disabled)| (fill.0, disabled.0))
            .next()
            .unwrap();
        assert!(
            disabled,
            "a missing host must stay unavailable across appearance changes"
        );
        let ink = app.world().get::<TextColor>(caption).unwrap().0;
        assert!(
            contrast_on(ink, over(fill, banner)) >= 4.5,
            "the real unavailable caption needs AA contrast on its actual composite fill"
        );
        let (_, _, note_ink) = line(app.world_mut(), OverviewLineKind::BannerNote);
        assert!(
            contrast_on(note_ink.0, banner) >= 4.5,
            "the data-origin notice must remain readable on the status banner"
        );
    }
}

/// Triggering `ThemeSwitch` repaints every token-filled page surface
/// (banner, dot, mode chip, stop button, stat chips) from the new palette
/// and keeps every entity id.
#[test]
fn theme_flip_repaints_every_page_surface_in_place() {
    let mut app = mounted_default();
    let ids_before = overview_entity_ids(app.world_mut());

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();

    let (_, banner_fill, _) = card(world);
    assert_eq!(
        banner_fill, light.accent_container,
        "banner fill re-derived from the light tokens"
    );

    let mut dots = world.query::<(Entity, &StatusDot, &BackgroundColor)>();
    let (_, _, dot_fill) = dots.iter(world).next().expect("status dot mounted");
    assert_eq!(dot_fill.0, light.success, "dot restamped");

    let mut chip_inks = world.query::<(Entity, &OverviewModeChip, &BackgroundColor)>();
    let (_, _, chip_fill) = chip_inks.iter(world).next().expect("mode chip mounted");
    assert_eq!(chip_fill.0, light.accent, "mode chip restamped");

    let mut stops = world.query::<(Entity, &CoreControlButton, &BackgroundColor)>();
    let (_, _, stop_fill) = stops.iter(world).next().expect("stop button mounted");
    assert_eq!(
        stop_fill.0, light.surface_elevated,
        "uncomposed lifecycle control stays disabled and follows the theme"
    );

    let mut chips = world.query::<(Entity, &OverviewChip, &BackgroundColor)>();
    for (_, _, fill) in chips.iter(world) {
        assert_eq!(fill.0, light.surface, "stat chip fill follows the theme");
    }

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "the reskin never remounts"
    );
}

/// A `ThemeSwitch` must not paint role ink over state semantics: after the
/// switch and the page's same-frame replay of its last projection, the
/// unavailable banner keeps the danger fill, the state word its readable
/// `on_accent` ink (not the `Display` role's plain ink a bare `apply_theme`
/// restamp would leave behind) and the uplink its success ink — with every
/// entity id intact.
#[test]
fn theme_switch_keeps_the_unavailable_state_inks_and_ids() {
    let dark = UiPalette::new(&Theme::dark());
    let mut app = mounted_app_with(DemoOverviewSource::unavailable());
    let ids_before = overview_entity_ids(app.world_mut());
    let (state_id, state_text, ink) = line(app.world_mut(), OverviewLineKind::State);
    assert_eq!(state_text, "运行出错");
    assert_eq!(ink.0, dark.on_accent, "precondition: the danger ink is on");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "the replay is a restamp, never a remount"
    );
    let (state_id_after, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_id_after, state_id, "the state line keeps its id");
    assert_eq!(state_text, "运行出错", "the verdict survives the switch");
    assert_eq!(
        ink.0, light.on_accent,
        "the state ink stays semantic after the switch"
    );
    assert_ne!(ink.0, light.ink, "role ink must not win over state ink");

    let (_, upload, upload_ink) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 未观测");
    assert_eq!(
        upload_ink.0, light.success,
        "the uplink ink stays the success token"
    );

    let mut accents = world.query::<(&OnAccentText, &TextColor)>();
    for (_, accent) in accents.iter(world) {
        assert_eq!(
            accent.0, light.on_accent,
            "on-accent copy stays readable on its accent fill"
        );
    }

    let (_, fill, stored) = card(world);
    assert_eq!(stored, CoreLifecycle::Failed);
    assert_eq!(
        fill, light.danger,
        "the banner fill re-derives from the new palette"
    );
}

/// Same contract for the stopped state: the state word's dim ink survives
/// the switch instead of snapping to the role's full ink.
#[test]
fn theme_switch_keeps_the_stopped_state_ink_dim() {
    let mut app = mounted_app_with(DemoOverviewSource::stopped());
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "已停止");
    assert_eq!(ink.0, light.ink_dim);
    assert_ne!(ink.0, light.ink);
}

/// A `ThemeSwitch` re-rasterizes the chart under the SAME image handle
/// (chart.rs's write-back contract): entity id stable, asset id stable,
/// pixels re-derived from the new palette.
#[test]
fn theme_flip_rerasterizes_the_chart_in_place() {
    let mut app = mounted_default();
    app.update(); // the frame sync_charts stamps the raster on
    let (plate_id, _) = chart_plate(app.world_mut());
    let handle = app
        .world()
        .get::<ImageNode>(plate_id)
        .expect("chart rasterized on mount")
        .image
        .clone();
    let data_before = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .expect("chart asset")
        .data
        .clone();

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let world = app.world_mut();
    assert!(
        world.get::<ChartPlate>(plate_id).is_some(),
        "the chart keeps its entity id"
    );
    let node = world.get::<ImageNode>(plate_id).expect("the node survives");
    assert_eq!(
        node.image.id(),
        handle.id(),
        "same handle — write-back, never a swap"
    );
    let data_after = world
        .resource::<Assets<Image>>()
        .get(&node.image)
        .expect("chart asset")
        .data
        .clone();
    assert_ne!(data_before, data_after, "the chart inks follow the theme");
}
