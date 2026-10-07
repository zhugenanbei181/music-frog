//! A cleanup review blocks only the Rules page; it never overwrites domain disabled flags.
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_statistics::{RulesStatisticsState, StatisticsControl};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};

#[derive(SystemParam)]
pub struct StatisticsFocus<'w, 's> {
    state: ResMut<'w, RulesStatisticsState>,
    sdk: ResMut<'w, InputFocus>,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
    children: Query<'w, 's, &'static Children>,
    controls: Query<'w, 's, (Entity, &'static StatisticsControl, Has<ModalScrim>)>,
    buttons: Query<'w, 's, (), With<Button>>,
    fields: Query<'w, 's, (Entity, &'static TextField, &'static mut TextFieldFocused)>,
    live: Query<'w, 's, ()>,
}
pub fn sync(mut focus: StatisticsFocus, mut commands: Commands) {
    let open = focus.state.model.confirmation.is_some();
    if open {
        if !focus.state.holding_focus {
            focus.state.previous_focus = focus
                .fields
                .iter()
                .find(|(_, field, focused)| focused.0 && !field.0.is_disabled())
                .map(|(entity, _, _)| entity)
                .or_else(|| focus.sdk.get());
            focus.sdk.clear();
            if let Some((entity, _, _)) = focus.controls.iter().find(|(_, action, scrim)| {
                matches!(action, StatisticsControl::CancelCleanup) && !*scrim
            }) {
                focus.sdk.set(entity, FocusCause::Navigated);
            }
            focus.state.holding_focus = true;
        }
        for page in &focus.pages {
            for entity in focus.children.iter_descendants(page) {
                if focus.buttons.contains(entity) || focus.fields.contains(entity) {
                    if !focus.state.blocked.contains(&entity) {
                        commands.entity(entity).insert(InteractionBlocked);
                        focus.state.blocked.push(entity);
                    }
                    if let Ok((_, _, mut focused)) = focus.fields.get_mut(entity) {
                        focused.0 = false;
                    }
                }
            }
        }
        if focus
            .sdk
            .get()
            .is_some_and(|entity| !focus.controls.contains(entity))
        {
            focus.sdk.clear();
        }
    } else if focus.state.holding_focus {
        for entity in focus.state.blocked.drain(..) {
            if focus.live.contains(entity) {
                commands.entity(entity).remove::<InteractionBlocked>();
            }
        }
        focus.state.holding_focus = false;
        focus.sdk.clear();
        if let Some(entity) = focus
            .state
            .previous_focus
            .take()
            .filter(|entity| focus.live.contains(*entity))
        {
            focus.sdk.set(entity, FocusCause::Navigated);
            if let Ok((_, field, mut focused)) = focus.fields.get_mut(entity) {
                focused.0 = !field.0.is_disabled();
            }
        }
    }
}
