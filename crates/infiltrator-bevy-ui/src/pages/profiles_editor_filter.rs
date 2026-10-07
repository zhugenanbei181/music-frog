//! Native controlled inputs replay the same source-bound editor as the workbench.
use crate::pages::profiles::LastProfilesProjection;
use crate::pages::profiles_editor_panes::{
    EditorFilterDedupButton, EditorFilterSaveButton, EditorFilterStatusText,
    ProfileEditorOptionsState, ProfileEditorPane,
};
use crate::pages::profiles_editor_panes_sync::editor_profile;
use crate::pages::profiles_editor_state::ProfileEditorState;
use accesskit::{Role, Toggled};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui::BorderColor;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::subscription_filter_copy::status;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::subscription_filter_form::FilterField;

#[derive(Component, Clone, Copy, Default)]
pub struct EditorFilterText(pub FilterField);
#[derive(Component, Clone, Copy, Default)]
pub struct EditorFilterDiscard;
#[derive(Component, Clone, Copy, Default)]
pub struct EditorFilterTransaction;

pub(super) fn initialize(
    insert: On<Insert<EditorFilterText>>,
    options: Res<ProfileEditorOptionsState>,
    mut fields: Query<(&EditorFilterText, &mut TextField)>,
) {
    if options.filter.source_profile().is_some()
        && let Ok((kind, mut field)) = fields.get_mut(insert.entity)
    {
        field
            .0
            .restore_text(kind.0.value(&options.filter.draft).into());
    }
}

pub(super) fn capture(
    options: &mut ProfileEditorOptionsState,
    fields: &Query<(&EditorFilterText, &TextField)>,
) {
    if !options.filter.can_edit() || options.filter_restore {
        return;
    }
    for (kind, field) in fields {
        if !field.0.preedit().is_empty() {
            options.filter.composing();
        }
        options.filter.edit(kind.0, field.0.text().into());
    }
}

pub(super) fn receive(
    mut options: ResMut<ProfileEditorOptionsState>,
    fields: Query<(&EditorFilterText, &TextField)>,
    document: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
) {
    capture(&mut options, &fields);
    let profile = editor_profile(&document, last.as_ref().and_then(|value| value.0.as_ref()));
    options.filter.bind_profile(profile.as_deref());
}

pub(super) fn replay(
    mut options: ResMut<ProfileEditorOptionsState>,
    mut fields: Query<(
        &EditorFilterText,
        &mut TextField,
        &mut TextFieldFocused,
        Option<&mut AccessibilityNode>,
    )>,
) {
    let disabled = options.pane != ProfileEditorPane::Filter || !options.filter.can_edit();
    for (kind, mut field, mut focus, node) in &mut fields {
        if options.filter_restore {
            field
                .0
                .restore_text(kind.0.value(&options.filter.draft).into());
        }
        field.0.set_disabled(disabled);
        if disabled {
            focus.0 = false;
        }
        if let Some(mut node) = node {
            if disabled {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
        }
    }
    if disabled {
        options.filter_focus = None;
    }
    options.filter_restore = false;
}

#[derive(QueryData)]
#[query_data(mutable)]
pub(super) struct FilterControl {
    entity: Entity,
    discard: Has<EditorFilterDiscard>,
    disabled: Option<&'static ButtonDisabled>,
    choice: Option<&'static EditorFilterDedupButton>,
    visual: Option<&'static mut ControlVisual>,
    node: Option<&'static mut AccessibilityNode>,
}
#[derive(QueryFilter)]
pub(super) struct FilterControls {
    controls: Or<(
        With<EditorFilterSaveButton>,
        With<EditorFilterDedupButton>,
        With<EditorFilterDiscard>,
    )>,
}
pub(super) fn controls(
    options: Res<ProfileEditorOptionsState>,
    fields: Query<&TextField, With<EditorFilterText>>,
    mut buttons: Query<FilterControl, FilterControls>,
    mut commands: Commands,
) {
    let composing = fields.iter().any(|field| !field.0.preedit().is_empty());
    for button in &mut buttons {
        let disabled = if button.discard {
            options.filter.pending.is_some()
        } else {
            !options.filter.can_edit() || composing
        };
        if button.disabled.is_none_or(|value| value.0 != disabled) {
            commands
                .entity(button.entity)
                .insert(ButtonDisabled(disabled));
        }
        let selected = button
            .choice
            .is_some_and(|choice| choice.index == options.filter.draft.dedup_index);
        if let Some(mut visual) = button.visual {
            visual.0 = selected;
        }
        if let Some(mut node) = button.node {
            if disabled {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
            if button.choice.is_some() {
                node.set_role(Role::RadioButton);
                node.set_toggled(if selected {
                    Toggled::True
                } else {
                    Toggled::False
                });
            }
        }
    }
}

pub(super) fn render_status(
    options: Res<ProfileEditorOptionsState>,
    locale: Res<UiLocale>,
    mut labels: Query<&mut Text, With<EditorFilterStatusText>>,
    palette: Res<UiPalette>,
    mut borders: Query<&mut BorderColor, With<EditorFilterTransaction>>,
) {
    let error = options.filter.failure.is_some() || options.filter.read_failure().is_some();
    for mut border in &mut borders {
        *border = BorderColor::all(if error {
            palette.danger
        } else {
            palette.border
        });
    }
    let value = status(&options.filter, locale.code());
    for mut text in &mut labels {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}
pub(super) fn discard(
    event: On<Activate>,
    buttons: Query<(), With<EditorFilterDiscard>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    if buttons.get(event.entity).is_ok() && options.filter.cancel() {
        options.filter_restore = true;
    }
}
