//! DUAL-09-14: systems and observers for the Bevy editor's Mixin/Filter panes.
//!
//! Split from `profiles_editor_panes.rs` (state + scenes) so both files stay
//! inside the business line budget. Every action here submits the *shared*
//! command (`LoadProfileOptions`, `SaveMixinOverlay`, `SaveSubscriptionFilter`)
//! and every rendered fact is restamped from the shared projection.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::profiles::{
    LastProfilesProjection, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::profiles_editor_body::editor_rows_scene;
use crate::pages::profiles_editor_copy::replay_mixin_copy;
use crate::pages::profiles_editor_filter::{self, EditorFilterText};
use crate::pages::profiles_editor_mixin_studio::{
    on_mixin_toggle_activated, refresh_mixin_studio_body,
};
use crate::pages::profiles_editor_panes::{
    EditorFilterDedupButton, EditorFilterField, EditorFilterSaveButton, EditorFilterStatusText,
    MixinEditorBody, MixinEditorDiagnosticPill, MixinEditorDiagnosticText, MixinEditorFocusButton,
    MixinEditorReloadButton, MixinEditorSaveButton, MixinEditorSnippetButton,
    MixinEditorStatusText, ProfileEditorOptionsState, ProfileEditorPane, ProfileEditorPaneArea,
    ProfileEditorPaneButton,
};
use crate::pages::profiles_editor_state::ProfileEditorState;
use crate::pages::profiles_editor_transactions::{self, submit_document, submit_mixin};
use crate::pages::profiles_mixin_copy::replay_studio_copy;
use crate::pages::profiles_script_workbench::ScriptWorkbenchState;
use crate::pages::snapshot_restore::RestoreState;
use crate::shortcuts::modifiers_from_keyboard;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::input_focus::InputFocus;
use bevy::scene::CommandsSceneExt;
use bevy::ui::prelude::{BackgroundColor, Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::{
    sync_button_disabled, sync_control_labels, sync_control_visuals,
};
use infiltrator_bevy_widgets::multiline_editor::MultilineEditor;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::render::sync_text_fields;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;
use infiltrator_domain::mixin_studio::preflight_mixin;

/// Toggle each pane area to the active pane and restamp the switcher chips.
pub fn sync_profile_editor_pane_areas(
    options: Res<ProfileEditorOptionsState>,
    palette: Res<UiPalette>,
    mut areas: Query<(&ProfileEditorPaneArea, &mut Node)>,
    mut buttons: Query<(&ProfileEditorPaneButton, &mut BackgroundColor)>,
) {
    for (area, mut node) in &mut areas {
        let target = if area.pane == options.pane {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != target {
            node.display = target;
        }
    }
    for (button, mut background) in &mut buttons {
        let target = if button.pane == options.pane {
            palette.accent
        } else {
            palette.surface_elevated
        };
        if background.0 != target {
            background.0 = target;
        }
    }
}

/// The profile the editor card is bound to: the loaded document's profile, or
/// the active profile in the shared projection when nothing is loaded yet.
pub(super) fn editor_profile(
    document: &ProfileEditorState,
    last: Option<&ProfilesProjection>,
) -> Option<String> {
    if !document.profile.is_empty() {
        return Some(document.profile.clone());
    }
    last.and_then(|projection| {
        projection
            .profiles
            .iter()
            .find(|profile| profile.is_active)
            .map(|profile| profile.id.clone())
    })
}

/// Adopt the shared sidecar snapshot and restamp the pane status lines.
#[derive(QueryFilter)]
pub struct MixinStatusFilter {
    status: With<MixinEditorStatusText>,
    filter: Without<EditorFilterStatusText>,
    diagnostic: Without<MixinEditorDiagnosticText>,
    pill: Without<MixinEditorDiagnosticPill>,
}
#[derive(QueryFilter)]
pub struct MixinDiagnosticFilter {
    diagnostic: With<MixinEditorDiagnosticText>,
    status: Without<MixinEditorStatusText>,
    filter: Without<EditorFilterStatusText>,
    pill: Without<MixinEditorDiagnosticPill>,
}
#[derive(QueryFilter)]
pub struct MixinPillFilter {
    pill: With<MixinEditorDiagnosticPill>,
    status: Without<MixinEditorStatusText>,
    filter: Without<EditorFilterStatusText>,
    diagnostic: Without<MixinEditorDiagnosticText>,
}
#[derive(SystemParam)]
pub struct EditorPaneProjection<'w, 's> {
    options: ResMut<'w, ProfileEditorOptionsState>,
    document: Res<'w, ProfileEditorState>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    filter_inputs: Query<'w, 's, (&'static EditorFilterText, &'static TextField)>,
}
pub fn sync_profile_editor_panes(
    update: On<ProfilesProjectionUpdated>,
    view: EditorPaneProjection,
) {
    let EditorPaneProjection {
        mut options,
        document,
        handle,
        filter_inputs,
    } = view;
    profiles_editor_filter::capture(&mut options, &filter_inputs);
    let projection = &update.0;
    let profile = editor_profile(&document, Some(projection));
    if let Some(snapshot) = projection.profile_options.as_ref()
        && profile
            .as_deref()
            .is_none_or(|name| name == snapshot.source.profile.as_str())
    {
        options.adopt_snapshot(&snapshot.source, &snapshot.mixin_yaml, &snapshot.filter);
    }
    options
        .mixin
        .session
        .observe_read_status(&projection.editor_read, true);
    // The pane may have been opened before the document (and therefore the
    // sidecar profile) was known; ask the shared application once it is, so an
    // open pane never sits blank until the user clicks reload.
    if options.pane != ProfileEditorPane::Profile
        && let (Some(profile), Some(handle)) = (profile.as_deref(), handle.as_deref())
        && projection
            .profile_options
            .as_ref()
            .is_none_or(|snapshot| snapshot.source.profile != profile)
    {
        request_sidecar(&mut options, profile, handle);
    }
}

/// Rebuild the Mixin body whenever its buffer generation moved.
pub fn refresh_mixin_editor_body(
    mut options: ResMut<ProfileEditorOptionsState>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<MixinEditorBody>>,
) {
    if options.mixin.last_rendered == options.mixin.generation {
        return;
    }
    options.mixin.last_rendered = options.mixin.generation;
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        let scene = editor_rows_scene(&options.mixin, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
}

/// Ask the shared application for the sidecar once per profile.
fn request_sidecar(
    options: &mut ProfileEditorOptionsState,
    profile: &str,
    handle: &CommandSinkHandle,
) {
    if options.requested_for.as_deref() == Some(profile) {
        return;
    }
    options.requested_for = Some(profile.to_owned());
    handle.submit(UiCommand::LoadProfileOptions {
        profile: Some(profile.to_owned()),
    });
}

/// Switch the visible pane; the Mixin/Filter panes lazily load the sidecar.
pub fn on_profile_editor_pane_activated(
    activate: On<Activate>,
    buttons: Query<&ProfileEditorPaneButton>,
    document: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    options.pane = button.pane;
    if options.pane == ProfileEditorPane::Profile {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let Some(profile) = editor_profile(&document, projection) else {
        return;
    };
    request_sidecar(&mut options, &profile, &handle);
}

/// Reload the stored sidecar through the shared application.
pub fn on_mixin_editor_reload(
    activate: On<Activate>,
    buttons: Query<(), With<MixinEditorReloadButton>>,
    document: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let Some(profile) = editor_profile(&document, projection) else {
        return;
    };
    options.requested_for = None;
    request_sidecar(&mut options, &profile, &handle);
}

/// Commit the Mixin buffer through the shared sidecar use-case. A buffer that
/// fails the shared preflight is refused locally; the application re-checks.
pub fn on_mixin_editor_save(
    activate: On<Activate>,
    buttons: Query<(), With<MixinEditorSaveButton>>,
    document: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    if editor_profile(&document, projection).is_none() {
        return;
    }
    if let Some(diagnostic) = options.mixin.diagnostic.as_ref() {
        options.mixin.notice = Some(diagnostic.message.clone());
        return;
    }
    // DUAL-10-10: the shared preflight (syntax + merge + output validation)
    // is the real gate; a local YAML-only check would miss a merge that the
    // kernel cannot load.
    let base = document.buffer.full_text();
    let report = preflight_mixin(&base, &options.mixin.buffer.full_text());
    if let Some(error) = report.error {
        options.mixin.notice = Some(error);
        return;
    }
    options.mixin.notice = None;
    // The dirty flag clears when the shared snapshot publishes the stored
    // bytes back (a failed save leaves the buffer marked as unsaved).
    submit_mixin(&mut options.mixin, &handle);
}

/// Grab the keyboard for the Mixin buffer.
pub fn on_mixin_editor_focus(
    activate: On<Activate>,
    buttons: Query<(), With<MixinEditorFocusButton>>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if !options.mixin.session.can_edit() {
        return;
    }
    options.mixin.focused = !options.mixin.focused;
    options.mixin.generation = options.mixin.generation.wrapping_add(1);
}

/// Insert a shared catalogue snippet into the Mixin buffer at its caret.
pub fn on_mixin_editor_snippet_activated(
    activate: On<Activate>,
    buttons: Query<&MixinEditorSnippetButton>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(snippet) = YAML_SNIPPETS.get(button.index) else {
        return;
    };
    if options.mixin.session.can_edit() {
        let _ = options.mixin.insert_snippet(snippet.id);
    }
}

/// Pick the dedup strategy of the shared draft.
pub fn on_editor_filter_dedup_activated(
    activate: On<Activate>,
    buttons: Query<&EditorFilterDedupButton>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if let Some(mode) = SubscriptionFilterDedup::from_index(button.index) {
        options.filter.pick(mode);
    }
}

/// Commit the shared filter draft. The shared parser gates the submit first,
/// so a malformed rename line surfaces in the pane instead of silently
/// failing inside the command pump.
pub fn on_editor_filter_save(
    activate: On<Activate>,
    buttons: Query<(), With<EditorFilterSaveButton>>,
    document: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    mut options: ResMut<ProfileEditorOptionsState>,
    fields: Query<(&EditorFilterText, &TextField)>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    if editor_profile(&document, projection).as_deref() != options.filter.source_profile() {
        return;
    }
    profiles_editor_filter::capture(&mut options, &fields);
    if fields
        .iter()
        .any(|(_, field)| !field.0.preedit().is_empty())
    {
        return;
    }
    let pending = match options.filter.begin() {
        Ok(pending) => pending,
        Err(_) => return,
    };
    let command = UiCommand::SaveSubscriptionFilter {
        source: pending.source.source,
        filter: pending.draft,
    };
    if let Some(id) = handle.submit_tracked(command) {
        options.filter_request = Some((id, pending.token));
    } else {
        options.filter.finish(
            pending.token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Filter service has no terminal acknowledgment",
                true,
            )),
        );
    }
}

/// Focus one filter field for the keyboard seam.
pub fn on_editor_filter_field_activated(
    activate: On<Activate>,
    fields: Query<(&EditorFilterField, &Children)>,
    mut options: ResMut<ProfileEditorOptionsState>,
    mut commands: Commands,
    text_fields: Query<Entity, With<TextField>>,
) {
    let Ok((field, _children)) = fields.get(activate.entity) else {
        return;
    };
    let Some(kind) = field.kind else {
        return;
    };
    options.filter_focus = Some(kind);
    for (other, other_children) in &fields {
        let focused = other.kind == Some(kind);
        for child in other_children.iter() {
            if text_fields.get(*child).is_err() {
                continue;
            }
            if focused {
                commands.entity(*child).insert(TextFieldFocused(true));
            } else {
                commands.entity(*child).remove::<TextFieldFocused>();
            }
        }
    }
}

// ---- keyboard routing ------------------------------------------------------

/// One keyboard seam for the whole editor card: the active pane owns the keys.
/// The profile document and the Mixin overlay share the same buffer rules; the
/// Filter pane feeds the focused controlled text field.
#[derive(SystemParam)]
pub struct EditorKeyboard<'w, 's> {
    restore: Option<Res<'w, RestoreState>>,
    keys: MessageReader<'w, 's, KeyboardInput>,
    keyboard: Option<Res<'w, ButtonInput<KeyCode>>>,
    document: ResMut<'w, ProfileEditorState>,
    options: ResMut<'w, ProfileEditorOptionsState>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    script: Option<Res<'w, ScriptWorkbenchState>>,
    focus: Res<'w, InputFocus>,
    native: Query<'w, 's, (), With<MultilineEditor>>,
}
pub fn route_editor_keyboard(context: EditorKeyboard) {
    let EditorKeyboard {
        restore,
        mut keys,
        keyboard,
        mut document,
        mut options,
        handle,
        script,
        focus,
        native,
    } = context;
    if restore
        .as_ref()
        .is_some_and(|restore| restore.model.visible)
    {
        keys.clear();
        return;
    }
    if script
        .as_deref()
        .is_some_and(|script| script.model.export_visible)
        || focus.get().is_some_and(|entity| native.contains(entity))
    {
        document.focused = false;
        options.mixin.focused = false;
        keys.clear();
        return;
    }
    let modifiers = keyboard
        .as_deref()
        .map(modifiers_from_keyboard)
        .unwrap_or_default();
    let pressed: Vec<Key> = keys
        .read()
        .filter(|key| key.state == ButtonState::Pressed)
        .map(|key| key.logical_key.clone())
        .collect();
    let modified = modifiers.ctrl || modifiers.alt || modifiers.meta;
    match options.pane {
        ProfileEditorPane::Profile => {
            if !document.focused || !document.session.can_edit() {
                return;
            }
            for key in &pressed {
                if modified {
                    let is_save = modifiers.ctrl
                        && !modifiers.alt
                        && !modifiers.meta
                        && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("s"));
                    if is_save && let Some(handle) = handle.as_ref() {
                        submit_document(&mut document, handle);
                    }
                    continue;
                }
                document.apply_key(key);
            }
        }
        ProfileEditorPane::Mixin => {
            if !options.mixin.focused || !options.mixin.session.can_edit() {
                return;
            }
            for key in &pressed {
                if modified {
                    let is_save = modifiers.ctrl
                        && !modifiers.alt
                        && !modifiers.meta
                        && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("s"));
                    if is_save
                        && !options.mixin.profile.is_empty()
                        && let Some(handle) = handle.as_ref()
                    {
                        let report = preflight_mixin(
                            &document.buffer.full_text(),
                            &options.mixin.buffer.full_text(),
                        );
                        if report.is_blocking() {
                            options.mixin.notice = report.error;
                        } else {
                            submit_mixin(&mut options.mixin, handle);
                        }
                    }
                    continue;
                }
                options.mixin.apply_key(key);
            }
        }
        ProfileEditorPane::Filter => {}
    }
}

pub struct ProfilesEditorPanesPlugin;

impl Plugin for ProfilesEditorPanesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileEditorOptionsState>();
        app.add_observer(profiles_editor_transactions::receive);
        app.add_observer(profiles_editor_transactions::discard);
        app.add_observer(profiles_editor_filter::initialize);
        app.add_observer(profiles_editor_filter::discard);
        app.add_systems(
            Update,
            (
                profiles_editor_filter::receive,
                profiles_editor_filter::replay,
                profiles_editor_filter::render_status,
                profiles_editor_filter::controls,
            )
                .chain()
                .before(sync_control_visuals)
                .before(sync_control_labels)
                .before(sync_text_fields),
        );
        app.add_systems(
            PostUpdate,
            (
                profiles_editor_filter::replay,
                profiles_editor_filter::render_status,
                profiles_editor_filter::controls,
                profiles_editor_transactions::controls,
                ApplyDeferred,
            )
                .chain()
                .before(sync_button_disabled),
        );
        app.add_observer(on_profile_editor_pane_activated);
        app.add_observer(on_mixin_editor_focus);
        app.add_observer(on_mixin_editor_reload);
        app.add_observer(on_mixin_editor_save);
        app.add_observer(on_mixin_toggle_activated);
        app.add_observer(on_mixin_editor_snippet_activated);
        app.add_observer(on_editor_filter_dedup_activated);
        app.add_observer(on_editor_filter_save);
        app.add_observer(finish_editor_filter);
        app.add_observer(on_editor_filter_field_activated);
        app.add_observer(sync_profile_editor_panes);
        app.add_systems(
            Update,
            (
                sync_profile_editor_pane_areas,
                replay_mixin_copy,
                replay_studio_copy,
                // The studio rebuild respawns the middle column's editor body,
                // so it must run before the editor rows are restamped.
                refresh_mixin_studio_body,
                refresh_mixin_editor_body,
                ApplyDeferred,
                profiles_editor_transactions::controls,
                ApplyDeferred,
            )
                .chain()
                .before(sync_control_visuals)
                .before(sync_control_labels)
                .before(sync_text_fields),
        );
    }
}

fn finish_editor_filter(
    event: On<CommandExecutedEvent>,
    mut options: ResMut<ProfileEditorOptionsState>,
) {
    let Some((id, token)) = options.filter_request else {
        return;
    };
    let Some(pending) = &options.filter.pending else {
        return;
    };
    if event.request_id != id
        || event.command
            != (UiCommand::SaveSubscriptionFilter {
                source: pending.source.source.clone(),
                filter: pending.draft.clone(),
            })
    {
        return;
    }
    let result = event
        .result
        .clone()
        .and_then(CommandOutput::into_subscription_filter);
    if options.filter.finish(token, result) {
        options.filter_request = None;
    }
}
