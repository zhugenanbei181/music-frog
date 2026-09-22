//! DUAL-09-14: systems and observers for the Bevy editor's Mixin/Filter panes.
//!
//! Split from `profiles_editor_panes.rs` (state + scenes) so both files stay
//! inside the business line budget. Every action here submits the *shared*
//! command (`LoadProfileOptions`, `SaveMixinOverlay`, `SaveSubscriptionFilter`)
//! and every rendered fact is restamped from the shared projection.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::scene::CommandsSceneExt;
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, Display, Node};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{LastProfilesProjection, ProfilesProjection};
use crate::pages::profiles_editor_body::editor_rows_scene;
use crate::pages::profiles_editor_panes::{
    EditorFilterDedupButton, EditorFilterField, EditorFilterFieldKind, EditorFilterSaveButton,
    EditorFilterStatusText, MixinEditorBody, MixinEditorDiagnosticPill, MixinEditorDiagnosticText,
    MixinEditorFocusButton, MixinEditorReloadButton, MixinEditorSaveButton,
    MixinEditorSnippetButton, MixinEditorStatusText, ProfileEditorOptionsState, ProfileEditorPane,
    ProfileEditorPaneArea, ProfileEditorPaneButton, chip_background, filter_status_line,
};
use crate::pages::profiles_editor_state::{ProfileEditorState, diagnostic_line, status_line};

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
fn editor_profile(
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_profile_editor_panes(
    update: On<crate::pages::profiles::ProfilesProjectionUpdated>,
    mut options: ResMut<ProfileEditorOptionsState>,
    document: Res<ProfileEditorState>,
    palette: Res<UiPalette>,
    handle: Option<Res<CommandSinkHandle>>,
    mut mixin_status: Query<
        &mut Text,
        (
            With<MixinEditorStatusText>,
            Without<EditorFilterStatusText>,
            Without<MixinEditorDiagnosticText>,
            Without<MixinEditorDiagnosticPill>,
        ),
    >,
    mut mixin_diagnostics: Query<
        (&mut Text, &mut TextColor),
        (
            With<MixinEditorDiagnosticText>,
            Without<MixinEditorStatusText>,
            Without<EditorFilterStatusText>,
            Without<MixinEditorDiagnosticPill>,
        ),
    >,
    mut filter_status: Query<
        &mut Text,
        (
            With<EditorFilterStatusText>,
            Without<MixinEditorStatusText>,
            Without<MixinEditorDiagnosticText>,
            Without<MixinEditorDiagnosticPill>,
        ),
    >,
    mut pills: Query<
        (&mut Text, &mut BackgroundColor),
        (
            With<MixinEditorDiagnosticPill>,
            Without<MixinEditorStatusText>,
            Without<EditorFilterStatusText>,
            Without<MixinEditorDiagnosticText>,
        ),
    >,
    mut dedup_chips: Query<
        (&EditorFilterDedupButton, &mut BackgroundColor),
        (
            Without<MixinEditorDiagnosticPill>,
            Without<MixinEditorStatusText>,
            Without<EditorFilterStatusText>,
        ),
    >,
    fields: Query<(&EditorFilterField, &Children)>,
    mut text_fields: Query<&mut TextField>,
) {
    let projection = &update.0;
    let profile = editor_profile(&document, Some(projection));
    if let Some(snapshot) = projection.profile_options.as_ref()
        && profile
            .as_deref()
            .is_none_or(|name| name == snapshot.profile.as_str())
    {
        options.adopt_snapshot(&snapshot.profile, &snapshot.mixin_yaml, &snapshot.filter);
    }
    // The pane may have been opened before the document (and therefore the
    // sidecar profile) was known; ask the shared application once it is, so an
    // open pane never sits blank until the user clicks reload.
    if options.pane != ProfileEditorPane::Profile
        && let (Some(profile), Some(handle)) = (profile.as_deref(), handle.as_deref())
        && projection
            .profile_options
            .as_ref()
            .is_none_or(|snapshot| snapshot.profile != profile)
    {
        request_sidecar(&mut options, profile, handle);
    }

    // Mixin status line: the shared status projection, never a local guess.
    let status = status_line(&options.mixin, Some(projection));
    for mut text in &mut mixin_status {
        if text.0 != status {
            text.0 = status.clone();
        }
    }

    // Mixin shared preflight verdict + its pill.
    let (diagnostic_text, has_error) = diagnostic_line(&options.mixin);
    let verdict_color = if has_error {
        palette.danger
    } else {
        palette.success
    };
    for (mut text, mut text_color) in &mut mixin_diagnostics {
        if text.0 != diagnostic_text {
            text.0 = diagnostic_text.clone();
        }
        if text_color.0 != verdict_color {
            text_color.0 = verdict_color;
        }
    }
    let pill_label = if has_error {
        "语法错误".to_owned()
    } else {
        "语法通过".to_owned()
    };
    for (mut text, mut background) in &mut pills {
        if text.0 != pill_label {
            text.0 = pill_label.clone();
        }
        if background.0 != verdict_color {
            background.0 = verdict_color;
        }
    }

    // Filter pane: restamp the four fields + dedup chips from the shared draft.
    for (field, children) in &fields {
        let Some(editor_kind) = field.kind else {
            continue;
        };
        let value = editor_kind.draft_field().value(&options.filter).to_owned();
        for child in children.iter() {
            if let Ok(mut text_field) = text_fields.get_mut(*child)
                && text_field.0.text() != value
            {
                text_field.0.apply(TextFieldInput::SetText(value.clone()));
            }
        }
    }
    for (chip, mut background) in &mut dedup_chips {
        let target = chip_background(options.filter.dedup_index == chip.index, &palette);
        if background.0 != target {
            background.0 = target;
        }
    }
    let filter_text = filter_status_line(&options);
    for mut text in &mut filter_status {
        if text.0 != filter_text {
            text.0 = filter_text.clone();
        }
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
    let Some(profile) = editor_profile(&document, projection) else {
        return;
    };
    if options.mixin.diagnostic.is_some() {
        options.mixin.notice = Some("Mixin 覆盖未通过共享语法预检，未提交".to_owned());
        return;
    }
    // DUAL-10-10: the shared preflight (syntax + merge + output validation)
    // is the real gate; a local YAML-only check would miss a merge that the
    // kernel cannot load.
    let base = document.buffer.full_text();
    let report =
        infiltrator_domain::mixin_studio::preflight_mixin(&base, &options.mixin.buffer.full_text());
    if let Some(error) = report.error {
        options.mixin.notice = Some(format!("Mixin 覆盖未通过共享预检：{error}"));
        return;
    }
    options.mixin.notice = None;
    // The dirty flag clears when the shared snapshot publishes the stored
    // bytes back (a failed save leaves the buffer marked as unsaved).
    handle.submit(UiCommand::SaveMixinOverlay {
        profile,
        mixin_yaml: options.mixin.buffer.full_text(),
    });
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
    let Some(snippet) = infiltrator_contract::yaml_snippets::YAML_SNIPPETS.get(button.index) else {
        return;
    };
    let _ = options.mixin.insert_snippet(snippet.id);
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
    options.filter.dedup_index = button.index;
    options.filter_notice = None;
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
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let Some(profile) = editor_profile(&document, projection) else {
        options.filter_notice = Some("没有打开任何配置，过滤管道未提交".to_owned());
        return;
    };
    match infiltrator_domain::profile_options::filter_spec_from_draft(&options.filter) {
        Ok(_) => {
            options.filter_notice = None;
            handle.submit(UiCommand::SaveSubscriptionFilter {
                profile_id: profile,
                filter: options.filter.clone(),
            });
        }
        Err(error) => {
            options.filter_notice = Some(format!("draft 无法编译：{error}"));
        }
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
pub fn route_editor_keyboard(
    mut keys: MessageReader<KeyboardInput>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut document: ResMut<ProfileEditorState>,
    mut options: ResMut<ProfileEditorOptionsState>,
    fields: Query<(&EditorFilterField, &Children)>,
    mut text_fields: Query<&mut TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let modifiers = keyboard
        .as_deref()
        .map(crate::shortcuts::modifiers_from_keyboard)
        .unwrap_or_default();
    let pressed: Vec<Key> = keys
        .read()
        .filter(|key| key.state == ButtonState::Pressed)
        .map(|key| key.logical_key.clone())
        .collect();
    let modified = modifiers.ctrl || modifiers.alt || modifiers.meta;
    match options.pane {
        ProfileEditorPane::Profile => {
            if !document.focused {
                return;
            }
            for key in &pressed {
                if modified {
                    let is_save = modifiers.ctrl
                        && !modifiers.alt
                        && !modifiers.meta
                        && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("s"));
                    if is_save && let Some(handle) = handle.as_ref() {
                        handle.submit(UiCommand::SaveProfileDocument {
                            profile: document.profile.clone(),
                            content: document.buffer.full_text(),
                            allow_protected: document.protection_override,
                        });
                    }
                    continue;
                }
                document.apply_key(key);
            }
        }
        ProfileEditorPane::Mixin => {
            if !options.mixin.focused {
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
                        let report = infiltrator_domain::mixin_studio::preflight_mixin(
                            &document.buffer.full_text(),
                            &options.mixin.buffer.full_text(),
                        );
                        if report.is_blocking() {
                            options.mixin.notice = report.error;
                        } else {
                            handle.submit(UiCommand::SaveMixinOverlay {
                                profile: options.mixin.profile.clone(),
                                mixin_yaml: options.mixin.buffer.full_text(),
                            });
                        }
                    }
                    continue;
                }
                options.mixin.apply_key(key);
            }
        }
        ProfileEditorPane::Filter => {
            let Some(kind) = options.filter_focus else {
                return;
            };
            if modified {
                return;
            }
            for key in &pressed {
                if *key == Key::Tab {
                    options.filter_focus = Some(next_filter_field(kind));
                    continue;
                }
                if *key == Key::Escape {
                    options.filter_focus = None;
                    continue;
                }
                let Some(input) = filter_field_input(key) else {
                    continue;
                };
                for (field, children) in &fields {
                    if field.kind != Some(kind) {
                        continue;
                    }
                    for child in children.iter() {
                        if let Ok(mut text_field) = text_fields.get_mut(*child) {
                            text_field.0.apply(input.clone());
                            let text = text_field.0.text().to_owned();
                            let mut draft = options.filter.clone();
                            kind.draft_field().set(&mut draft, text);
                            options.filter = draft;
                        }
                    }
                }
            }
        }
    }
}

fn next_filter_field(kind: EditorFilterFieldKind) -> EditorFilterFieldKind {
    match kind {
        EditorFilterFieldKind::Include => EditorFilterFieldKind::Exclude,
        EditorFilterFieldKind::Exclude => EditorFilterFieldKind::ExcludeTypes,
        EditorFilterFieldKind::ExcludeTypes => EditorFilterFieldKind::Renames,
        EditorFilterFieldKind::Renames => EditorFilterFieldKind::Include,
    }
}

fn filter_field_input(key: &Key) -> Option<TextFieldInput> {
    match key {
        Key::Character(text) => Some(TextFieldInput::Insert(text.to_string())),
        Key::Space => Some(TextFieldInput::Insert(" ".to_owned())),
        Key::Backspace => Some(TextFieldInput::Backspace),
        Key::Delete => Some(TextFieldInput::Delete),
        Key::ArrowLeft => Some(TextFieldInput::Left(false)),
        Key::ArrowRight => Some(TextFieldInput::Right(false)),
        Key::Home => Some(TextFieldInput::Home),
        Key::End => Some(TextFieldInput::End),
        _ => None,
    }
}

/// Register the pane systems and observers.
pub struct ProfilesEditorPanesPlugin;

impl Plugin for ProfilesEditorPanesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileEditorOptionsState>();
        app.add_observer(on_profile_editor_pane_activated);
        app.add_observer(on_mixin_editor_focus);
        app.add_observer(on_mixin_editor_reload);
        app.add_observer(on_mixin_editor_save);
        app.add_observer(crate::pages::profiles_editor_mixin_studio::on_mixin_toggle_activated);
        app.add_observer(on_mixin_editor_snippet_activated);
        app.add_observer(on_editor_filter_dedup_activated);
        app.add_observer(on_editor_filter_save);
        app.add_observer(on_editor_filter_field_activated);
        app.add_observer(sync_profile_editor_panes);
        app.add_systems(
            Update,
            (
                sync_profile_editor_pane_areas,
                // The studio rebuild respawns the middle column's editor body,
                // so it must run before the editor rows are restamped.
                crate::pages::profiles_editor_mixin_studio::refresh_mixin_studio_body,
                refresh_mixin_editor_body,
            )
                .chain(),
        );
    }
}
