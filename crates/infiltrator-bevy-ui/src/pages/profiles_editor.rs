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

use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
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
use bevy::scene::{CommandsSceneExt, Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::editor::{CodeEditorState, SyntaxTokenKind, tokenize_yaml_line};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{LastProfilesProjection, ProfilesProjection};
use crate::pages::profiles_editor_state::{
    PROFILE_EDITOR_RENDER_LIMIT, ProfileEditorState, diagnostic_line, protection_toggle_visual,
    status_line,
};

/// Marker for the editor card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorRoot;

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
    let state = ProfileEditorState {
        profile: document
            .map(|document| document.profile.clone())
            .unwrap_or_default(),
        buffer: CodeEditorState::new(
            document
                .map(|document| document.content.as_str())
                .unwrap_or(""),
        ),
        dirty: false,
        loaded_content: document.map(|document| document.content.clone()),
        diagnostic: document.and_then(|document| document.syntax.clone()),
        notice: None,
        protection_override: false,
        focused: false,
        generation: 0,
        // The first Update pass renders the body from this state.
        last_rendered: u64::MAX,
    };
    let status = status_line(&state, Some(projection));
    let (diagnostic_text, has_error) = diagnostic_line(&state);
    let initial_rows = editor_rows_scene(&state, palette);

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
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S4),
                                }
                                Children [
                                    ( Text({ format!("配置文档编辑器 · YAML ({buffer_lines} 行)") }) TextRole(Role::BodyStrong) ),
                                    ( Text(status) ProfileEditorStatusText TextRole(Role::Caption) ),
                                    ( Text({ protection.label_zh().to_owned() }) ProfileEditorProtectionText TextRole(Role::Caption) ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { action_button("编辑", palette.surface_elevated) } ),
                            ( { protection_toggle(protection, palette) } ),
                            ( { reload_button(palette) } ),
                            ( { format_button(palette) } ),
                            ( { save_button(palette) } ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( { diagnostic_pill(has_error, palette) } ),
                    ( Text(diagnostic_text) ProfileEditorDiagnosticText TextRole(Role::Mono) ),
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
                Children [
                    ( Text({ "快速插入片段（共享目录）".to_owned() }) TextRole(Role::Caption) ),
                    { snippet_buttons(palette) },
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
                Children [
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                        }
                        ProfileEditorBody
                        Children [
                            ( { initial_rows } ),
                        ]
                    ),
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
            let label = snippet.label_zh.to_owned();
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
                template_value(ProfileEditorSnippetButton { index })
                Children [
                    ( Text({ label }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect()
}

fn action_button(label: &str, background: Color) -> Box<dyn Scene> {
    let label = label.to_owned();
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
        ProfileEditorFocusButton
        Children [
            ( Text({ label }) TextRole(Role::Caption) ),
        ]
    })
}

/// DUAL-09-12: the explicit unlock. The button is always mounted and restamped
/// from the shared protection + the surface's unlock toggle, so a document
/// that loads after the page mount still gets it.
fn protection_toggle(protection: ProfileWriteProtection, palette: &UiPalette) -> Box<dyn Scene> {
    let (label, background) = protection_toggle_visual(protection, false, palette);
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
        ProfileEditorProtectionToggle
        Children [
            ( Text({ label }) ProfileEditorProtectionToggleLabel TextRole(Role::Caption) ),
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
            ( Text({ "重新加载".to_owned() }) TextRole(Role::Caption) ),
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
        ProfileEditorFormatButton
        Children [
            ( Text({ "格式化（共享保真）".to_owned() }) TextRole(Role::Caption) ),
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
        ProfileEditorSaveButton
        Children [
            ( Text({ "保存并应用".to_owned() }) TextRole(Role::Body) ),
        ]
    })
}

fn diagnostic_pill(has_error: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let (label, background) = if has_error {
        ("语法错误", palette.danger)
    } else {
        ("语法通过", palette.success)
    };
    Box::new(bsn! {
        Node {
            padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ background })
        Children [
            ( Text({ label.to_owned() }) ProfileEditorDiagnosticPill TextRole(Role::Caption) ),
        ]
    })
}

fn token_color(kind: SyntaxTokenKind, palette: &UiPalette) -> Color {
    match kind {
        SyntaxTokenKind::Comment => palette.ink_dim,
        SyntaxTokenKind::Keyword => palette.accent,
        SyntaxTokenKind::StringLiteral => palette.success,
        SyntaxTokenKind::NumberLiteral => palette.warning,
        SyntaxTokenKind::Punctuation => palette.ink_dim,
        SyntaxTokenKind::Plain => palette.ink,
    }
}

/// Gutter + code rows for the current window, with the cursor line washed and
/// the diagnostic line painted in the danger token. DUAL-09-02: the window,
/// the hidden-line counts and the indentation levels come from the shared
/// `EditorViewport`, so this surface and the Iced editor never drift.
fn editor_rows_scene(state: &ProfileEditorState, palette: &UiPalette) -> Box<dyn Scene> {
    let viewport = state.viewport();
    let diagnostic_row = state.diagnostic.as_ref().map(|diagnostic| diagnostic.line);
    let cursor_row = state.buffer.cursor_row + 1;
    let mut rows: Vec<Box<dyn Scene>> = Vec::with_capacity(viewport.rendered_len() + 2);
    if viewport.hidden_above() > 0 {
        rows.push(notice_row(
            &format!(
                "… 上方还有 {} 行未渲染（跟随光标的有界窗口）",
                viewport.hidden_above()
            ),
            palette,
        ));
    }
    for (number, line, indent_level) in state.rendered_lines() {
        let is_diagnostic = diagnostic_row == Some(number);
        let is_cursor = cursor_row == number && state.focused;
        let background = if is_diagnostic {
            palette.danger
        } else if is_cursor {
            palette.surface_elevated
        } else {
            palette.window_clear
        };
        let gutter_color = if is_diagnostic {
            palette.window_clear
        } else {
            palette.ink_dim
        };
        let tokens: Vec<Box<dyn Scene>> = tokenize_yaml_line(line)
            .into_iter()
            .map(|token| {
                let color = if is_diagnostic {
                    palette.window_clear
                } else {
                    token_color(token.kind, palette)
                };
                Box::new(bsn! {
                    (
                        Text({ token.text })
                        TextRole(Role::Mono)
                        TextColor({ color })
                    )
                }) as Box<dyn Scene>
            })
            .collect();
        rows.push(Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(space::S8),
                min_height: px(16.0),
            }
            BackgroundColor({ background })
            Children [
                (
                    Node {
                        min_width: px(36.0),
                        justify_content: JustifyContent::FlexEnd,
                    }
                    Children [
                        (
                            Text({ format!("{number}") })
                            TextRole(Role::Mono)
                            TextColor({ gutter_color })
                        ),
                    ]
                ),
                ( { indent_rail(indent_level, palette) } ),
                { tokens },
            ]
        }));
    }
    if viewport.hidden_below() > 0 {
        rows.push(notice_row(
            &format!(
                "… 下方还有 {} 行未渲染（有界窗口，不是虚拟滚动）",
                viewport.hidden_below()
            ),
            palette,
        ));
    }
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
        }
        Children [
            { rows },
        ]
    })
}

/// DUAL-09-02: the shared indentation reference, rendered as a rail at the
/// start of the row (one tick per closed indentation level).
fn indent_rail(level: usize, palette: &UiPalette) -> Box<dyn Scene> {
    let color = palette.accent_container;
    let mut ticks: Vec<Box<dyn Scene>> = Vec::with_capacity(level);
    for _ in 0..level {
        ticks.push(Box::new(bsn! {
            Node {
                width: px(2.0),
                height: px(10.0),
                margin: UiRect::right(Val::Px(2.0)),
            }
            BackgroundColor({ color })
        }) as Box<dyn Scene>);
    }
    Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            min_width: px(4.0),
        }
        Children [
            { ticks },
        ]
    })
}

fn notice_row(text: &str, palette: &UiPalette) -> Box<dyn Scene> {
    let label = text.to_owned();
    let color = palette.warning;
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
        Children [
            ( Text({ label }) TextRole(Role::Caption) TextColor({ color }) ),
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
    update: On<crate::pages::profiles::ProfilesProjectionUpdated>,
    mut state: ResMut<ProfileEditorState>,
    mut protections: Query<&mut Text, With<ProfileEditorProtectionText>>,
) {
    // The card owns no storage: 「重新加载」 asks the shared application for the
    // active profile's document. Nothing is requested implicitly, so a poll can
    // never replace an open buffer behind the user's back.
    let projection = &update.0;
    if let Some(document) = projection.profile_document.as_ref()
        && state.load_document(document)
    {
        state.notice = None;
    }
    let protection = projection
        .profile_document
        .as_ref()
        .map(|document| document.write_protection)
        .unwrap_or_default();
    let label = match state.notice.as_deref() {
        Some(notice) => format!(
            "{} · {} · {notice}",
            protection.label_zh(),
            protection.hint_zh()
        ),
        None => format!("{} · {}", protection.label_zh(), protection.hint_zh()),
    };
    for mut text in &mut protections {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

/// Restamp the status line, the syntax banner and its pill from the live
/// buffer state. Compare-and-set, so an idle frame writes nothing.
#[allow(clippy::type_complexity)]
pub fn restamp_profile_editor(
    state: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    palette: Res<UiPalette>,
    mut status: Query<
        &mut Text,
        (
            With<ProfileEditorStatusText>,
            Without<ProfileEditorProtectionText>,
            Without<ProfileEditorDiagnosticText>,
            Without<ProfileEditorDiagnosticPill>,
        ),
    >,
    mut diagnostics: Query<
        (&mut Text, &mut TextColor),
        (
            With<ProfileEditorDiagnosticText>,
            Without<ProfileEditorStatusText>,
            Without<ProfileEditorDiagnosticPill>,
        ),
    >,
    mut pills: Query<
        (&mut Text, &mut BackgroundColor),
        (
            With<ProfileEditorDiagnosticPill>,
            Without<ProfileEditorStatusText>,
            Without<ProfileEditorDiagnosticText>,
            Without<ProfileEditorProtectionToggleLabel>,
        ),
    >,
    mut toggles: Query<
        (&mut Text, &mut BackgroundColor),
        (
            With<ProfileEditorProtectionToggleLabel>,
            Without<ProfileEditorStatusText>,
            Without<ProfileEditorDiagnosticText>,
            Without<ProfileEditorDiagnosticPill>,
        ),
    >,
) {
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let status_text = status_line(&state, projection);
    for mut text in &mut status {
        if text.0 != status_text {
            text.0 = status_text.clone();
        }
    }
    let (diagnostic_text, has_error) = diagnostic_line(&state);
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
    let pill_label = if has_error {
        "语法错误"
    } else {
        "语法通过"
    };
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
    let (toggle_label, toggle_background) =
        protection_toggle_visual(protection, state.protection_override, &palette);
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
    state.focused = !state.focused;
    state.generation = state.generation.wrapping_add(1);
}

/// Reload the stored document through the shared application.
pub fn on_profile_editor_reload(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorReloadButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::LoadProfileDocument { profile: None });
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
    let _ = state.format();
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
    let _ = state.insert_snippet(snippet.id);
}

/// Commit the buffer through the shared guarded write path.
pub fn on_profile_editor_save(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorSaveButton>>,
    state: Res<ProfileEditorState>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let protection = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profile_document.as_ref())
        .map(|document| document.write_protection)
        .unwrap_or_default();
    if !state.can_save(protection) {
        return;
    }
    handle.submit(UiCommand::SaveProfileDocument {
        profile: state.profile.clone(),
        content: state.buffer.full_text(),
        allow_protected: state.protection_override,
    });
}

/// Toggle the explicit unlock for a protected subscription.
pub fn on_profile_editor_protection_toggle(
    activate: On<Activate>,
    buttons: Query<(), With<ProfileEditorProtectionToggle>>,
    last: Option<Res<LastProfilesProjection>>,
    mut state: ResMut<ProfileEditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let protection = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profile_document.as_ref())
        .map(|document| document.write_protection)
        .unwrap_or_default();
    if !protection.is_protected() {
        return;
    }
    state.protection_override = !state.protection_override;
}

/// The editor keyboard seam: the buffer owns printable keys, Backspace,
/// Delete, Enter, Tab, arrows, Home/End and Ctrl+S while focused.
pub fn profile_editor_keyboard_input(
    mut keys: MessageReader<KeyboardInput>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<ProfileEditorState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if !state.focused {
        keys.clear();
        return;
    }
    let modifiers = keyboard
        .as_deref()
        .map(crate::shortcuts::modifiers_from_keyboard)
        .unwrap_or_default();
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            // Ctrl+S commits; every other modified chord falls through to the
            // global shortcut registry.
            let is_save = modifiers.ctrl
                && !modifiers.alt
                && !modifiers.meta
                && matches!(&key.logical_key, Key::Character(text) if text.eq_ignore_ascii_case("s"));
            if is_save && let Some(handle) = handle.as_ref() {
                handle.submit(UiCommand::SaveProfileDocument {
                    profile: state.profile.clone(),
                    content: state.buffer.full_text(),
                    allow_protected: state.protection_override,
                });
            }
            continue;
        }
        match &key.logical_key {
            Key::Character(text) => state.insert_text(text),
            Key::Space => state.insert_text(" "),
            Key::Enter => state.insert_text("\n"),
            Key::Tab => state.insert_text("  "),
            Key::Backspace => state.backspace(),
            Key::Delete => state.delete_forward(),
            Key::ArrowUp => state.move_cursor(true),
            Key::ArrowDown => state.move_cursor(false),
            Key::ArrowLeft => {
                state.buffer.move_left();
                state.generation += 1;
            }
            Key::ArrowRight => {
                state.buffer.move_right();
                state.generation += 1;
            }
            Key::Home => {
                state.buffer.move_home();
                state.generation += 1;
            }
            Key::End => {
                state.buffer.move_end();
                state.generation += 1;
            }
            Key::Escape => state.focused = false,
            _ => {}
        }
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
                profile_editor_keyboard_input,
                refresh_profile_editor_body,
                restamp_profile_editor,
            )
                .chain(),
        );
    }
}
