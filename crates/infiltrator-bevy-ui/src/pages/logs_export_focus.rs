//! The modal suspends only its native logs input; retired entities are never reactivated.
use crate::pages::logs_export::{LogsExportAction, LogsExportState};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Has;
use bevy::ecs::system::{Query, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
#[derive(SystemParam)]
pub struct LogExportFocus<'w, 's> {
    sdk: ResMut<'w, InputFocus>,
    actions: Query<'w, 's, (Entity, &'static LogsExportAction, Has<ModalScrim>)>,
    live: Query<'w, 's, ()>,
    fields: Query<
        'w,
        's,
        (
            Entity,
            &'static NativeTextField,
            &'static mut TextField,
            &'static mut TextFieldFocused,
        ),
    >,
}
impl LogExportFocus<'_, '_> {
    pub fn sync(&mut self, state: &mut LogsExportState) {
        if state.model.open && !state.holding_focus {
            state.previous_sdk_focus = self.sdk.get();
            self.sdk.clear();
            if let Some((entity, _, _)) = self
                .actions
                .iter()
                .find(|(_, action, scrim)| !*scrim && matches!(action, LogsExportAction::Cancel))
            {
                self.sdk.set(entity, FocusCause::Navigated);
            }
            for (entity, marker, mut field, mut focused) in &mut self.fields {
                if marker.0 == 11 {
                    state
                        .held_fields
                        .push((entity, field.0.is_disabled(), focused.0));
                    field.0.set_disabled(true);
                    focused.0 = false;
                }
            }
            state.holding_focus = true;
        } else if !state.model.open && state.holding_focus {
            for (entity, disabled, focused) in state.held_fields.drain(..) {
                if let Ok((_, marker, mut field, mut focus)) = self.fields.get_mut(entity)
                    && marker.0 == 11
                {
                    field.0.set_disabled(disabled);
                    focus.0 = focused && !disabled;
                }
            }
            state.holding_focus = false;
            self.sdk.clear();
            if let Some(entity) = state
                .previous_sdk_focus
                .take()
                .filter(|entity| self.live.contains(*entity))
            {
                self.sdk.set(entity, FocusCause::Navigated);
            }
        } else if state.model.open
            && self
                .sdk
                .get()
                .is_some_and(|entity| !self.actions.contains(entity))
        {
            self.sdk.clear();
        }
    }
}
