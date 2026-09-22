//! DUAL-11-14: the Bevy rules-workspace JSON partition.
//!
//! Iced edits three JSON documents of the active profile (rule-providers,
//! proxy-providers, sniffer) through `text_editor` buffers and the shared
//! `ConfigurationApplication::save_*` use-cases. This module gives the Bevy
//! surface the same capability: the documents arrive through the shared read
//! model (`RulesPageSnapshot.json_documents`, serialised from the same profile
//! the Iced editor loads), the buffer is the shared-widget
//! [`CodeEditorState`], and saving submits
//! [`UiCommand::ApplyRulesJsonDocument`], which the shared application parses,
//! validates and persists.
//!
//! **Honest boundary**: the Bevy command bus is fire-and-forget, so the status
//! line reports that a save was *submitted* and reports 「已与读模型一致」 only
//! once the read model publishes the submitted text back.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::editor::{CodeEditorState, code_editor_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::rules::RulesProjectionUpdated;

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
    /// Honest status line (submitted / adopted / refused).
    pub status: Option<String>,
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
        self.generation = self.generation.wrapping_add(1);
    }

    /// Adopt the published documents. A buffer the user edited is never
    /// clobbered: it stays dirty until it is saved or the user switches away.
    pub fn adopt(&mut self, documents: &[RulesJsonDocumentSnapshot]) {
        self.published = documents.to_vec();
        self.ensure_buffers();
        for document in documents {
            let index = document.section.index();
            if self.buffers[index].full_text() == document.json {
                // The read model confirms the buffer bytes: a submitted edit is
                // no longer pending, whatever the local flag said.
                if let Some(dirty) = self.dirty.get_mut(index) {
                    *dirty = false;
                }
                continue;
            }
            if self.dirty.get(index).copied().unwrap_or(false) {
                continue;
            }
            self.buffers[index] = CodeEditorState::new(&document.json);
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
    pub fn submit_command(&self) -> Option<UiCommand> {
        let json = self.buffer().full_text();
        if json.trim().is_empty() {
            return None;
        }
        Some(UiCommand::ApplyRulesJsonDocument {
            section: self.section,
            json,
        })
    }

    /// Status text for the active section.
    pub fn status_label(&self) -> String {
        let index = self.section.index();
        let focus = if self.focused {
            "编辑中"
        } else {
            "未聚焦"
        };
        let saved = if self.dirty.get(index).copied().unwrap_or(false) {
            "有未提交改动"
        } else if self.published_document().is_some() {
            "已与读模型一致"
        } else {
            "读模型未发布该文档"
        };
        match self.status.as_deref() {
            Some(status) => format!("{focus} · {saved} · {status}"),
            None => format!("{focus} · {saved}"),
        }
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

/// Bare-Chinese label of a shared JSON section (Bevy presentation convention).
pub const fn json_section_label_zh(section: RulesJsonSection) -> &'static str {
    match section {
        RulesJsonSection::RuleProviders => "规则提供者 JSON",
        RulesJsonSection::ProxyProviders => "代理提供者 JSON",
        RulesJsonSection::Sniffer => "嗅探器 JSON",
    }
}

/// Bare-Chinese save label of a shared JSON section.
pub const fn json_section_save_label_zh(section: RulesJsonSection) -> &'static str {
    match section {
        RulesJsonSection::RuleProviders => "保存规则提供者",
        RulesJsonSection::ProxyProviders => "保存代理提供者",
        RulesJsonSection::Sniffer => "保存嗅探器",
    }
}

fn section_chip_scene(section: RulesJsonSection, palette: &UiPalette) -> Box<dyn Scene> {
    let index = section.index();
    let selected = index == 0;
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px * 0.85),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ if selected { palette.accent } else { palette.surface_elevated } })
        Button
        RulesJsonSectionChip(index)
        Children [
            (
                Text({ json_section_label_zh(section).to_owned() })
                TextRole(Role::Caption)
                TextColor({ if selected { palette.on_accent } else { palette.ink_dim } })
                RulesJsonSectionLabel(index)
            ),
        ]
    })
}

fn edit_button_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.border })
        Button
        RulesJsonEditButton
        Children [
            ( Text({ "编辑缓冲区".to_owned() }) TextRole(Role::Body) ),
        ]
    })
}

fn save_button_scene(section: RulesJsonSection, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.accent })
        Button
        RulesJsonSaveButton
        Children [
            (
                Text({ json_section_save_label_zh(section).to_owned() })
                TextRole(Role::BodyStrong)
                RulesJsonSaveLabel
            ),
        ]
    })
}

/// Scene constructor for the JSON partition of the rules workspace.
pub fn rules_json_scene(palette: &UiPalette, state: &RulesJsonState) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = RulesJsonSection::ALL
        .iter()
        .map(|section| section_chip_scene(*section, palette))
        .collect();
    let status = state.status_label();
    let body: Box<dyn Scene> = code_editor_scene(state.buffer(), palette);
    let edit_button = edit_button_scene(palette);
    let save_button = save_button_scene(state.section, palette);

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S12),
        }
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "规则工作区 JSON 编辑器 (Rule Workspace JSON)".to_owned() }) TextRole(Role::BodyStrong) ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            { vec![edit_button, save_button] },
                        ]
                    ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    { chips },
                ]
            ),
            ( Text({ status }) RulesJsonStatusText TextRole(Role::Caption) ),
            (
                Node {
                    width: percent(100),
                    height: px(320.0),
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
                        RulesJsonEditorBody
                        Children [
                            { vec![body] },
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// DUAL-11-14: restamp the partition chrome (status line, save label, section
/// chip fills) from the state. The editor body is rebuilt separately by
/// [`refresh_rules_json_body`]; everything else is compare-and-set in place.
pub fn restamp_rules_json(
    state: Option<Res<RulesJsonState>>,
    palette: Res<UiPalette>,
    mut statuses: Query<&mut Text, (With<RulesJsonStatusText>, Without<RulesJsonSaveLabel>)>,
    mut save_labels: Query<&mut Text, (With<RulesJsonSaveLabel>, Without<RulesJsonStatusText>)>,
    mut chips: Query<(&mut BackgroundColor, &RulesJsonSectionChip)>,
    mut section_labels: Query<(&mut TextColor, &RulesJsonSectionLabel)>,
) {
    let Some(state) = state else {
        return;
    };
    let status = state.status_label();
    for mut text in &mut statuses {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
    let save = json_section_save_label_zh(state.section).to_owned();
    for mut text in &mut save_labels {
        if text.0 != save {
            text.0 = save.clone();
        }
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
        state.status = Some("此宿主未组合命令通道，未提交".to_owned());
        return;
    };
    let Some(command) = state.submit_command() else {
        state.status = Some("JSON 内容为空，未提交".to_owned());
        return;
    };
    handle.submit(command);
    if let Some(dirty) = state.dirty.get_mut(index) {
        *dirty = true;
    }
    state.status = Some("已提交共享应用保存（等待读模型回读）".to_owned());
}

/// The editor keyboard seam: only the focused buffer consumes keys, and only
/// while the partition body is mounted. Ctrl+S submits the active section.
pub fn rules_json_keyboard_input(
    mut keys: bevy::ecs::message::MessageReader<KeyboardInput>,
    keys_state: Option<Res<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>>,
    bodies: Query<(), With<RulesJsonEditorBody>>,
    tabs: Option<Res<crate::pages::rules_tabs::RulesTabState>>,
    mut state: Option<ResMut<RulesJsonState>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let pressed: Vec<Key> = keys
        .read()
        .filter(|key| key.state == bevy::input::ButtonState::Pressed)
        .map(|key| key.logical_key.clone())
        .collect();
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if !state.focused || bodies.is_empty() {
        return;
    }
    // A hidden partition never owns the keyboard, even if focus was left on.
    if tabs.as_ref().is_some_and(|tabs| {
        tabs.tab != infiltrator_contract::rules_workspace::RulesTab::JsonEditors
    }) {
        return;
    }
    let modifiers = keys_state
        .as_deref()
        .map(crate::shortcuts::modifiers_from_keyboard)
        .unwrap_or_default();
    for key in pressed {
        if modifiers.ctrl && matches!(&key, Key::Character(text) if text.eq_ignore_ascii_case("s"))
        {
            if let (Some(handle), Some(command)) = (handle.as_ref(), state.submit_command()) {
                handle.submit(command);
                let index = state.section.index();
                if let Some(dirty) = state.dirty.get_mut(index) {
                    *dirty = true;
                }
                state.status = Some("已提交共享应用保存（等待读模型回读）".to_owned());
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

    fn documents() -> Vec<RulesJsonDocumentSnapshot> {
        vec![
            RulesJsonDocumentSnapshot {
                section: RulesJsonSection::RuleProviders,
                json: "{\n  \"a\": 1\n}".to_owned(),
            },
            RulesJsonDocumentSnapshot {
                section: RulesJsonSection::Sniffer,
                json: "{}".to_owned(),
            },
        ]
    }

    #[test]
    fn every_shared_section_has_a_label_and_a_save_label() {
        for section in RulesJsonSection::ALL {
            assert!(!json_section_label_zh(section).is_empty());
            assert!(!json_section_save_label_zh(section).is_empty());
        }
        assert_eq!(
            json_section_label_zh(RulesJsonSection::Sniffer),
            "嗅探器 JSON"
        );
        assert_eq!(
            json_section_save_label_zh(RulesJsonSection::RuleProviders),
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
            json: "{\n  \"server\": 1\n}".to_owned(),
        }]);
        assert_eq!(
            state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
            "{\n  \"edited\": true\n}"
        );
        assert!(state.status_label().contains("有未提交改动"));

        // A read-back that matches the buffer clears the pending flag.
        state.adopt(&[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            json: "{\n  \"edited\": true\n}".to_owned(),
        }]);
        assert!(!state.dirty[RulesJsonSection::RuleProviders.index()]);
        assert!(!state.status_label().contains("有未提交改动"));

        // A clean buffer follows the read model.
        state.dirty[RulesJsonSection::RuleProviders.index()] = false;
        state.adopt(&[RulesJsonDocumentSnapshot {
            section: RulesJsonSection::RuleProviders,
            json: "{\n  \"server\": 2\n}".to_owned(),
        }]);
        assert_eq!(
            state.buffers[RulesJsonSection::RuleProviders.index()].full_text(),
            "{\n  \"server\": 2\n}"
        );
        assert!(state.status_label().contains("已与读模型一致"));
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
        assert!(state.status_label().contains("编辑中"));
        assert!(state.apply_key(&Key::Escape));
        assert!(!state.focused);
        assert!(!state.apply_key(&Key::Character("x".into())));
    }

    #[test]
    fn submit_builds_the_shared_section_intent_and_refuses_empty_buffers() {
        let mut state = RulesJsonState::default();
        state.adopt(&documents());
        state.section = RulesJsonSection::Sniffer;
        match state.submit_command() {
            Some(UiCommand::ApplyRulesJsonDocument { section, json }) => {
                assert_eq!(section, RulesJsonSection::Sniffer);
                assert_eq!(json, "{}");
            }
            other => panic!("expected the shared JSON intent, got {other:?}"),
        }

        state.section = RulesJsonSection::ProxyProviders;
        assert!(state.submit_command().is_none(), "empty buffer is refused");
        assert!(state.status_label().contains("读模型未发布该文档"));
    }
}
