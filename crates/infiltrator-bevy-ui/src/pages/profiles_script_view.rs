//! Locale replay and a scoped modal boundary preserve the actual editor entities.
use crate::pages::profiles_script_scene::{
    ScriptExportOverlay, ScriptResultBody, ScriptResultRow, ScriptReviewControl, ScriptReviewLine,
};
use crate::pages::profiles_script_workbench::{ScriptControl, ScriptInput, ScriptWorkbenchState};
use crate::route::{ActiveRoute, Route};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::scene::{CommandsSceneExt, bsn};
use bevy::text::EditableText;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node, percent};
use bevy::ui_widgets::Button;
use infiltrator_application::script_console_projection::report_rows;
use infiltrator_application::script_export_projection::project_script_export;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::multiline_editor::{MultilineEditor, MultilineReadOnly};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ControlState {
    entity: Entity,
    control: &'static ScriptControl,
    review: Has<ScriptReviewControl>,
    disabled: Option<&'static mut ButtonDisabled>,
}
#[derive(QueryData)]
pub struct BackgroundControl {
    entity: Entity,
    button: Has<Button>,
    editor: Has<MultilineEditor>,
    single_line: Has<TextField>,
    review: Has<ScriptReviewControl>,
}
#[derive(QueryFilter)]
pub struct BackgroundFilter {
    controls: Or<(With<Button>, With<MultilineEditor>, With<TextField>)>,
}
#[derive(SystemParam)]
pub struct ScriptView<'w, 's> {
    state: ResMut<'w, ScriptWorkbenchState>,
    route: Res<'w, ActiveRoute>,
    locale: Res<'w, UiLocale>,
    focus: ResMut<'w, InputFocus>,
    overlays: Query<'w, 's, &'static mut Node, With<ScriptExportOverlay>>,
    review_texts:
        Query<'w, 's, (&'static ScriptReviewLine, &'static mut Text), Without<ScriptResultRow>>,
    controls: Query<'w, 's, ControlState>,
    fields: Query<
        'w,
        's,
        (
            Entity,
            &'static mut MultilineReadOnly,
            &'static EditableText,
        ),
        With<ScriptInput>,
    >,
    legacy_focus: Query<'w, 's, &'static mut TextFieldFocused>,
    backgrounds: Query<'w, 's, BackgroundControl, BackgroundFilter>,
    live: Query<'w, 's, ()>,
    bodies: Query<'w, 's, Entity, With<ScriptResultBody>>,
    children: Query<'w, 's, &'static Children>,
    rows: Query<'w, 's, (&'static ScriptResultRow, &'static mut Text), Without<ScriptReviewLine>>,
}
pub fn render(mut view: ScriptView, mut commands: Commands) {
    let open = view.state.model.export_visible && view.route.0 == Some(Route::Profiles);
    if !open
        && let Some(entity) = view
            .focus
            .get()
            .filter(|entity| view.fields.contains(*entity))
    {
        view.state.last_editor_focus = Some(entity);
    }
    let presentation = project_script_export(&view.state.model, view.locale.code());
    for mut node in &mut view.overlays {
        node.display = if open { Display::Flex } else { Display::None };
    }
    for (line, mut text) in &mut view.review_texts {
        let value = match line {
            ScriptReviewLine::Status => &presentation.status,
            ScriptReviewLine::Details => &presentation.details,
            ScriptReviewLine::Content => &presentation.content,
            ScriptReviewLine::Path => &presentation.path,
        };
        if text.0 != *value {
            text.0 = value.clone();
        }
    }
    let composing = view.fields.iter().any(|(_, _, field)| field.is_composing());
    for (_, mut field, _) in &mut view.fields {
        field.0 = view.state.model.busy();
    }
    for mut control in &mut view.controls {
        let enabled = !composing
            && if control.review {
                match control.control {
                    ScriptControl::Confirm => presentation.confirm,
                    ScriptControl::Cancel => presentation.close,
                    ScriptControl::Retry => presentation.retry,
                    _ => false,
                }
            } else {
                !open
                    && !view.state.model.busy()
                    && (!matches!(control.control, ScriptControl::Retry)
                        || view.state.model.can_retry())
            };
        if let Some(disabled) = control.disabled.as_deref_mut()
            && disabled.0 == enabled
        {
            disabled.0 = !enabled;
        }
    }
    if open {
        if !view.state.holding_focus {
            view.state.holding_focus = true;
            view.state.previous_focus = view.state.last_editor_focus.or_else(|| view.focus.get());
            view.focus.clear();
            if let Some(control) = view.controls.iter().find(|control| {
                control.review
                    && matches!(control.control, ScriptControl::Cancel)
                    && control.disabled.is_some()
            }) {
                view.focus.set(control.entity, FocusCause::Navigated);
            }
        }
        for mut focused in &mut view.legacy_focus {
            focused.0 = false;
        }
        for target in &view.backgrounds {
            if (target.button || target.editor || target.single_line)
                && !target.review
                && !view.state.blocked.contains(&target.entity)
            {
                commands.entity(target.entity).insert(InteractionBlocked);
                view.state.blocked.push(target.entity);
            }
        }
    } else if view.state.holding_focus {
        view.state.holding_focus = false;
        for entity in view.state.blocked.drain(..) {
            if view.live.contains(entity) {
                commands.entity(entity).remove::<InteractionBlocked>();
            }
        }
        view.focus.clear();
        if let Some(entity) = view
            .state
            .previous_focus
            .take()
            .filter(|entity| view.live.contains(*entity))
        {
            view.focus.set(entity, FocusCause::Navigated);
        }
    }
    let mut rows = report_rows(view.state.model.snapshot.as_ref(), view.locale.code());
    if view.state.model.is_running() {
        rows.insert(
            0,
            Lang(view.locale.code())
                .tr("script_workbench_running")
                .into_owned(),
        );
    }
    if !open && let Some(failure) = &view.state.model.failure {
        rows.insert(0, failure.message.clone());
    }
    if view.state.model.export.is_some() && !open {
        rows.push(presentation.status.clone());
        rows.push(presentation.details.clone());
        rows.push(format!(
            "{}\n{}",
            Lang(view.locale.code()).tr("script_export_preview"),
            presentation.content
        ));
        if !presentation.path.is_empty() {
            rows.push(presentation.path.clone());
        }
    }
    for body in &view.bodies {
        let count = view
            .children
            .get(body)
            .map(|children| children.len())
            .unwrap_or(0);
        if count != rows.len() {
            commands.entity(body).despawn_children();
            for (index, value) in rows.iter().enumerate() {
                let row = bsn! {
                    Node { width: percent(100) }
                    Children [ Text({value.clone()}) ScriptResultRow(index) TextRole(Role::Mono) ]
                };
                commands.spawn_scene(row).insert(ChildOf(body));
            }
        } else {
            for (row, mut text) in &mut view.rows {
                if let Some(value) = rows.get(row.0)
                    && text.0 != *value
                {
                    text.0 = value.clone();
                }
            }
        }
    }
    view.state.rows = rows;
}
