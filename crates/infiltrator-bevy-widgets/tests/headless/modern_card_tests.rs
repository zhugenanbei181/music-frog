//! Headless tests for ModernCard: the typed state ladder resolves the paint
//! (hairline inner stroke plus a Level 1 shadow at rest, a deeper shadow, a
//! -1px micro-lift and a highlight wash under the pointer, the accent stroke
//! on the marked card), and the mounted card repaints in place — the same
//! entity before and after, no remount of the scene tree.

use super::support::headless_app;
use bevy::app::Startup;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Res};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{BackgroundColor, BorderColor, Node, Val};
use bevy::ui::{UiTransform, Val2};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::modern_card::{
    ModernCard, ModernCardMarked, ModernCardState, modern_card_paint, modern_card_scene,
    resolve_modern_card_state,
};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::theme::Theme;

fn content_scene() -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {}
    })
}

#[test]
fn the_state_ladder_resolves_every_fact_combination() {
    assert_eq!(
        resolve_modern_card_state(false, Some(PickingInteraction::Pressed)),
        ModernCardState::Pressed
    );
    assert_eq!(
        resolve_modern_card_state(false, Some(PickingInteraction::Hovered)),
        ModernCardState::Hovered
    );
    assert_eq!(
        resolve_modern_card_state(false, Some(PickingInteraction::None)),
        ModernCardState::Resting
    );
    assert_eq!(
        resolve_modern_card_state(false, None),
        ModernCardState::Resting
    );
    assert_eq!(
        resolve_modern_card_state(true, None),
        ModernCardState::Marked
    );
    assert_eq!(
        resolve_modern_card_state(true, Some(PickingInteraction::Hovered)),
        ModernCardState::MarkedHovered
    );
    assert_eq!(
        resolve_modern_card_state(true, Some(PickingInteraction::Pressed)),
        ModernCardState::MarkedPressed
    );
}

#[test]
fn the_resting_card_carries_the_level_one_shadow_but_no_lift() {
    let palette = UiPalette::new(&Theme::dark());
    let paint = modern_card_paint(ModernCardState::Resting, &palette);
    assert_eq!(paint.fill, palette.surface_elevated);
    assert_eq!(paint.border, palette.border);
    assert_eq!(paint.lift, None);
    assert!(!paint.shadow.0.is_empty());

    let hovered = modern_card_paint(ModernCardState::Hovered, &palette);
    // The hovered shadow is deeper than the resting Level 1 one.
    assert_ne!(hovered.shadow.0, paint.shadow.0);
    assert_eq!(hovered.lift, Some(Val2::new(Val::ZERO, rem(-1.0))));
    // The highlight is the hover wash composited over the card surface.
    assert_eq!(hovered.fill, palette.over_surface(palette.hover_bg));
}

#[test]
fn the_marked_card_keeps_its_reading_surface_and_rides_the_accent_stroke() {
    let palette = UiPalette::new(&Theme::dark());
    let paint = modern_card_paint(ModernCardState::Marked, &palette);
    assert_eq!(paint.fill, palette.surface_elevated);
    assert_eq!(paint.border, palette.accent);
    assert_eq!(paint.lift, None);

    let pressed = modern_card_paint(ModernCardState::MarkedPressed, &palette);
    assert_eq!(pressed.fill, palette.pressed_bg);
}

#[test]
fn the_mounted_card_repaints_in_place_under_the_pointer() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(modern_card_scene(content_scene(), &palette));
        },
    );
    app.update();

    let palette = *app.world_mut().resource::<UiPalette>();

    let card = {
        let world = app.world_mut();
        let mut cards = world.query::<Entity, With<ModernCard>>();
        let Some(card) = cards.iter(world).next() else {
            panic!("the modern card scene mounted no ModernCard entity");
        };
        card
    };

    let resting = {
        let world = app.world_mut();
        let mut query = world.query::<(&BackgroundColor, &BorderColor, &UiTransform)>();
        let (fill, border, transform) = query.get(world, card).expect("card visuals");
        assert_eq!(fill.0, palette.surface_elevated);
        assert_eq!(border.top, palette.border);
        assert_eq!(transform.translation, Val2::ZERO);
        fill.0
    };

    // The pointer arrives: same entity, repaint in place.
    app.world_mut()
        .commands()
        .entity(card)
        .insert(PickingInteraction::Hovered);
    app.update();

    {
        let world = app.world_mut();
        let mut query = world.query::<(&BackgroundColor, &UiTransform)>();
        let (fill, transform) = query.get(world, card).expect("card visuals");
        assert_eq!(fill.0, palette.over_surface(palette.hover_bg));
        assert_ne!(fill.0, resting);
        assert_eq!(transform.translation, Val2::new(Val::ZERO, rem(-1.0)));
    }

    // The owner marks the card: the accent rides the stroke, the wash stays.
    app.world_mut()
        .commands()
        .entity(card)
        .insert(ModernCardMarked(true));
    app.update();

    {
        let world = app.world_mut();
        let mut query = world.query::<(&BackgroundColor, &BorderColor)>();
        let (fill, border) = query.get(world, card).expect("card visuals");
        assert_eq!(fill.0, palette.over_surface(palette.hover_bg));
        assert_eq!(border.top, palette.accent);
    }

    // A disabled card never reacts to the pointer: the paint freezes even
    // though the interaction component is still the hovered one.
    app.world_mut()
        .commands()
        .entity(card)
        .insert(ButtonDisabled(true));
    app.update();

    {
        let world = app.world_mut();
        let mut query = world.query::<&BackgroundColor>();
        let fill = query.get(world, card).expect("card visuals");
        assert_eq!(fill.0, palette.over_surface(palette.hover_bg));
    }
}
