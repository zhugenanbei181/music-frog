//! Temporary native interaction suppression does not overwrite a control's domain disabled state.
use crate::text_input::TextField;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::system::{Commands, Query};
use bevy::ui::InteractionDisabled;

#[derive(Component, Clone, Copy, Default)]
pub struct InteractionBlocked;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct InputInteractionState {
    entity: Entity,
    field: &'static TextField,
    blocked: Has<InteractionBlocked>,
    sdk_disabled: Has<InteractionDisabled>,
    accessibility: &'static mut AccessibilityNode,
}
pub fn sync_inputs(mut commands: Commands, mut fields: Query<InputInteractionState>) {
    for mut field in &mut fields {
        let disabled = field.field.0.is_disabled() || field.blocked;
        if disabled != field.sdk_disabled {
            if disabled {
                commands.entity(field.entity).insert(InteractionDisabled);
            } else {
                commands
                    .entity(field.entity)
                    .remove::<InteractionDisabled>();
            }
        }
        if field.accessibility.is_disabled() != disabled {
            if disabled {
                field.accessibility.set_disabled();
            } else {
                field.accessibility.clear_disabled();
            }
        }
    }
}
