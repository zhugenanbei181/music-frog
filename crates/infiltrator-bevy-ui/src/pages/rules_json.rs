//! DUAL-11-14: the Bevy rules-workspace JSON partition.
//!
//! Iced edits three JSON section projections (rule-providers, proxy-providers,
//! sniffer) through `text_editor` buffers and typed configuration commands.
//! This module gives the Bevy
//! surface the same capability: the documents arrive through the shared read
//! model (`RulesPageSnapshot.json_documents`, serialised from the same profile
//! the Iced editor loads), the buffer is the shared-widget
//! [`CodeEditorState`], and saving decodes text into a typed update bound to
//! the exact profile revision published with that text.
//!
//! **Honest boundary**: the Bevy command bus is fire-and-forget, so the status
//! line reports that a save was *submitted* and reports 「已与读模型一致」 only
//! once the read model publishes the submitted text back.

use crate::a11y::button_semantic_node;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::page_models::RulesPageModelState;
use crate::pages::rules::RulesProjectionUpdated;
use crate::shortcuts::modifiers_from_keyboard;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::scene::{CommandsSceneExt, Ready, Scene, SceneList, bsn, bsn_list, on};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::editor::code_editor_scene;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::profile_configuration::{
    ProfileConfigurationRequest, ProfileConfigurationUpdate,
};
use infiltrator_contract::rule_json_feedback::RuleJsonFeedback;
use infiltrator_contract::rules_workspace::{
    RulesJsonDocumentSnapshot, RulesJsonSection, RulesTab,
};
use infiltrator_presentation::rule_json_projection::json_editor_status;
use infiltrator_presentation::rules_page_model::{RulesOperation, RulesPageModel};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::profile_document_identity::ProfileDocumentIdentity;

/// Which JSON document the partition shows, plus its edit buffer.
///
/// The *owner* of the JSON text is the shared rules page model: it holds the
/// three document buffers as typed drafts with their committed baseline, so
/// "the buffer holds an unaccepted edit" is a derived fact of the draft and
/// its baseline — never a boolean beside the text.
#[derive(Resource)]
pub struct RulesJsonState {
    /// Active section of the partition.
    pub section: RulesJsonSection,
    /// One editor buffer per shared section, indexed by `section.index()`.
    ///
    /// The buffer is the editor widget's own state (cursor, selection); the
    /// text it shows is the shared model's draft.
    pub buffers: Vec<CodeEditorState>,
    /// Keyboard ownership: only a focused buffer consumes keys.
    pub focused: bool,
    /// Bumped on every buffer replacement so the body scene is rebuilt.
    pub generation: u64,
    /// `generation` the mounted body was built from.
    pub last_rendered: u64,
    /// Last published documents (the shared read model).
    pub published: Vec<RulesJsonDocumentSnapshot>,
    /// Source identity paired with each local buffer. A buffer with an
    /// unaccepted edit retains the revision from which editing began when a
    /// newer read arrives.
    pub buffer_identities: Vec<Option<ProfileDocumentIdentity>>,
    /// Honest status line (submitted / adopted / refused).
    pub status: Option<RuleJsonFeedback>,
}

impl Default for RulesJsonState {
    fn default() -> Self {
        Self {
            section: RulesJsonSection::default(),
            buffers: RulesJsonSection::ALL
                .iter()
                .map(|_| CodeEditorState::new(""))
                .collect(),
            focused: false,
            generation: 0,
            last_rendered: 0,
            published: Vec::new(),
            buffer_identities: vec![None; RulesJsonSection::ALL.len()],
            status: None,
        }
    }
}

impl RulesJsonState {
    /// Buffer of the active section.
    pub fn buffer(&self) -> &CodeEditorState {
        &self.buffers[self.section.index()]
    }

    /// Buffer of the active section, mutably.
    pub fn buffer_mut(&mut self) -> &mut CodeEditorState {
        let index = self.section.index();
        &mut self.buffers[index]
    }

    /// The published document of the active section, when the read model has
    /// one.
    pub fn published_document(&self) -> Option<&str> {
        self.published
            .iter()
            .find(|document| document.section == self.section)
            .map(|document| document.json.as_str())
    }

    fn ensure_buffers(&mut self) {
        if self.buffers.len() == RulesJsonSection::ALL.len() {
            return;
        }
        self.buffers = RulesJsonSection::ALL
            .iter()
            .map(|_| CodeEditorState::new(""))
            .collect();
        self.buffer_identities = vec![None; RulesJsonSection::ALL.len()];
        self.generation = self.generation.wrapping_add(1);
    }

    /// Adopt the published documents into the shared model.
    ///
    /// A buffer the user edited is never clobbered: the published text
    /// becomes the model's new baseline and the edit stays visible.
    pub fn adopt(&mut self, documents: &[RulesJsonDocumentSnapshot], model: &mut RulesPageModel) {
        self.published = documents.to_vec();
        self.ensure_buffers();
        model.observe_json_documents(documents);
        for document in documents {
            let index = document.section.index();
            let current = self.buffers[index].full_text();
            let modified = model.json_modified(document.section);
            let same_value = modified
                && match (
                    typed_rules_update(document.section, &current),
                    typed_rules_update(document.section, &document.json),
                ) {
                    (Ok(left), Ok(right)) => left == right,
                    _ => false,
                };
            if !modified || same_value {
                if same_value {
                    // The read model confirms the typed value; its
                    // pretty-printer may normalize whitespace from the local
                    // buffer, so the published text becomes the draft.
                    model
                        .json_buffer_mut(document.section)
                        .edit(document.json.clone());
                }
                if current != document.json {
                    self.buffers[index] = CodeEditorState::new(&document.json);
                    self.generation = self.generation.wrapping_add(1);
                }
                if let Some(identity) = self.buffer_identities.get_mut(index) {
                    *identity = Some(document.identity.clone());
                }
                continue;
            }
            // The user still holds an unaccepted edit: their bytes and the
            // revision they were read from stay.
        }
    }

    /// DUAL-11-14: apply one keystroke to the focused buffer. An unfocused
    /// buffer never consumes a key, so a hidden editor cannot be edited.
    ///
    /// The keystroke lands in the editor widget; the shared model owns the
    /// resulting draft and its baseline.
    pub fn apply_key(&mut self, key: &Key, model: &mut RulesPageModel) -> bool {
        if !self.focused {
            return false;
        }
        let buffer = self.buffer_mut();
        match key {
            Key::Character(text) => buffer.insert_text(text),
            Key::Space => buffer.insert_text(" "),
            Key::Enter => buffer.insert_text("\n"),
            Key::Tab => buffer.insert_text("  "),
            Key::Backspace => buffer.delete_backwards(),
            Key::Delete => {
                buffer.delete_forward();
            }
            Key::ArrowUp => {
                buffer.move_up();
            }
            Key::ArrowDown => {
                buffer.move_down();
            }
            Key::ArrowLeft => {
                buffer.move_left();
            }
            Key::ArrowRight => {
                buffer.move_right();
            }
            Key::Home => {
                buffer.move_home();
            }
            Key::End => {
                buffer.move_end();
            }
            Key::Escape => {
                self.focused = false;
                return true;
            }
            _ => return false,
        }
        let text = self.buffers[self.section.index()].full_text().to_owned();
        model.json_buffer_mut(self.section).edit(text);
        self.status = None;
        true
    }

    /// Build the save command for the active section, if the buffer can be
    /// submitted.
    fn submit_command(&self) -> Result<UiCommand, RulesJsonSubmissionFailure> {
        let json = self.buffer().full_text();
        if json.trim().is_empty() {
            return Err(RulesJsonSubmissionFailure::Empty);
        }
        let identity = self
            .buffer_identities
            .get(self.section.index())
            .and_then(Clone::clone)
            .ok_or(RulesJsonSubmissionFailure::SourceUnavailable)?;
        let update = typed_rules_update(self.section, &json)
            .map_err(|()| RulesJsonSubmissionFailure::InvalidDocument)?;
        Ok(UiCommand::ApplyProfileConfiguration(Box::new(
            ProfileConfigurationRequest { identity, update },
        )))
    }

    /// Status text for the active section.
    ///
    /// The "modified" fact is the shared model's: it is derived from the
    /// draft against its committed baseline, never a flag beside the text.
    pub fn status_label(&self, language: &str, modified: bool) -> String {
        json_editor_status(
            self.focused,
            modified,
            self.published_document().is_some(),
            self.status,
            &Lang(language),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RulesJsonSubmissionFailure {
    Empty,
    SourceUnavailable,
    InvalidDocument,
}

fn typed_rules_update(
    section: RulesJsonSection,
    json: &str,
) -> Result<ProfileConfigurationUpdate, ()> {
    match section {
        RulesJsonSection::RuleProviders => serde_json::from_str(json)
            .map(ProfileConfigurationUpdate::RuleProviders)
            .map_err(|_| ()),
        RulesJsonSection::ProxyProviders => serde_json::from_str(json)
            .map(ProfileConfigurationUpdate::ProxyProviders)
            .map_err(|_| ()),
        RulesJsonSection::Sniffer => serde_json::from_str(json)
            .map(ProfileConfigurationUpdate::Sniffer)
            .map_err(|_| ()),
    }
}

/// Marker on a JSON section chip; payload is the shared section index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonSectionChip(pub usize);

/// Marker on a section chip label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonSectionLabel(pub usize);

/// The container whose children are the rendered lines of the active buffer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonEditorBody;

/// Grabs/releases the keyboard for the editor buffer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonEditButton;

/// Submits the active buffer through the shared application.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonSaveButton;

/// The honest status line of the partition.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonStatusText;

/// Marker on the save button's label; it names the active section.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonSaveLabel;

/// Fixed height of the JSON editor panel (px); the partition scrolls inside it.
const JSON_EDITOR_BODY_HEIGHT_PX: f32 = 320.0;

fn section_chip_scene(section: RulesJsonSection, palette: &UiPalette) -> Box<dyn Scene> {
    let index = section.index();
    let selected = index == 0;
    Box::new(bsn! {
            Node {
                min_height: px(palette.control_height_px * 0.85),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ if selected { Token::Accent } else { Token::SurfaceElevated } })
            Button
            RulesJsonSectionChip(index)
            Children [
                LocalizedText::plain(section.i18n_key())
                TextRole(Role::Caption)
                ThemeTextColor({ if selected { Token::OnAccent } else { Token::InkDim } })
                RulesJsonSectionLabel(index)
            ]
    })
}

fn edit_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Border })
            Button
            RulesJsonEditButton
            Children [
                LocalizedText::plain("profiles_editor_buffer_title") TextRole(Role::Body)
            ]
    })
}

fn save_button_scene(section: RulesJsonSection, palette: &UiPalette) -> Box<dyn Scene> {
    let semantic = button_semantic_node("");
    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Accent })
            Button
            RulesJsonSaveButton
            semantic LocalizedLabel::plain(section.save_i18n_key())
            Children [
                LocalizedText::plain(section.save_i18n_key())
                TextRole(Role::BodyStrong)
                RulesJsonSaveLabel
            ]
    })
}

/// The section chips as one scene list: a repeated group, so it is authored as
/// a [`SceneList`] instead of an inline `Children [...]` enumeration.
fn section_chip_scenes(palette: &UiPalette) -> impl SceneList + use<> {
    let chips: Vec<Box<dyn Scene>> = RulesJsonSection::ALL
        .iter()
        .map(|section| section_chip_scene(*section, palette))
        .collect();
    bsn_list! { { chips } }
}

/// Scene constructor for the JSON partition of the rules workspace.
pub fn rules_json_scene(palette: &UiPalette, state: &RulesJsonState) -> impl Scene + use<> {
    let status = state.status_label("en-US", false);
    let body: Box<dyn Scene> = code_editor_scene(state.buffer(), palette);
    let edit_button = edit_button_scene(palette);
    let save_button = save_button_scene(state.section, palette);

    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(rem(space::S8)),
                }
                Children [
                    LocalizedText::plain("rules_json_editor_title") TextRole(Role::BodyStrong)
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        { vec![edit_button, save_button] }
                    ]
                ]
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S8),
                }
                Children [
                    { section_chip_scenes(palette) }
                ]
                --
                Text({ status }) RulesJsonStatusText TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    height: rem(JSON_EDITOR_BODY_HEIGHT_PX),
                    padding: UiRect::all(rem(space::S8)),
                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                ThemeBackgroundColor({ Token::WindowClear })
                Children [
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                    }
                    RulesJsonEditorBody
                    on(render_editor_body)
                    Children [
                        { vec![body] }
                    ]
                ]
            ]
    }
}

/// Project the active buffer into a freshly mounted editor body.
///
/// The partition scene is built from a default state, so a mounted body starts
/// out empty; [`bevy_scene::Ready`] is the seam that says "this body and its
/// whole subtree exist now". Waiting for the body to be mounted in the same
/// frame the buffer changed would leave the editor showing stale content.
fn render_editor_body(
    ready: On<Ready>,
    mut state: Option<ResMut<RulesJsonState>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    commands.entity(ready.entity).despawn_children();
    commands
        .spawn_scene(code_editor_scene(state.buffer(), &palette))
        .insert(ChildOf(ready.entity));
    state.last_rendered = state.generation;
}

/// DUAL-11-14: restamp the partition chrome (status line, save label, section
/// chip fills) from the state. The editor body is rebuilt separately by
/// [`refresh_rules_json_body`]; everything else is compare-and-set in place.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct JsonSaveCopy {
    text: &'static mut Text,
    copy: &'static mut LocalizedText,
}
#[derive(SystemParam)]
pub struct JsonChrome<'w, 's> {
    locale: Res<'w, UiLocale>,
    state: Option<Res<'w, RulesJsonState>>,
    model: Option<Res<'w, RulesPageModelState>>,
    palette: Res<'w, UiPalette>,
    statuses:
        Query<'w, 's, &'static mut Text, (With<RulesJsonStatusText>, Without<RulesJsonSaveLabel>)>,
    save_labels:
        Query<'w, 's, JsonSaveCopy, (With<RulesJsonSaveLabel>, Without<RulesJsonStatusText>)>,
    save_buttons: Query<'w, 's, &'static mut LocalizedLabel, With<RulesJsonSaveButton>>,
    chips: Query<'w, 's, (&'static mut BackgroundColor, &'static RulesJsonSectionChip)>,
    section_labels: Query<'w, 's, (&'static mut TextColor, &'static RulesJsonSectionLabel)>,
}
pub fn restamp_rules_json(chrome: JsonChrome) {
    let JsonChrome {
        locale,
        state,
        model,
        palette,
        mut statuses,
        mut save_labels,
        mut save_buttons,
        mut chips,
        mut section_labels,
    } = chrome;
    let Some(state) = state else {
        return;
    };
    let modified = model.is_some_and(|model| model.0.json_modified(state.section));
    let status = state.status_label(locale.code(), modified);
    for mut text in &mut statuses {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
    let save = Lang(locale.code())
        .tr(state.section.save_i18n_key())
        .into_owned();
    for mut label in &mut save_labels {
        label.copy.key = state.section.save_i18n_key();
        if label.text.0 != save {
            label.text.0 = save.clone();
        }
    }
    for mut label in &mut save_buttons {
        label.0.key = state.section.save_i18n_key();
    }
    for (mut background, chip) in &mut chips {
        let selected = RulesJsonSection::from_index(chip.0) == state.section;
        let want = if selected {
            palette.accent
        } else {
            palette.surface_elevated
        };
        if background.0 != want {
            background.0 = want;
        }
    }
    for (mut color, label) in &mut section_labels {
        let selected = RulesJsonSection::from_index(label.0) == state.section;
        let want = if selected {
            palette.on_accent
        } else {
            palette.ink_dim
        };
        if color.0 != want {
            color.0 = want;
        }
    }
}

/// DUAL-11-14: adopt the documents the shared reader published. Runs on every
/// rules projection update, so the Bevy editor always starts from the same
/// text the Iced editor loads.
pub fn sync_rules_json(
    update: On<RulesProjectionUpdated>,
    mut state: Option<ResMut<RulesJsonState>>,
    mut model: Option<ResMut<RulesPageModelState>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let Some(model) = model.as_deref_mut() else {
        return;
    };
    state.adopt(&update.0.json_documents, &mut model.0);
}

/// Rebuild the editor body whenever the active buffer was replaced.
pub fn refresh_rules_json_body(
    mut state: Option<ResMut<RulesJsonState>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    bodies: Query<Entity, With<RulesJsonEditorBody>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if state.last_rendered == state.generation {
        return;
    }
    state.last_rendered = state.generation;
    for body in &bodies {
        commands.entity(body).despawn_children();
        commands
            .spawn_scene(code_editor_scene(state.buffer(), &palette))
            .insert(ChildOf(body));
    }
}

/// Section chip / edit / save activation.
pub fn on_rules_json_action_activated(
    activate: On<Activate>,
    chips: Query<&RulesJsonSectionChip>,
    edit_buttons: Query<(), With<RulesJsonEditButton>>,
    save_buttons: Query<(), With<RulesJsonSaveButton>>,
    mut state: Option<ResMut<RulesJsonState>>,
    mut model: Option<ResMut<RulesPageModelState>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    state.ensure_buffers();
    if let Ok(chip) = chips.get(activate.entity) {
        let section = RulesJsonSection::from_index(chip.0);
        if state.section != section {
            state.section = section;
            if let Some(model) = model.as_deref_mut() {
                model.0.select_json_section(section);
            }
            state.status = None;
            state.generation = state.generation.wrapping_add(1);
        }
        return;
    }
    if edit_buttons.contains(activate.entity) {
        state.focused = !state.focused;
        state.status = None;
        return;
    }
    if !save_buttons.contains(activate.entity) {
        return;
    }
    let Some(handle) = handle else {
        state.status = Some(RuleJsonFeedback::HostUnavailable);
        return;
    };
    let command = match state.submit_command() {
        Ok(command) => command,
        Err(error) => {
            state.status = Some(match error {
                RulesJsonSubmissionFailure::Empty => RuleJsonFeedback::Empty,
                RulesJsonSubmissionFailure::SourceUnavailable => {
                    RuleJsonFeedback::SourceUnavailable
                }
                RulesJsonSubmissionFailure::InvalidDocument => RuleJsonFeedback::InvalidDocument,
            });
            return;
        }
    };
    handle.submit(command);
    if let Some(model) = model.as_deref_mut() {
        model.0.operation_mut(save_operation(state.section)).begin();
    }
    state.status = Some(RuleJsonFeedback::AwaitingReadback);
}

/// The shared rules operation that saves one JSON section.
fn save_operation(section: RulesJsonSection) -> RulesOperation {
    RulesOperation::ALL
        .into_iter()
        .find(|operation| operation.json_section() == Some(section))
        .unwrap_or(RulesOperation::Save)
}

/// The editor keyboard seam: only the focused buffer consumes keys, and only
/// while the partition body is mounted. Ctrl+S submits the active section.
pub fn rules_json_keyboard_input(
    mut keys: MessageReader<KeyboardInput>,
    keys_state: Option<Res<ButtonInput<KeyCode>>>,
    bodies: Query<(), With<RulesJsonEditorBody>>,
    mut state: Option<ResMut<RulesJsonState>>,
    mut model: Option<ResMut<RulesPageModelState>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let pressed: Vec<Key> = keys
        .read()
        .filter(|key| key.state == ButtonState::Pressed)
        .map(|key| key.logical_key.clone())
        .collect();
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if !state.focused || bodies.is_empty() {
        return;
    }
    // A hidden partition never owns the keyboard, even if focus was left on.
    // The tab is the shared model's fact.
    if model
        .as_ref()
        .is_some_and(|model| model.0.tab() != RulesTab::JsonEditors)
    {
        return;
    }
    let modifiers = keys_state
        .as_deref()
        .map(modifiers_from_keyboard)
        .unwrap_or_default();
    for key in pressed {
        if modifiers.ctrl && matches!(&key, Key::Character(text) if text.eq_ignore_ascii_case("s"))
        {
            match (handle.as_ref(), state.submit_command()) {
                (None, _) => state.status = Some(RuleJsonFeedback::HostUnavailable),
                (Some(_), Err(error)) => {
                    state.status = Some(match error {
                        RulesJsonSubmissionFailure::Empty => RuleJsonFeedback::Empty,
                        RulesJsonSubmissionFailure::SourceUnavailable => {
                            RuleJsonFeedback::SourceUnavailable
                        }
                        RulesJsonSubmissionFailure::InvalidDocument => {
                            RuleJsonFeedback::InvalidDocument
                        }
                    });
                }
                (Some(handle), Ok(command)) => {
                    handle.submit(command);
                    if let Some(model) = model.as_deref_mut() {
                        model.0.operation_mut(save_operation(state.section)).begin();
                    }
                    state.status = Some(RuleJsonFeedback::AwaitingReadback);
                }
            }
            continue;
        }
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            continue;
        }
        let Some(model) = model.as_deref_mut() else {
            return;
        };
        if state.apply_key(&key, &mut model.0) {
            state.generation = state.generation.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod rules_json_tests;
