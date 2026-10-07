//! test-intent: behavior
//! Actual SDK pointer and ordinary keyboard events for isolated native surfaces.
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use std::time::Duration;
fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::None {
                width: 1180,
                height: 780,
            },
            position: Vec2::new(20.0, 20.0),
        },
    )
}
pub fn press(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.update();
}
pub fn click_entity(app: &mut App, entity: Entity) {
    press(app, entity);
    app.world_mut().trigger(PointerClick {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
        duration: Duration::from_millis(100),
    });
    app.update();
}
/// Deliver a complete SDK gesture without advancing the frame between controls.
pub fn click_before_frame(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    // Commit the SDK Pressed component before its click observer queries it.
    // This is test assembly; production schedules own their deferred boundaries.
    app.world_mut().flush();
    app.world_mut().trigger(PointerClick {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
        duration: Duration::from_millis(100),
    });
    app.world_mut().flush();
}
pub fn type_text(app: &mut App, value: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyM,
        logical_key: Key::Character(value.into()),
        text: Some(value.into()),
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

pub fn replace_text(app: &mut App, value: &str) {
    if !app.world().contains_resource::<ButtonInput<KeyCode>>() {
        app.init_resource::<ButtonInput<KeyCode>>();
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    type_text(app, "a");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ControlLeft);
    type_text(app, value);
}
