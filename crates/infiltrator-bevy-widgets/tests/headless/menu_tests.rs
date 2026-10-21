//! Headless tests for the menu: the official headless menu primitives own
//! the behaviour (the popup opens from its trigger, the arrow keys walk the
//! entries, an entry activates, Escape dismisses and the focus returns to
//! the trigger), the typed outcome is derived from the official events, and
//! the token skin repaints in place.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::camera::visibility::VisibilityPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Messages;
use bevy::ecs::system::{Commands, Res};
use bevy::input::ButtonState;
use bevy::input::InputPlugin;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{FocusCause, InputDispatchPlugin, InputFocus};
use bevy::mesh::MeshPlugin;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextPlugin;
use bevy::ui::BackgroundColor;
use bevy::ui::prelude::{Display, Node};
use bevy::ui_widgets::{Activate, MenuFocusState};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::menu::{
    MenuEntry, MenuOutcome, MenuPanel, MenuRowIndex, MenuTrigger, menu_row_fill, menu_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};

/// The headless menu composition: the widget kit (the official menu layer
/// included), the input plumbing the official keyboard navigation needs, and
/// a primary window to carry the focus.
fn menu_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, TextPlugin));
    app.add_plugins((InputPlugin, InputDispatchPlugin));
    // The official focus bookkeeping only reaches the entries of a visible
    // popup, so the headless composition resolves the inherited visibility
    // like the windowed host does.
    app.add_plugins(MeshPlugin);
    app.add_plugins(VisibilityPlugin);
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    (app, window)
}

/// The menu under test: three entries (one of them disabled) around a
/// separator.
fn demo_entries() -> Vec<MenuEntry> {
    vec![
        MenuEntry::item("switch mode"),
        MenuEntry::Separator,
        MenuEntry::disabled("restart core"),
        MenuEntry::item("open logs"),
    ]
}

/// Mount the menu scene and settle the first frame.
fn mount_menu(app: &mut App, entries: Vec<MenuEntry>) {
    app.add_systems(
        Startup,
        move |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(menu_scene("menu".to_string(), entries.clone(), &palette));
        },
    );
    app.update();
}

/// The trigger node (the official `MenuButton`).
fn trigger_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut triggers = world.query::<(Entity, &MenuTrigger)>();
    triggers.iter(world).next().expect("one menu trigger").0
}

/// The popup node (the official `MenuPopup`).
fn panel_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut panels = world.query::<(Entity, &MenuPanel)>();
    panels.iter(world).next().expect("one menu panel").0
}

/// The item rows with their entry index, in scene order.
fn row_entities(app: &mut App) -> Vec<(Entity, usize)> {
    let world = app.world_mut();
    let mut rows = world.query::<(Entity, &MenuRowIndex)>();
    let mut found: Vec<(Entity, usize)> = rows
        .iter(world)
        .map(|(entity, index)| (entity, index.0))
        .collect();
    found.sort_by_key(|(_, index)| *index);
    found
}

/// The official input focus.
fn focus(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

/// The outcomes the official events produced so far.
fn outcomes(app: &App) -> Vec<MenuOutcome> {
    let messages = app.world().resource::<Messages<MenuOutcome>>();
    let mut found = Vec::new();
    for id in 0..messages.len() {
        if let Some((outcome, _)) = messages.get_message(id) {
            found.push(*outcome);
        }
    }
    found
}

/// Send one key press to the focused entity and settle the frame.
fn press(app: &mut App, window: Entity, code: KeyCode, logical: Key) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
}

/// Activate the trigger the way the official button does, then settle the
/// frame.
fn open_menu(app: &mut App) {
    let trigger = trigger_entity(app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: trigger });
    app.update();
}

#[test]
fn the_popup_is_mounted_closed_and_hidden() {
    let (mut app, _window) = menu_app();
    mount_menu(&mut app, demo_entries());

    let popup = panel_entity(&mut app);
    assert_eq!(
        app.world().entity(popup).get::<MenuFocusState>(),
        Some(&MenuFocusState::Closed),
        "the popup starts closed: the official state is the truth"
    );
    assert_eq!(
        app.world()
            .entity(popup)
            .get::<Node>()
            .map(|node| node.display),
        Some(Display::None),
        "the closed popup takes no room in the layout"
    );
    assert!(outcomes(&app).is_empty(), "a closed menu reports nothing");
}

#[test]
fn the_official_popup_opens_from_the_trigger_and_takes_the_focus() {
    let (mut app, _window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);

    let popup = panel_entity(&mut app);
    assert_eq!(
        app.world().entity(popup).get::<MenuFocusState>(),
        Some(&MenuFocusState::Open),
        "the official layer opened the popup"
    );
    assert_eq!(
        app.world()
            .entity(popup)
            .get::<Node>()
            .map(|node| node.display),
        Some(Display::Flex),
        "the open popup is visible"
    );
    let rows = row_entities(&mut app);
    assert_eq!(
        focus(&app),
        Some(rows[0].0),
        "the official focus landed on the first entry"
    );
}

#[test]
fn the_arrow_keys_walk_the_entries_by_the_official_navigation() {
    let (mut app, window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);
    let rows = row_entities(&mut app);
    assert_eq!(rows.len(), 3, "the separator is not an entry row");

    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(
        focus(&app),
        Some(rows[1].0),
        "ArrowDown walks to the next entry"
    );
    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(
        focus(&app),
        Some(rows[2].0),
        "ArrowDown walks past the disabled entry to the last one"
    );
    press(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp);
    assert_eq!(focus(&app), Some(rows[1].0), "ArrowUp walks back");
    press(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp);
    assert_eq!(
        focus(&app),
        Some(rows[0].0),
        "ArrowUp walks to the first entry"
    );
    press(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp);
    assert_eq!(
        focus(&app),
        Some(rows[2].0),
        "the official navigation cycles around the entries"
    );
    assert!(
        outcomes(&app).is_empty(),
        "walking the entries decides nothing"
    );
}

#[test]
fn enter_activates_the_focused_entry_and_reports_the_outcome() {
    let (mut app, window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);
    let trigger = trigger_entity(&mut app);
    let rows = row_entities(&mut app);

    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(focus(&app), Some(rows[2].0));

    press(&mut app, window, KeyCode::Enter, Key::Enter);
    assert_eq!(
        outcomes(&app),
        vec![MenuOutcome::Confirmed(3)],
        "the official activation names the entry index"
    );
    assert_eq!(
        focus(&app),
        Some(trigger),
        "the focus goes back to the trigger"
    );
    let popup = panel_entity(&mut app);
    assert_eq!(
        app.world().entity(popup).get::<MenuFocusState>(),
        Some(&MenuFocusState::Closed),
        "the official layer closed the popup"
    );
}

#[test]
fn escape_dismisses_the_menu_and_hands_the_focus_back_to_the_trigger() {
    let (mut app, window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);
    let trigger = trigger_entity(&mut app);

    press(&mut app, window, KeyCode::Escape, Key::Escape);
    assert_eq!(
        outcomes(&app),
        vec![MenuOutcome::Canceled],
        "a dismissal without an activation cancels"
    );
    assert_eq!(
        focus(&app),
        Some(trigger),
        "the focus goes back to the trigger"
    );
    let popup = panel_entity(&mut app);
    assert_eq!(
        app.world().entity(popup).get::<MenuFocusState>(),
        Some(&MenuFocusState::Closed)
    );
}

#[test]
fn the_focus_leaving_the_menu_dismisses_it() {
    let (mut app, _window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);

    // The official layer watches the focus: leaving the popup closes it.
    let outside = app.world_mut().spawn((TabIndex(0),)).id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(outside, FocusCause::Navigated);
    app.update();
    // The official close is noticed in the frame's PostUpdate and its event
    // lands with the next frame's command flush.
    app.update();

    assert_eq!(
        outcomes(&app),
        vec![MenuOutcome::Canceled],
        "the click outside cancels the menu"
    );
    let popup = panel_entity(&mut app);
    assert_eq!(
        app.world().entity(popup).get::<MenuFocusState>(),
        Some(&MenuFocusState::Closed)
    );
}

#[test]
fn a_disabled_entry_is_reachable_but_never_activates() {
    let (mut app, window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);
    let rows = row_entities(&mut app);

    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(
        focus(&app),
        Some(rows[1].0),
        "the disabled entry is reachable, so the unavailable state is visible"
    );

    press(&mut app, window, KeyCode::Enter, Key::Enter);
    assert!(outcomes(&app).is_empty(), "a disabled entry never confirms");
    assert_eq!(
        focus(&app),
        Some(rows[1].0),
        "the disabled entry keeps the focus"
    );
}

#[test]
fn the_theme_flip_repaints_the_menu_without_a_respawn() {
    let (mut app, _window) = menu_app();
    mount_menu(&mut app, demo_entries());
    open_menu(&mut app);

    let popup = panel_entity(&mut app);
    let rows = row_entities(&mut app);
    let dark = UiPalette::new(&Theme::dark());
    assert_eq!(
        app.world().entity(rows[0].0).get::<BackgroundColor>(),
        Some(&BackgroundColor(dark.hover_bg)),
        "the focused entry paints the hover token"
    );

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    assert_eq!(
        app.world().entity(popup).get::<BackgroundColor>(),
        Some(&BackgroundColor(light.surface)),
        "the panel re-derives from the light palette"
    );
    assert_eq!(
        app.world().entity(rows[0].0).get::<BackgroundColor>(),
        Some(&BackgroundColor(light.hover_bg)),
        "the focused entry re-derives from the light palette"
    );
    assert_eq!(
        menu_row_fill(true, &light),
        light.hover_bg,
        "the highlight vocabulary is the hover token"
    );
}
