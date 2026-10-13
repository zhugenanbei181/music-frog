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
use crate::pages::rules::RulesProjectionUpdated;
use crate::pages::rules_tabs::RulesTabState;
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
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::rule_json_projection::json_editor_status;
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
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::rule_json_feedback::RuleJsonFeedback;
use infiltrator_contract::rules_workspace::{
    RulesJsonDocumentSnapshot, RulesJsonSection, RulesTab,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// Which JSON document the partition shows, plus its edit buffer.
#[derive(Resource)]
pub struct RulesJsonState {
    /// Active section of the partition.
    pub section: RulesJsonSection,
    /// One editor buffer per shared section, indexed by `section.index()`.
    pub buffers: Vec<CodeEditorState>,
    /// Whether the buffer holds text the read model does not (yet) reflect.
    pub dirty: Vec<bool>,
    /// Keyboard ownership: only a focused buffer consumes keys.
    pub focused: bool,
    /// Bumped on every buffer replacement so the body scene is rebuilt.
    pub generation: u64,
    /// `generation` the mounted body was built from.
    pub last_rendered: u64,
    /// Last published documents (the shared read model).
    pub published: Vec<RulesJsonDocumentSnapshot>,
    /// Source identity paired with each local buffer. Dirty buffers retain the
    /// revision from which editing began when a newer read arrives.
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
            dirty: vec![false; RulesJsonSection::ALL.len()],
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
        self.dirty = vec![false; RulesJsonSection::ALL.len()];
        self.buffer_identities = vec![None; RulesJsonSection::ALL.len()];
        self.generation = self.generation.wrapping_add(1);
    }

    /// Adopt the published documents. A buffer the user edited is never
    /// clobbered: it stays dirty until it is saved or the user switches away.
    pub fn adopt(&mut self, documents: &[RulesJsonDocumentSnapshot]) {
        self.published = documents.to_vec();
        self.ensure_buffers();
        for document in documents {
            let index = document.section.index();
            let current = self.buffers[index].full_text();
            let same_value = self.dirty.get(index).copied().unwrap_or(false)
                && match (
                    typed_rules_update(document.section, &current),
                    typed_rules_update(document.section, &document.json),
                ) {
                    (Ok(left), Ok(right)) => left == right,
                    _ => false,
                };
            if current == document.json || same_value {
                // The read model confirms the typed value; its pretty-printer
                // may normalize whitespace from the local buffer.
                if let Some(dirty) = self.dirty.get_mut(index) {
                    *dirty = false;
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
            if self.dirty.get(index).copied().unwrap_or(false) {
                continue;
            }
            self.buffers[index] = CodeEditorState::new(&document.json);
            self.buffer_identities[index] = Some(document.identity.clone());
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// DUAL-11-14: apply one keystroke to the focused buffer. An unfocused
    /// buffer never consumes a key, so a hidden editor cannot be edited.
    pub fn apply_key(&mut self, key: &Key) -> bool {
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
        let index = self.section.index();
        if let Some(dirty) = self.dirty.get_mut(index) {
            *dirty = true;
        }
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
    pub fn status_label(&self, language: &str) -> String {
        json_editor_status(
            self.focused,
            self.dirty
                .get(self.section.index())
                .copied()
                .unwrap_or(false),
            self.published_document().is_some(),
            self.status,
            language,
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

/// Scene constructor for the JSON partition of the rules workspace.
pub fn rules_json_scene(palette: &UiPalette, state: &RulesJsonState) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = RulesJsonSection::ALL
        .iter()
        .map(|section| section_chip_scene(*section, palette))
        .collect();
    let status = state.status_label("en-US");
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
                    { chips }
                ]
                --
                Text({ status }) RulesJsonStatusText TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    height: rem(320.0),
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
                    Children [
                        { vec![body] }
                    ]
                ]
            ]
    }
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
    let status = state.status_label(locale.code());
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
) {
    if let Some(state) = state.as_deref_mut() {
        state.adopt(&update.0.json_documents);
    }
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
    let index = state.section.index();
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
    if let Some(dirty) = state.dirty.get_mut(index) {
        *dirty = true;
    }
    state.status = Some(RuleJsonFeedback::AwaitingReadback);
}

/// The editor keyboard seam: only the focused buffer consumes keys, and only
/// while the partition body is mounted. Ctrl+S submits the active section.
pub fn rules_json_keyboard_input(
    mut keys: MessageReader<KeyboardInput>,
    keys_state: Option<Res<ButtonInput<KeyCode>>>,
    bodies: Query<(), With<RulesJsonEditorBody>>,
    tabs: Option<Res<RulesTabState>>,
    mut state: Option<ResMut<RulesJsonState>>,
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
    if tabs
        .as_ref()
        .is_some_and(|tabs| tabs.tab != RulesTab::JsonEditors)
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
                    let index = state.section.index();
                    if let Some(dirty) = state.dirty.get_mut(index) {
                        *dirty = true;
                    }
                    state.status = Some(RuleJsonFeedback::AwaitingReadback);
                }
            }
            continue;
        }
        if modifiers.ctrl || modifiers.alt || modifiers.meta {
            continue;
        }
        if state.apply_key(&key) {
            state.generation = state.generation.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_contract::profile_document_identity::{
        ProfileDocumentRevision, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind,
        ProfileFormatId, ProfileId, ProfileSchemaVersion, ProfileSourceId,
    };

    fn identity(revision: &str) -> ProfileDocumentIdentity {
        ProfileDocumentIdentity::new(
            ProfileId::new("profile-main").unwrap(),
            EngineId::new("mihomo").unwrap(),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new("local-main").unwrap(),
            ),
            ProfileDocumentRevision::new(revision).unwrap(),
            ProfileFormatId::new("mihomo-yaml").unwrap(),
            ProfileSchemaVersion::new(1).unwrap(),
        )
    }

    fn documents() -> Vec<RulesJsonDocumentSnapshot> {
        vec![
            RulesJsonDocumentSnapshot {
                section: RulesJsonSection::RuleProviders,
                identity: identity("revision-1"),
                json: "{\n  \"a\": 1\n}".to_owned(),
            },
            RulesJsonDocumentSnapshot {
                section: RulesJsonSection::Sniffer,
                identity: identity("revision-1"),
                json: "{}".to_owned(),
            },
        ]
    }

    #[test]
    fn every_shared_section_has_a_label_and_a_save_label() {
        for section in RulesJsonSection::ALL {
            assert!(!Lang("zh-CN").tr(section.i18n_key()).is_empty());
            assert!(!Lang("zh-CN").tr(section.save_i18n_key()).is_empty());
        }
        assert_eq!(
            Lang("zh-CN").tr(RulesJsonSection::Sniffer.i18n_key()),
            "嗅探器 JSON"
        );
        assert_eq!(
            Lang("zh-CN").tr(RulesJsonSection::RuleProviders.save_i18n_key()),
            "保存规则提供者"
        );
    }

    #[test]
    fn adoption_fills_empty_buffers_and_never_clobbers_a_dirty_one() {
        let mut state = RulesJsonState::default();
        state.adopt(&documents());
        assert_eq!(state.buffers.len(), RulesJsonSection::ALL.len());
        assert_eq!(
            state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
            "{\n  \"a\": 1\n}"
        );
        assert_eq!(
            state.buffers[RulesJsonSection::ProxyProviders.index()].full_text(),
            ""
        );

        // A dirty buffer keeps the user's bytes when the read model moves.
        state.section = RulesJsonSection::RuleProviders;
        state.dirty[RulesJsonSection::RuleProviders.index()] = true;
        state.buffer_mut().set_text("{\n  \"edited\": true\n}");
        state.adopt(&[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-2"),
            json: "{\n  \"server\": 1\n}".to_owned(),
        }]);
        assert_eq!(
            state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
            "{\n  \"edited\": true\n}"
        );
        assert_eq!(
            state.buffer_identities[RulesJsonSection::RuleProviders.index()],
            Some(identity("revision-1")),
            "dirty text keeps the revision it was read from"
        );
        assert!(state.status_label("zh-CN").contains("有未提交改动"));

        // A read-back that matches the buffer clears the pending flag.
        state.adopt(&[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-2"),
            json: "{\n  \"edited\": true\n}".to_owned(),
        }]);
        assert!(!state.dirty[RulesJsonSection::RuleProviders.index()]);
        assert_eq!(
            state.buffer_identities[RulesJsonSection::RuleProviders.index()],
            Some(identity("revision-2"))
        );
        assert!(!state.status_label("zh-CN").contains("有未提交改动"));

        // A clean buffer follows the read model.
        state.dirty[RulesJsonSection::RuleProviders.index()] = false;
        state.adopt(&[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            identity: identity("revision-3"),
            json: "{\n  \"server\": 2\n}".to_owned(),
        }]);
        assert_eq!(
            state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
            "{\n  \"server\": 2\n}"
        );
        assert!(state.status_label("zh-CN").contains("已与读模型一致"));
    }

    #[test]
    fn editing_marks_the_buffer_dirty_and_escape_releases_focus() {
        let mut state = RulesJsonState::default();
        state.adopt(&documents());
        state.section = RulesJsonSection::RuleProviders;
        state.focused = true;
        assert!(state.apply_key(&Key::End));
        assert!(state.apply_key(&Key::Character("!".into())));
        assert!(state.buffer().full_text().contains("!"));
        assert!(state.dirty[RulesJsonSection::RuleProviders.index()]);
        assert!(state.status_label("zh-CN").contains("编辑中"));
        assert!(state.apply_key(&Key::Escape));
        assert!(!state.focused);
        assert!(!state.apply_key(&Key::Character("x".into())));
    }

    #[test]
    fn submit_builds_a_revision_bound_typed_update_and_refuses_invalid_buffers() {
        let mut state = RulesJsonState::default();
        state.adopt(&documents());
        state.section = RulesJsonSection::Sniffer;
        match state.submit_command() {
            Ok(UiCommand::ApplyProfileConfiguration(request)) => {
                assert_eq!(request.identity, identity("revision-1"));
                assert_eq!(
                    request.update,
                    ProfileConfigurationUpdate::Sniffer(Default::default())
                );
            }
            other => panic!("expected a typed profile update, got {other:?}"),
        }

        state.section = RulesJsonSection::ProxyProviders;
        assert_eq!(
            state.submit_command(),
            Err(RulesJsonSubmissionFailure::Empty),
            "empty buffer is refused"
        );
        assert!(state.status_label("zh-CN").contains("读模型未发布该文档"));

        state.section = RulesJsonSection::RuleProviders;
        state.buffer_mut().set_text("[]");
        assert_eq!(
            state.submit_command(),
            Err(RulesJsonSubmissionFailure::InvalidDocument)
        );
    }
}
