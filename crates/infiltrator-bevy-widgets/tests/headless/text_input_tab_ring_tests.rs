//! test-intent: behavior
//! The native field tab ring is scoped, visible and painted — never world-wide.
//!
//! `NativeTextField(n)` used to be compared across *every* field in the world,
//! so two fields that both declared `0` — the DNS page's search box and the
//! hosts dialog's address box — produced a ring whose sequence was settled by
//! archetype storage order: Tab from the dialog's address field landed on
//! whichever search field the ECS happened to enumerate first, and the domain
//! field became unreachable by keyboard. The widget layer owns that answer
//! (`text_input::native::NativeTabRing`); these cases are its acceptance.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Res};
use bevy::input::ButtonInput;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::scene::{CommandsSceneExt, Scene, ScenePlugin, bsn};
use bevy::ui::prelude::{AlignItems, Display, FlexDirection, Node};
use bevy::ui_widgets::ModalDialog;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::modal::ModalDialogCard;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextFieldFocused, text_field_scene};
use infiltrator_bevy_widgets::theme::Theme;

/// A field outside any dialog (the page's own search box).
#[derive(Component, Clone, Copy, Debug, Default)]
struct PageField;
/// A field inside the presented dialog, whose declared number collides with `PageField`.
#[derive(Component, Clone, Copy, Debug, Default)]
struct DialogAddress;
/// The dialog's second field.
#[derive(Component, Clone, Copy, Debug, Default)]
struct DialogDomain;
/// A field the surface keeps mounted but hidden (`Display::None` branch).
#[derive(Component, Clone, Copy, Debug, Default)]
struct HiddenField;
/// The second of three colliding page-level fields (paint-order case).
#[derive(Component, Clone, Copy, Debug, Default)]
struct SecondField;
/// The third of three colliding page-level fields (paint-order case).
#[derive(Component, Clone, Copy, Debug, Default)]
struct ThirdField;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

/// The page: one search field, one hidden field, and a *presented* hosts-style
/// dialog whose two fields declare `0` and `1` — the same number the page's own
/// search field declares.
fn page_with_open_dialog_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
            }
            Children [
                @{ (text_field_scene(String::new(), palette), bsn! { PageField NativeTextField(0) }) }
                --
                Node {
                    display: Display::None,
                    align_items: AlignItems::Start,
                }
                Children [
                    @{ (text_field_scene(String::new(), palette), bsn! { HiddenField NativeTextField(3) }) }
                ]
                --
                Node {
                    align_items: AlignItems::Start,
                }
                ModalDialogCard
                ModalDialog
                Children [
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Start,
                    }
                    Children [
                        @{ (text_field_scene(String::new(), palette), bsn! { DialogAddress NativeTextField(0) }) }
                        --
                        @{ (text_field_scene(String::new(), palette), bsn! { DialogDomain NativeTextField(1) }) }
                    ]
                ]
            ]
    }
}

/// Three sibling fields that all declare `0`, so the ring can only order them
/// by where they paint.
fn three_colliding_fields_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
            }
            Children [
                @{ (text_field_scene(String::new(), palette), bsn! { PageField NativeTextField(0) }) }
                --
                @{ (text_field_scene(String::new(), palette), bsn! { SecondField NativeTextField(0) }) }
                --
                @{ (text_field_scene(String::new(), palette), bsn! { ThirdField NativeTextField(0) }) }
            ]
    }
}

fn entity_with<C: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<C>>()
        .single(world)
        .expect("the marker is mounted")
}

fn focused(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut marks = world.query::<(Entity, &TextFieldFocused)>();
    marks
        .iter(world)
        .filter(|(_, have)| have.0)
        .map(|(entity, _)| entity)
        .collect()
}

/// Put the native key owner's focus on one field, the way a pointer press does.
fn focus(app: &mut App, field: Entity) {
    let world = app.world_mut();
    {
        let mut marks = world.query::<&mut TextFieldFocused>();
        for mut have in marks.iter_mut(world) {
            have.0 = false;
        }
    }
    world
        .get_mut::<TextFieldFocused>(field)
        .unwrap_or_else(|| panic!("{field:?} is not a field"))
        .0 = true;
    app.update();
}

/// One Tab stroke, delivered the way the widget layer's key owner reads it.
fn tab(app: &mut App, shift: bool) {
    let mut keys = ButtonInput::<KeyCode>::default();
    if shift {
        keys.press(KeyCode::ShiftLeft);
    }
    app.insert_resource(keys);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Tab,
        logical_key: Key::Tab,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

/// A presented dialog owns its fields: tabbing inside it walks the dialog's own
/// sequence and never leaks to the page behind it.
#[test]
fn a_presented_dialog_scopes_the_ring_to_its_own_fields() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(page_with_open_dialog_scene(&palette));
        },
    );
    app.update();
    app.update();

    let address = entity_with::<DialogAddress>(&mut app);
    let domain = entity_with::<DialogDomain>(&mut app);
    let page = entity_with::<PageField>(&mut app);

    focus(&mut app, address);
    tab(&mut app, false);
    assert_eq!(
        focused(&mut app),
        vec![domain],
        "inside a presented dialog the declared 0/1 order the dialog, so the next stop is its own second field"
    );

    tab(&mut app, true);
    assert_eq!(
        focused(&mut app),
        vec![address],
        "shift-tab walks the same scoped ring back"
    );

    focus(&mut app, domain);
    tab(&mut app, false);
    let now = focused(&mut app);
    assert_eq!(
        now,
        vec![address],
        "the dialog's ring wraps inside the dialog"
    );
    assert!(
        !now.contains(&page),
        "the page behind a presented dialog keeps no tab stop"
    );
}

/// Outside a dialog the ring is the visible, non-dialog fields: a
/// `Display::None` branch leaves it, so a hidden card cannot steal a stop, and
/// the dialog's own fields stay the dialog's business.
#[test]
fn a_hidden_branch_and_a_presented_dialog_leave_the_page_ring() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(page_with_open_dialog_scene(&palette));
        },
    );
    app.update();
    app.update();

    let page = entity_with::<PageField>(&mut app);
    let hidden = entity_with::<HiddenField>(&mut app);
    let address = entity_with::<DialogAddress>(&mut app);
    let domain = entity_with::<DialogDomain>(&mut app);

    focus(&mut app, page);
    tab(&mut app, false);
    let now = focused(&mut app);
    assert_eq!(
        now,
        vec![page],
        "the page's ring is its visible non-dialog fields, so the stroke wraps onto itself"
    );
    for absent in [hidden, address, domain] {
        assert!(
            !now.contains(&absent),
            "{absent:?} must hold no focus from outside the dialog"
        );
    }
}

/// Ties on the declared number resolve by paint order, not by archetype storage
/// order, so the sequence is the same in every run.
#[test]
fn duplicate_declared_numbers_resolve_in_painted_order() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(three_colliding_fields_scene(&palette));
        },
    );
    app.update();
    app.update();

    let first = entity_with::<PageField>(&mut app);
    let second = entity_with::<SecondField>(&mut app);
    let third = entity_with::<ThirdField>(&mut app);

    focus(&mut app, first);
    tab(&mut app, false);
    assert_eq!(
        focused(&mut app),
        vec![second],
        "the next painted row answers the stroke"
    );
    tab(&mut app, false);
    assert_eq!(focused(&mut app), vec![third], "and the one after that");
    tab(&mut app, false);
    assert_eq!(
        focused(&mut app),
        vec![first],
        "the ring wraps in painted order"
    );
}
