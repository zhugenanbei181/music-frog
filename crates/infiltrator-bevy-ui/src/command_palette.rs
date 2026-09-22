//! Global Command Palette (`Ctrl+K` / search): pure state machine + BSN scene.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md):
//! - 100% `bsn!` scene composition for the modal scrim and floating search dialog;
//! - Pure state machine core ([`CommandPaletteState`]) testable headlessly;
//! - The rows are the shared `infiltrator_contract::command_catalogue` list —
//!   the same entries (and the same accelerators from the shared shortcut
//!   registry) the Iced palette renders. This module owns presentation and the
//!   selection cursor only; dispatch lives in [`crate::command_palette_shell`].

use bevy::a11y::AccessibilityNode;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::command_catalogue::{CommandCatalogue, CommandEntry};
use infiltrator_contract::shortcuts::ShortcutRegistry;

/// The palette's pure state: the shared catalogue plus the query/selection
/// cursor. Arrow navigation wraps; an empty query keeps every row.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub catalogue: CommandCatalogue,
    pub filtered_indices: Vec<usize>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteState {
    pub fn new() -> Self {
        Self::from_catalogue(CommandCatalogue::new())
    }

    pub fn from_catalogue(catalogue: CommandCatalogue) -> Self {
        let filtered_indices = (0..catalogue.len()).collect();
        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            catalogue,
            filtered_indices,
        }
    }

    /// Swap in a catalogue (the stored profile list changed) while keeping the
    /// open query and clamping the cursor into the new result set.
    pub fn set_catalogue(&mut self, catalogue: CommandCatalogue) {
        self.catalogue = catalogue;
        self.refilter();
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.refilter();
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.refilter();
        self.selected_index = 0;
    }

    /// Append one typed character (the palette's keyboard seam).
    pub fn push_query_char(&mut self, character: char) {
        self.query.push(character);
        self.refilter();
        self.selected_index = 0;
    }

    /// Remove the last character (Unicode-safe: the shared catalogue filter is
    /// substring based, and `pop` removes one scalar value).
    pub fn pop_query_char(&mut self) {
        self.query.pop();
        self.refilter();
        self.selected_index = 0;
    }

    /// Recompute the filtered index list: the shared substring rule plus the
    /// shared pinyin matcher over the bare-Chinese titles (the same matcher
    /// the Iced palette applies), so "ymjx" finds the DNS page on both ends.
    pub fn refilter(&mut self) {
        let query = self.query.trim();
        if query.is_empty() {
            self.filtered_indices = (0..self.catalogue.len()).collect();
        } else {
            self.filtered_indices = self
                .catalogue
                .entries()
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.matches(query)
                        || infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(
                            &entry.title_zh,
                            query,
                        )
                        || infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(&entry.id, query)
                })
                .map(|(index, _)| index)
                .collect();
        }
        if !self.filtered_indices.is_empty() && self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + self.filtered_indices.len() - 1)
                % self.filtered_indices.len();
        }
    }

    pub fn current_selected_action(&self) -> Option<&CommandEntry> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|index| self.catalogue.entry(*index))
    }

    /// The accelerator the shared registry currently binds to this entry's
    /// action, when the entry is a global-chord command.
    pub fn accelerator_for(&self, index: usize, registry: &ShortcutRegistry) -> Option<String> {
        let entry = self.catalogue.entry(index)?;
        let action = entry.target.shortcut_action()?;
        registry
            .get(action)
            .map(|binding| binding.chord.display_string(false))
    }
}

/// Event triggering opening of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenCommandPalette;

/// Event triggering closing of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseCommandPalette;

/// Event triggering toggle of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleCommandPalette;

/// Event executing the selected command palette action.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecuteSelectedPaletteAction;

/// Event requesting execution of one filtered row (row click).
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutePaletteEntry(pub usize);

/// Marker component on the command palette root entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteOverlayRoot;

/// Marker on an individual action row button: the display index within the
/// filtered list.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteRow(pub usize);

/// Accessibility node constructor for the Command Palette dialog: the shared
/// grammar row supplies role and label, so the Bevy tree and the Iced labels
/// stay one vocabulary (DUAL-15-10).
pub fn command_palette_semantic_node() -> AccessibilityNode {
    crate::a11y::semantic_node(ShellA11yNode::CommandPaletteDialog)
}

/// Accessibility node constructor for the palette's live query line: a
/// read-only `Text` row of the shared grammar, mounted on the real query node
/// so a screen reader hears what is being typed (DUAL-15-10).
pub fn command_palette_query_semantic_node() -> AccessibilityNode {
    crate::a11y::semantic_node(ShellA11yNode::CommandPaletteQuery)
}

/// Declarative scene for an individual catalogue row.
pub fn command_palette_item_scene(
    palette: &UiPalette,
    entry: &CommandEntry,
    hint: Option<String>,
    is_selected: bool,
    display_index: usize,
) -> impl Scene + use<> {
    let bg = if is_selected {
        palette.accent.with_alpha(0.18)
    } else {
        Color::NONE
    };
    let edge = if is_selected {
        palette.accent
    } else {
        Color::NONE
    };
    let title_text = entry.title_zh.clone();
    let category_text = category_label(entry);
    let hint_text = hint.unwrap_or_default();

    bsn! {
        Node {
            width: percent(100),
            height: px(40.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S4)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
        }
        BackgroundColor(bg)
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        Button
        CommandPaletteRow(display_index)
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Text({ category_text })
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ title_text })
                        TextRole(Role::Body)
                    ),
                ]
            ),
            (
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    (
                        Text({ hint_text })
                        TextRole(Role::Caption)
                    ),
                ]
            ),
        ]
    }
}

/// Bare-Chinese category label used by the Bevy surface: the shared contract
/// owns the vocabulary.
fn category_label(entry: &CommandEntry) -> String {
    entry.category.label_zh().to_owned()
}

/// Declarative BSN modal scene for the Command Palette.
pub fn command_palette_modal_scene(
    palette: &UiPalette,
    state: &CommandPaletteState,
    registry: &ShortcutRegistry,
) -> impl Scene + use<> {
    let semantic = command_palette_semantic_node();
    let query_semantic = command_palette_query_semantic_node();
    let query_display = if state.query.is_empty() {
        "输入关键词检索页面或快捷运维指令...".to_owned()
    } else {
        state.query.clone()
    };
    let edge = palette.border;

    let items_boxed: Vec<Box<dyn Scene>> = state
        .filtered_indices
        .iter()
        .take(8)
        .enumerate()
        .filter_map(|(display_index, index)| {
            let entry = state.catalogue.entry(*index)?;
            let hint = state.accelerator_for(*index, registry);
            let is_selected = display_index == state.selected_index;
            Some(Box::new(command_palette_item_scene(
                palette,
                entry,
                hint,
                is_selected,
                display_index,
            )) as Box<dyn Scene>)
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::top(Val::Px(80.0)),
        }
        BackgroundColor({ palette.scrim })
        CommandPaletteOverlayRoot
        template_value(semantic)
        Children [
            (
                Node {
                    width: px(560.0),
                    max_width: percent(92),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S16)),
                    row_gap: Val::Px(space::S12),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                Children [
                    // Search bar row
                    (
                        Node {
                            width: percent(100),
                            height: px(44.0),
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S8)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            column_gap: Val::Px(space::S8),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            (
                                Text({ query_display })
                                TextRole(Role::Body)
                                CommandPaletteQueryLabel
                                template_value(query_semantic)
                            ),
                        ]
                    ),
                    // Items list
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S4),
                        }
                        Children [
                            { items_boxed },
                        ]
                    ),
                    // Footer hint
                    (
                        Node {
                            width: percent(100),
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::top(Val::Px(space::S4)),
                        }
                        Children [
                            (
                                Text({ "↑↓ 导航 · Enter 执行 · Esc 关闭".to_owned() })
                                TextRole(Role::Caption)
                            ),
                            (
                                Text({ format!("{}/{} 项", state.filtered_indices.len(), state.catalogue.len()) })
                                TextRole(Role::Caption)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// Marker on the palette's query text node (so a remount latch can read it
/// back in tests).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteQueryLabel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_lifecycle_and_filtering() {
        let mut state = CommandPaletteState::new();
        assert!(!state.is_open);
        assert_eq!(state.filtered_indices.len(), state.catalogue.len());

        state.open();
        assert!(state.is_open);

        // Filter by 'dns' -> the shared catalogue keeps the DNS page and the
        // cache flush action.
        state.set_query("dns");
        assert_eq!(state.filtered_indices.len(), 2);
        let first = state.current_selected_action().unwrap();
        assert_eq!(first.id, "nav.dns");

        // Navigate next
        state.select_next();
        let second = state.current_selected_action().unwrap();
        assert_eq!(second.id, "action.flush_dns_cache");

        // Wraparound
        state.select_next();
        let wrap = state.current_selected_action().unwrap();
        assert_eq!(wrap.id, "nav.dns");
        state.select_prev();
        assert_eq!(
            state.current_selected_action().unwrap().id,
            "action.flush_dns_cache"
        );

        // Close
        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_command_palette_category_matching_and_typing() {
        let mut state = CommandPaletteState::new();
        state.open();

        // Categories match by their shared Chinese label.
        state.set_query("代理模式");
        assert_eq!(state.filtered_indices.len(), 3);

        // The keyboard seam types and erases characters.
        state.set_query("");
        for character in "doct".chars() {
            state.push_query_char(character);
        }
        assert_eq!(state.query, "doct");
        assert_eq!(state.current_selected_action().unwrap().id, "nav.doctor");
        state.pop_query_char();
        assert_eq!(state.query, "doc");
    }

    #[test]
    fn test_command_palette_pinyin_matches_the_same_rows_as_iced() {
        // The shared matcher is the same engine Iced applies: substring plus
        // the regional-keyword pinyin initials (香港 → xg). A stored profile
        // named 香港 exercises it through the live profile rows.
        let catalogue = CommandCatalogue::with_profiles(&[
            infiltrator_contract::command_catalogue::ProfileChoice::new("sub-1", "香港 IEPL 01"),
        ]);
        let mut state = CommandPaletteState::from_catalogue(catalogue);
        state.open();
        state.set_query("xg");
        assert_eq!(
            state
                .current_selected_action()
                .map(|entry| entry.id.as_str()),
            Some("profile.sub-1")
        );

        // A plain substring still matches the product rows.
        state.set_query("dns");
        assert_eq!(
            state
                .current_selected_action()
                .map(|entry| entry.id.as_str()),
            Some("nav.dns")
        );
    }

    #[test]
    fn test_palette_renders_the_registry_accelerator() {
        let registry = ShortcutRegistry::with_defaults();
        let mut state = CommandPaletteState::new();
        state.open();
        let index = state
            .catalogue
            .index_of("action.toggle_mini_hud")
            .expect("mini hud row");
        assert_eq!(
            state.accelerator_for(index, &registry).as_deref(),
            Some("Ctrl+Alt+M")
        );
        let nav_index = state.catalogue.index_of("nav.dns").expect("nav row");
        assert_eq!(state.accelerator_for(nav_index, &registry), None);
    }
}
