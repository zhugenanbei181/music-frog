//! Restricted component replay and native interaction gates preserve background editing state.
use crate::pages::snapshot_restore::RestoreState;
use crate::pages::snapshot_restore_scene::{
    RestoreControl, RestoreLine, RestoreOverlay, ReviewControl,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::text::EditableText;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Button;
use infiltrator_application::snapshot_restore_projection::project_restore;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::TextField;

#[derive(QueryFilter)]
pub struct BackgroundFilter {
    controls: Or<(With<Button>, With<EditableText>, With<TextField>)>,
}
#[derive(QueryData)]
pub struct BackgroundControl {
    entity: Entity,
    review: Has<ReviewControl>,
    blocked: Has<InteractionBlocked>,
}
#[derive(SystemParam)]
pub struct RestoreView<'w, 's> {
    state: ResMut<'w, RestoreState>,
    locale: Res<'w, UiLocale>,
    focus: ResMut<'w, InputFocus>,
    overlays: Query<'w, 's, &'static mut Node, With<RestoreOverlay>>,
    labels: Query<'w, 's, (&'static RestoreLine, &'static mut Text)>,
    controls: Query<'w, 's, (&'static RestoreControl, &'static mut ButtonDisabled)>,
    backgrounds: Query<'w, 's, BackgroundControl, BackgroundFilter>,
    live: Query<'w, 's, ()>,
}
pub fn replay(mut view: RestoreView, mut commands: Commands) {
    let open = view.state.model.visible;
    let presentation = project_restore(&view.state.model, view.locale.code());
    for mut node in &mut view.overlays {
        node.display = if open { Display::Flex } else { Display::None };
    }
    for (role, mut text) in &mut view.labels {
        let copy = match role {
            RestoreLine::Status => &presentation.status,
            RestoreLine::Details => &presentation.details,
            RestoreLine::Content => &presentation.content,
        };
        if text.0 != *copy {
            text.0.clone_from(copy);
        }
    }
    for (control, mut disabled) in &mut view.controls {
        disabled.0 = match control {
            RestoreControl::Confirm => !presentation.confirm,
            RestoreControl::Cancel => !presentation.cancel,
            RestoreControl::Retry => !presentation.retry,
        };
    }
    if open {
        if view.state.previous_focus.is_none() {
            view.state.previous_focus = view.focus.get();
        }
        for target in &view.backgrounds {
            if target.review || target.blocked {
                continue;
            }
            commands.entity(target.entity).insert(InteractionBlocked);
            view.state.blocked.push(target.entity);
        }
        if view.focus.get().is_some_and(|entity| {
            view.backgrounds
                .get(entity)
                .is_ok_and(|target| !target.review)
        }) {
            view.focus.clear();
        }
    } else {
        for entity in view.state.blocked.drain(..) {
            if view.live.contains(entity) {
                commands.entity(entity).remove::<InteractionBlocked>();
            }
        }
        if let Some(entity) = view
            .state
            .previous_focus
            .take()
            .filter(|entity| view.live.contains(*entity))
        {
            view.focus.set(entity, FocusCause::Navigated);
        }
    }
}
