//! Native editor submission and correlated actual transaction receipts.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::profiles_editor::{
    ProfileEditorFocusButton, ProfileEditorFormatButton, ProfileEditorProtectionToggle,
    ProfileEditorSaveButton, ProfileEditorSnippetButton,
};
use crate::pages::profiles_editor_mixin_studio::MixinToggleButton;
use crate::pages::profiles_editor_panes::{
    MixinEditorFocusButton, MixinEditorSaveButton, MixinEditorSnippetButton,
    ProfileEditorOptionsState,
};
use crate::pages::profiles_editor_state::ProfileEditorState;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryData, With};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::{BackgroundColor, Node, UiRect, Val};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};

pub fn submit_document(state: &mut ProfileEditorState, handle: &CommandSinkHandle) {
    let pending = match state
        .session
        .begin_document(state.buffer.full_text(), state.protection_override)
    {
        Ok(pending) => pending,
        Err(failure) => {
            state.session.failure = Some(failure);
            return;
        }
    };
    let CommandIntent::SaveProfileDocument {
        source,
        content,
        allow_protected,
    } = pending.intent.clone()
    else {
        unreachable!()
    };
    submit(
        state,
        handle,
        UiCommand::SaveProfileDocument {
            source,
            content,
            allow_protected,
        },
        pending.operation,
    );
}
pub fn submit_mixin(state: &mut ProfileEditorState, handle: &CommandSinkHandle) {
    let pending = match state.session.begin_mixin(state.buffer.full_text()) {
        Ok(pending) => pending,
        Err(failure) => {
            state.session.failure = Some(failure);
            return;
        }
    };
    let CommandIntent::SaveMixinOverlay { source, mixin_yaml } = pending.intent.clone() else {
        unreachable!()
    };
    submit(
        state,
        handle,
        UiCommand::SaveMixinOverlay { source, mixin_yaml },
        pending.operation,
    );
}
fn submit(
    state: &mut ProfileEditorState,
    handle: &CommandSinkHandle,
    command: UiCommand,
    operation: u64,
) {
    if let Some(id) = handle.submit_tracked(command) {
        state.save_request = Some((id, operation));
    } else {
        let intent = state
            .session
            .pending
            .as_ref()
            .expect("prepared editor operation")
            .intent
            .clone();
        state.session.finish(
            operation,
            &intent,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Editor command has no tracked terminal result",
                true,
            )),
        );
    }
}
fn finish(state: &mut ProfileEditorState, event: &CommandExecutedEvent) -> bool {
    let Some((id, operation)) = state.save_request else {
        return false;
    };
    let Some(pending) = state.session.pending.clone() else {
        return false;
    };
    if id != event.request_id || event.command.to_intent().as_ref() != Some(&pending.intent) {
        return false;
    }
    let accepted = state
        .session
        .finish(operation, &pending.intent, event.result.clone());
    if accepted {
        state.save_request = None;
    }
    accepted
}
pub fn receive(
    event: On<CommandExecutedEvent>,
    mut document: ResMut<ProfileEditorState>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    if matches!(event.command, UiCommand::SaveProfileDocument { .. }) {
        if finish(&mut document, &event)
            && let Ok(CommandOutput::ProfileDocumentSaved(saved)) = &event.result
        {
            document.loaded_content = Some(saved.document.content.clone());
            document.dirty = false;
        }
    } else if matches!(event.command, UiCommand::SaveMixinOverlay { .. })
        && finish(&mut options.mixin, &event)
        && let Ok(CommandOutput::ProfileMixinSaved(saved)) = &event.result
    {
        options.mixin.buffer = CodeEditorState::new(&saved.options.mixin_yaml);
        options.mixin.loaded_content = Some(saved.options.mixin_yaml.clone());
        options.mixin_loaded = Some(saved.options.mixin_yaml.clone());
        options.mixin.dirty = false;
        options.mixin.generation += 1;
        document.load_document(&saved.document);
    }
}
#[derive(QueryData)]
pub struct EditorControlData {
    entity: Entity,
    disabled: Option<&'static ButtonDisabled>,
    save_document: Has<ProfileEditorSaveButton>,
    save_mixin: Has<MixinEditorSaveButton>,
    document_focus: Has<ProfileEditorFocusButton>,
    document_format: Has<ProfileEditorFormatButton>,
    document_snippet: Has<ProfileEditorSnippetButton>,
    document_unlock: Has<ProfileEditorProtectionToggle>,
    mixin_focus: Has<MixinEditorFocusButton>,
    mixin_snippet: Has<MixinEditorSnippetButton>,
    mixin_toggle: Has<MixinToggleButton>,
    discard: Option<&'static EditorDiscardButton>,
}
#[derive(Component, Clone, Default)]
pub struct EditorMutationControl;
#[derive(SystemParam)]
pub struct SaveControls<'w, 's> {
    document: Res<'w, ProfileEditorState>,
    options: Res<'w, ProfileEditorOptionsState>,
    buttons: Query<'w, 's, EditorControlData, With<EditorMutationControl>>,
    commands: Commands<'w, 's>,
}
pub fn controls(mut view: SaveControls) {
    for button in &view.buttons {
        let disabled = if button.save_document {
            !view.document.can_save(view.document.protection)
        } else if button.save_mixin {
            !view.options.mixin.session.can_save()
        } else if button.document_focus
            || button.document_format
            || button.document_snippet
            || button.document_unlock
        {
            !view.document.session.can_edit()
        } else if button.mixin_focus || button.mixin_snippet || button.mixin_toggle {
            !view.options.mixin.session.can_edit()
        } else if let Some(discard) = button.discard {
            let session = if discard.mixin {
                &view.options.mixin.session
            } else {
                &view.document.session
            };
            session.pending.is_some() || session.source().is_none()
        } else {
            false
        };
        if button.disabled.is_none_or(|current| current.0 != disabled) {
            view.commands
                .entity(button.entity)
                .insert(ButtonDisabled(disabled));
        }
    }
}

#[derive(Component, Clone, Default)]
pub struct EditorDiscardButton {
    pub mixin: bool,
}

pub fn discard_scene(mixin: bool, palette: &UiPalette) -> impl Scene {
    let background = palette.surface_elevated;
    bsn! {
        Node { min_height: Val::Px(28.0), padding: UiRect::horizontal(Val::Px(8.0)) }
        BackgroundColor({ background }) Button EditorMutationControl EditorDiscardButton { mixin }
        Children [ LocalizedText::plain("editor_discard_draft") TextRole(Role::Caption) ]
    }
}

pub fn discard(
    event: On<Activate>,
    buttons: Query<&EditorDiscardButton>,
    mut document: ResMut<ProfileEditorState>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let state = if button.mixin {
        &mut options.mixin
    } else {
        &mut document
    };
    if let Some(content) = state.session.discard() {
        state.buffer = CodeEditorState::new(&content);
        state.profile = state
            .session
            .source()
            .expect("discard adopted source")
            .profile
            .clone();
        state.loaded_content = Some(content);
        state.dirty = false;
        state.protection_override = false;
        state.notice = None;
        state.generation += 1;
        state.refresh_preflight();
    }
}
