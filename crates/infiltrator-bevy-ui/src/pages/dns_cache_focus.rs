//! Modal focus uses only controlled input queries; it cannot mutate unrelated scene state.
use crate::pages::dns_cache::CacheConfirmation;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Query, SystemParam};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
#[derive(SystemParam)]
pub struct CacheFocus<'w, 's> {
    fields: Query<'w, 's, (Entity, &'static TextField, &'static mut TextFieldFocused)>,
}
impl CacheFocus<'_, '_> {
    pub fn suspend(&mut self, state: &mut CacheConfirmation) {
        if !state.restore_focus.is_empty() {
            return;
        }
        for (entity, _, mut focus) in &mut self.fields {
            if focus.0 {
                state.restore_focus.push(entity);
                focus.0 = false;
            }
        }
    }
    pub fn restore(&mut self, state: &mut CacheConfirmation) {
        for entity in state.restore_focus.drain(..) {
            if let Ok((_, field, mut focused)) = self.fields.get_mut(entity)
                && !field.0.is_disabled()
            {
                focused.0 = true;
            }
        }
    }
}
