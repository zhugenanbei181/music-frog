//! DUAL-09-03/05/14: the Bevy profile document editor (配置文档编辑器).
//!
//! The editor is a real surface, not a mock: it renders the document the
//! shared application loaded (`ProfileDocumentSnapshot`), runs the *shared*
//! syntax preflight (`infiltrator_domain::config::preflight_yaml_syntax`) on
//! every keystroke for the line/column banner and the red line highlight,
//! formats through the *shared* AST-preserving formatter
//! (`infiltrator_domain::yaml_edit::format::format_yaml`) and commits through
//! the shared guarded write path (`CommandIntent::SaveProfileDocument`).
//!
//! Honest boundaries, stated in the UI:
//! * no virtual scroll — only [`PROFILE_EDITOR_RENDER_LIMIT`] lines around the
//!   cursor are rendered, so a 10k-line document is not claimed as 60 FPS;
//! * the keyboard seam is explicit (the 「编辑」 button grabs it), mirroring how
//!   the command palette owns its own keyboard while open.

#[path = "profiles_editor_query_access.rs"]
pub mod query_access;
use self::query_access::EditorVisualTargets;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{
    LastProfilesProjection, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::profiles_editor_body::editor_rows_scene;
use crate::pages::profiles_editor_copy::replay_document_copy;
use crate::pages::profiles_editor_panes::{
    ProfileEditorOptionsState, ProfileEditorPane, ProfileEditorPaneArea, filter_pane_scene,
    mixin_pane_scene, pane_switch_scene,
};
use crate::pages::profiles_editor_panes_sync::route_editor_keyboard;
use crate::pages::profiles_editor_state::{
    PROFILE_EDITOR_RENDER_LIMIT, ProfileEditorState, diagnostic_line, protection_toggle_visual,
    status_line,
};
use crate::pages::profiles_editor_transactions::EditorMutationControl;
use crate::pages::profiles_editor_transactions::{discard_scene, submit_document};
use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::keyboard::KeyboardInput;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::profile_editor_projection;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;

/// Marker for the editor card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorRoot;

#[derive(Component, Clone, Default)]
pub struct ProfileEditorTitle;

/// Container whose children are the gutter + rendered lines.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorBody;

/// Restamped status line (dirty state, cursor, apply outcome).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorStatusText;

/// Restamped syntax banner.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorDiagnosticText;

/// The offending line/column pill; painted with the verdict color.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorDiagnosticPill;

/// Restamped shared write-protection badge.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorProtectionText;

/// Grab the keyboard for the editor buffer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorFocusButton;

/// Reload the stored document through the shared application.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorReloadButton;

/// Format the buffer with the shared AST-preserving formatter.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorFormatButton;

/// Commit the buffer through the shared guarded write path.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorSaveButton;

/// Explicit unlock for a protected subscription (DUAL-09-12).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorProtectionToggle;

/// DUAL-09-04: one snippet-bar button. The component carries the *index* into
/// the shared catalogue, so the scene never inlines a snippet body.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorSnippetButton {
    pub index: usize,
}

/// The profile document editor card.
pub fn profile_editor_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let document = projection.profile_document.as_ref();
    let protection = document
        .map(|document| document.write_protection)
        .unwrap_or_default();
    let buffer_lines = document
        .map(|document| document.content.lines().count())
        .unwrap_or(0);
    let mut state = ProfileEditorState {
        profile: document
            .map(|document| document.profile.clone())
            .unwrap_or_default(),
        buffer: CodeEditorState::new(
            document
                .map(|document| document.content.as_str())
                .unwrap_or(""),
        ),
        session: Default::default(),
        save_request: None,
        protection,
        dirty: false,
        loaded_content: document.map(|document| document.content.clone()),
        diagnostic: document.and_then(|document| document.syntax.clone()),
        notice: None,
        format_note: None,
        protection_override: false,
        focused: false,
        generation: 0,
        // The first Update pass renders the body from this state.
        last_rendered: u64::MAX,
    };
    if let Some(document) = document {
        state.load_document(document);
    }
    let status = status_line(&state, Some(projection), UiLocale::default().code());
    let (diagnostic_text, has_error) = diagnostic_line(&state, UiLocale::default().code());
    let initial_rows = editor_rows_scene(&state, palette);
    let options = ProfileEditorOptionsState::from_projection(projection);

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            ProfileEditorRoot
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::FileText, 24.0, palette) }
                                    --
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(space::S4),
                                    }
                                    Children [
                                        Text({ profile_editor_projection::title(buffer_lines, UiLocale::default().code()) }) ProfileEditorTitle TextRole(Role::BodyStrong)
                                        --
                                        Text(status) ProfileEditorStatusText TextRole(Role::Caption)
                                        --
                                        Text({ profile_editor_projection::protection_label(protection, UiLocale::default().code()) }) ProfileEditorProtectionText TextRole(Role::Caption)
                                    ]
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                ProfileEditorPaneArea { pane: ProfileEditorPane::Profile }
                                Children [
                                    @{ action_button("editor_focus_keyboard", palette.surface_elevated) }
                                    --
                                    @{ protection_toggle(protection, palette) }
                                    --
                                    @{ reload_button(palette) }
                                    --
                                    @{ format_button(palette) }
                                    --
                                    @{ save_button(palette) }
                                    --
                                    @{ discard_scene(false, palette) }
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                            }
                            Children [
                                @{ pane_switch_scene(&options, palette) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            ProfileEditorPaneArea { pane: ProfileEditorPane::Profile }
                            Children [
                                @{ diagnostic_pill(has_error, palette) }
                                --
                                Text(diagnostic_text) ProfileEditorDiagnosticText TextRole(Role::Mono)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(space::S4),
                                row_gap: Val::Px(space::S4),
                            }
                            ProfileEditorPaneArea { pane: ProfileEditorPane::Profile }
                            Children [
                                LocalizedText::plain("profiles_snippet_quick_insert") TextRole(Role::Caption)
                                --
                                { snippet_buttons(palette) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                max_height: px(PROFILE_EDITOR_RENDER_LIMIT as f32 * 18.0),
                                padding: UiRect::all(Val::Px(space::S8)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                overflow: Overflow::scroll_y(),
                            }
                            BackgroundColor({ palette.window_clear })
                            ProfileEditorPaneArea { pane: ProfileEditorPane::Profile }
                            Children [
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                }
                                ProfileEditorBody
                                Children [
                                    @{ initial_rows }
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                display: Display::None,
                            }
                            ProfileEditorPaneArea { pane: ProfileEditorPane::Mixin }
                            Children [
                                @{ mixin_pane_scene(&options, palette) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                display: Display::None,
                            }
                            ProfileEditorPaneArea { pane: ProfileEditorPane::Filter }
                            Children [
                                @{ filter_pane_scene(&options, palette) }
                            ]
            }),
        ],
        palette,
    )
}

/// DUAL-09-04: the snippet bar — one button per shared catalogue entry. The
/// labels and bodies come from `infiltrator_contract::yaml_snippets`, so the
/// Bevy bar and the Iced bar offer exactly the same snippets.
fn snippet_buttons(palette: &UiPalette) -> Vec<Box<dyn Scene>> {
    YAML_SNIPPETS
        .iter()
        .enumerate()
        .map(|(index, snippet)| {
            let label = LocalizedText::plain(snippet.label_key);
            let background = palette.surface_elevated;
            Box::new(bsn! {
                            Node {
                                min_height: px(22.0),
                                padding: UiRect::horizontal(Val::Px(space::S6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                            }
                            BackgroundColor({ background })
                            Button
                            EditorMutationControl
                            ProfileEditorSnippetButton { index }
                            Children [
                                label TextRole(Role::Caption)
                            ]
            }) as Box<dyn Scene>
        })
        .collect()
}

fn action_button(label: &'static str, background: Color) -> Box<dyn Scene> {
    let label = LocalizedText::plain(label);
    Box::new(bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
            }
            BackgroundColor({ background })
            Button
            EditorMutationControl
            ProfileEditorFocusButton
            Children [
                label TextRole(Role::Caption)
            ]
    })
}

/// DUAL-09-12: the explicit unlock. The button is always mounted and restamped
/// from the shared protection + the surface's unlock toggle, so a document
/// that loads after the page mount still gets it.
fn protection_toggle(protection: ProfileWriteProtection, palette: &UiPalette) -> Box<dyn Scene> {
    let (label, background) =
        protection_toggle_visual(protection, false, palette, UiLocale::default().code());
    Box::new(bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
            }
            BackgroundColor({ background })
            Button
            EditorMutationControl
            ProfileEditorProtectionToggle
            Children [
                Text({ label }) ProfileEditorProtectionToggleLabel TextRole(Role::Caption)
            ]
    })
}

/// DUAL-09-12: label of the protection toggle, restamped in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorProtectionToggleLabel;

fn reload_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    Box::new(bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
            }
            BackgroundColor({ background })
            Button
            ProfileEditorReloadButton
            Children [
                LocalizedText::plain("common_reload") TextRole(Role::Caption)
            ]
    })
}

fn format_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    Box::new(bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
            }
            BackgroundColor({ background })
            Button
            EditorMutationControl
            ProfileEditorFormatButton
            Children [
                LocalizedText::plain("profiles_format_action") TextRole(Role::Caption)
            ]
    })
}

fn save_button(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.accent;
    Box::new(bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S12)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
            }
            BackgroundColor({ background })
            Button
            EditorMutationControl
            ProfileEditorSaveButton
            Children [
                LocalizedText::plain("dns_save") TextRole(Role::Body)
            ]
    })
}

fn diagnostic_pill(has_error: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let label = profile_editor_projection::syntax_label(has_error, UiLocale::default().code());
    let background = if has_error {
        palette.danger
    } else {
        palette.success
    };
    Box::new(bsn! {
            Node {
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
            }
            BackgroundColor({ background })
            Children [
                Text({ label.to_owned() }) ProfileEditorDiagnosticPill TextRole(Role::Caption)
            ]
    })
}

/// Rebuild the editor body whenever the buffer generation moved.
pub fn refresh_profile_editor_body(
    mut state: ResMut<ProfileEditorState>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<ProfileEditorBody>>,
) {
    if state.last_rendered == state.generation {
        return;
    }
    state.last_rendered = state.generation;
    for entity in &bodies {
        commands.entity(entity).despawn_children();
        let scene = editor_rows_scene(&state, &palette);
        commands.spawn_scene(scene).insert(ChildOf(entity));
    }
}

/// Adopt the shared document and protection whenever the projection updates.
pub fn sync_profile_editor(
    update: On<ProfilesProjectionUpdated>,
    mut state: ResMut<ProfileEditorState>,
) {
    // The card owns no storage: 「重新加载」 asks the shared application for the
    // active profile's document. Nothing is requested implicitly, so a poll can
    // never replace an open buffer behind the user's back.
    let projection = &update.0;
    state
        .session
        .observe_read_status(&projection.editor_read, false);
    if let Some(document) = projection.profile_document.as_ref()
        && state.load_document(document)
    {
        state.notice = None;
        state.format_note = None;
    }
    state
        .session
        .observe_read_status(&projection.editor_read, false);
}

/// Restamp the status line, the syntax banner and its pill from the live
/// buffer state. Compare-and-set, so an idle frame writes nothing.
pub fn restamp_profile_editor(
    state: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    targets: EditorVisualTargets,
) {
    let EditorVisualTargets {
        mut status,
        mut diagnostics,
        mut pills,
        mut toggles,
    } = targets;
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let status_text = status_line(&state, projection, locale.code());
    for mut text in &mut status {
        if text.0 != status_text {
            text.0 = status_text.clone();
        }
    }
    let (diagnostic_text, has_error) = diagnostic_line(&state, locale.code());
    let color = if has_error {
        palette.danger
    } else {
        palette.success
    };
    for (mut text, mut text_color) in &mut diagnostics {
        if text.0 != diagnostic_text {
            text.0 = diagnostic_text.clone();
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
    let pill_label = profile_editor_projection::syntax_label(has_error, locale.code());
    for (mut text, mut background) in &mut pills {
        if text.0 != pill_label {
            text.0 = pill_label.to_owned();
        }
        if background.0 != color {
            background.0 = color;
        }
    }
    let protection = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profile_document.as_ref())
        .map(|document| document.write_protection)
        .unwrap_or_default();
    let (toggle_label, toggle_background) = protection_toggle_visual(
        protection,
        state.protection_override,
        &palette,
        locale.code(),
    );
    for (mut text, mut background) in &mut toggles {
        if text.0 != toggle_label {
            text.0 = toggle_label.clone();
        }
        if background.0 != toggle_background {
            background.0 = toggle_background;
        }
    }
}

/// Grab the keyboard for the editor buffer.
pub fn on_profile_editor_focus(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorFocusButton>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if !state.session.can_edit() {
        return;
    }
    state.focused = !state.focused;
    state.generation = state.generation.wrapping_add(1);
}

/// Reload the stored document through the shared application.
pub fn on_profile_editor_reload(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorReloadButton>>,
    handle: Option<Res<CommandSinkHandle>>,
    state: Res<ProfileEditorState>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::LoadProfileDocument {
        profile: state.session.source().map(|source| source.profile.clone()),
    });
}

/// Format the buffer with the shared AST-preserving formatter.
pub fn on_profile_editor_format(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorFormatButton>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if state.session.can_edit() {
        let _ = state.format();
    }
}

/// DUAL-09-04: insert the catalogue snippet this button stands for.
pub fn on_profile_editor_snippet_activated(
    activate: On<Activate>,
    buttons: Query<&ProfileEditorSnippetButton>,
    mut state: ResMut<ProfileEditorState>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(snippet) = YAML_SNIPPETS.get(button.index) else {
        return;
    };
    if state.session.can_edit() {
        let _ = state.insert_snippet(snippet.id);
    }
}

/// Commit the buffer through the shared guarded write path.
pub fn on_profile_editor_save(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorSaveButton>>,
    mut state: ResMut<ProfileEditorState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if !state.can_save(state.protection) {
        return;
    }
    submit_document(&mut state, &handle);
}

/// Toggle the explicit unlock for a protected subscription.
pub fn on_profile_editor_protection_toggle(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorProtectionToggle>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let protection = state.protection;
    if !protection.is_protected() {
        return;
    }
    if state.session.can_edit() {
        state.protection_override = !state.protection_override;
    }
}

/// Register the editor keyboard seam and its body rebuild.
pub struct ProfilesEditorPlugin;

impl Plugin for ProfilesEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProfileEditorState>();
        // A headless composition has no InputPlugin, so the message type is
        // registered here; a windowed composition reuses the same queue.
        app.add_message::<KeyboardInput>();
        app.add_observer(on_profile_editor_focus);
        app.add_observer(on_profile_editor_reload);
        app.add_observer(on_profile_editor_format);
        app.add_observer(on_profile_editor_snippet_activated);
        app.add_observer(on_profile_editor_save);
        app.add_observer(on_profile_editor_protection_toggle);
        app.add_observer(sync_profile_editor);
        app.add_systems(
            Update,
            (
                route_editor_keyboard,
                refresh_profile_editor_body,
                restamp_profile_editor,
                replay_document_copy,
            )
                .chain(),
        );
    }
}
