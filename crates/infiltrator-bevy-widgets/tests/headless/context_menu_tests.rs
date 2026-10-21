//! Headless tests for the context menu: the official menu primitives own the
//! behaviour (the popup takes the focus, the arrow keys walk the entries, an
//! entry activates) and the official `Popover` owns the placement.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::camera::visibility::VisibilityPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::Messages;
use bevy::ecs::system::{Commands, Res};
use bevy::input::ButtonState;
use bevy::input::InputPlugin;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input_focus::{FocusCause, InputDispatchPlugin, InputFocus};
use bevy::math::Vec2;
use bevy::mesh::MeshPlugin;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextPlugin;
use bevy::ui_widgets::{MenuFocusState, MenuItem, MenuPopup};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::context_menu::{
    ContextMenuCard, ContextMenuEntryId, ContextMenuItem, ContextMenuOutcome, ContextMenuStage,
    ContextMenuState, context_menu_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

/// The headless context-menu composition: the widget kit (the official menu
/// layer included), the input plumbing the official keyboard navigation needs
/// and a primary window to carry the focus.
fn context_menu_app() -> (App, Entity) {
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
    app.init_resource::<ContextMenuState>();
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    (app, window)
}

/// The entries under test.
fn demo_items() -> Vec<ContextMenuItem> {
    vec![
        ContextMenuItem::new("ping", "Ping node"),
        ContextMenuItem::new("delete", "Delete node").destructive(),
        ContextMenuItem::new("retry", "Retry").disabled(),
    ]
}

/// Mount the context menu scene and settle the first frame.
fn mount_context_menu(app: &mut App) {
    let mut state = ContextMenuState::new();
    state.open(demo_items());
    app.add_systems(
        Startup,
        move |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(context_menu_scene(Vec2::new(120.0, 80.0), &state, &palette));
        },
    );
    app.update();
}

/// The card node (the official `MenuPopup` node).
fn card_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut cards = world.query::<(Entity, &ContextMenuCard)>();
    cards.iter(world).next().expect("one context card").0
}

/// The entry rows with their entry id, in scene order.
fn row_entities(app: &mut App) -> Vec<(Entity, String)> {
    let card = card_entity(app);
    let world = app.world_mut();
    let Some(children) = world.get::<Children>(card) else {
        return Vec::new();
    };
    children
        .iter()
        .filter_map(|child| {
            world
                .get::<ContextMenuEntryId>(*child)
                .map(|id| (*child, id.0.clone()))
        })
        .collect()
}

/// The official input focus.
fn focus(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

/// The outcomes the official events produced so far.
fn outcomes(app: &App) -> Vec<ContextMenuOutcome> {
    let messages = app.world().resource::<Messages<ContextMenuOutcome>>();
    let mut found = Vec::new();
    for id in 0..messages.len() {
        if let Some((outcome, _)) = messages.get_message(id) {
            found.push(outcome.clone());
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

#[test]
fn the_context_menu_mounts_the_official_popup_with_its_entries() {
    let (mut app, _window) = context_menu_app();
    mount_context_menu(&mut app);

    let card = card_entity(&mut app);
    assert!(
        app.world().entity(card).get::<MenuPopup>().is_some(),
        "the card is the official popup"
    );
    assert_eq!(
        app.world().entity(card).get::<MenuFocusState>(),
        Some(&MenuFocusState::Open),
        "the official layer opened the popup and took the focus"
    );
    let rows = row_entities(&mut app);
    assert_eq!(rows.len(), 3, "every entry is a row");
    for (entity, _) in &rows {
        assert!(
            app.world().entity(*entity).get::<MenuItem>().is_some(),
            "every entry is an official menu item"
        );
    }
    assert_eq!(
        focus(&app),
        Some(rows[0].0),
        "the official focus landed on the first entry"
    );
}

#[test]
fn the_arrow_keys_walk_the_context_menu_entries() {
    let (mut app, window) = context_menu_app();
    mount_context_menu(&mut app);
    let rows = row_entities(&mut app);

    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(
        focus(&app),
        Some(rows[1].0),
        "ArrowDown walks to the next entry"
    );
    press(&mut app, window, KeyCode::ArrowUp, Key::ArrowUp);
    assert_eq!(focus(&app), Some(rows[0].0), "ArrowUp walks back");
}

#[test]
fn activating_a_context_entry_reports_its_id() {
    let (mut app, window) = context_menu_app();
    mount_context_menu(&mut app);

    press(&mut app, window, KeyCode::Enter, Key::Enter);
    assert_eq!(
        outcomes(&app),
        vec![ContextMenuOutcome::Selected("ping".to_string())],
        "the official activation names the entry"
    );
    assert_eq!(
        app.world().resource::<ContextMenuState>().stage,
        ContextMenuStage::Closed,
        "the official close lands in the typed stage"
    );
}

#[test]
fn a_disabled_context_entry_never_activates() {
    let (mut app, window) = context_menu_app();
    mount_context_menu(&mut app);
    let rows = row_entities(&mut app);

    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    press(&mut app, window, KeyCode::ArrowDown, Key::ArrowDown);
    assert_eq!(
        focus(&app),
        Some(rows[2].0),
        "the disabled entry is reachable, so the unavailable state is visible"
    );

    press(&mut app, window, KeyCode::Enter, Key::Enter);
    assert!(outcomes(&app).is_empty(), "a disabled entry never confirms");
}

#[test]
fn the_focus_leaving_the_context_menu_dismisses_it() {
    let (mut app, _window) = context_menu_app();
    mount_context_menu(&mut app);

    let outside = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(outside, FocusCause::Navigated);
    app.update();
    // The official close is noticed in the frame's PostUpdate and its event
    // lands with the next frame's command flush.
    app.update();

    assert_eq!(
        outcomes(&app),
        vec![ContextMenuOutcome::Dismissed],
        "the click outside dismisses the menu"
    );
    assert_eq!(
        app.world().resource::<ContextMenuState>().stage,
        ContextMenuStage::Closed
    );
}
