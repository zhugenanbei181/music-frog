//! Select / Combobox: searchable dropdown selection with floating popover panel.
//!
//! **Pure State Machine Core**: [`ComboboxState`] owns the options list, filter string,
//! open/closed state, highlighted navigation index, and selection.
//! Keyboard actions ([`ComboboxAction`]: Open, Close, Toggle, Next, Previous, SetFilter,
//! SelectHighlighted, SelectId) advance the state machine deterministically, returning
//! [`ComboboxOutcome`] events. Zero-bevy and 100% headless-testable.
//!
//! **Scene Adapters**: [`combobox_scene`] renders the trigger button/field with token
//! chrome and chevron icon; [`combobox_dropdown_scene`] renders the anchored floating panel
//! with search filter and selectable items on token layers.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::Changed;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::popover::{ANCHOR_GAP_PX, AnchorHint, placement};
use crate::text::{Role, TextRole};
use crate::theme::{metrics, space};

/// One selectable option in a combobox / select dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComboboxOption {
    pub id: String,
    pub label: String,
    pub disabled: bool,
}

impl ComboboxOption {
    /// Construct a new active option.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Construct a disabled option.
    pub fn disabled(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: true,
        }
    }
}

/// Navigation and interaction actions for combobox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboboxAction {
    Open,
    Close,
    Toggle,
    Next,
    Previous,
    First,
    Last,
    SetFilter(String),
    SelectHighlighted,
    SelectId(String),
}

/// Outcome emitted when a combobox state transition occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboboxOutcome {
    Opened,
    Closed,
    FilterChanged(String),
    Selected { id: String, label: String },
}

/// Zero-bevy pure state machine for searchable select/combobox.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboboxState {
    options: Vec<ComboboxOption>,
    selected_id: Option<String>,
    filter: String,
    is_open: bool,
    highlighted_index: Option<usize>,
}

impl ComboboxState {
    /// Create a new combobox state with options.
    pub fn new(options: Vec<ComboboxOption>) -> Self {
        Self {
            options,
            selected_id: None,
            filter: String::new(),
            is_open: false,
            highlighted_index: None,
        }
    }

    /// Set initial selected id.
    pub fn with_selected(mut self, id: impl Into<String>) -> Self {
        self.selected_id = Some(id.into());
        self
    }

    /// Set initial filter string.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = filter.into();
        self
    }

    /// Current options list.
    pub fn options(&self) -> &[ComboboxOption] {
        &self.options
    }

    /// Currently selected option id.
    pub fn selected_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    /// Currently selected option label.
    pub fn selected_label(&self) -> Option<&str> {
        let sel_id = self.selected_id.as_deref()?;
        self.options
            .iter()
            .find(|opt| opt.id == sel_id)
            .map(|opt| opt.label.as_str())
    }

    /// Current search filter.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Whether dropdown popover is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Currently highlighted index within filtered options.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Get filtered options based on case-insensitive substring matching.
    pub fn filtered_options(&self) -> Vec<(usize, &ComboboxOption)> {
        let needle = self.filter.trim().to_lowercase();
        self.options
            .iter()
            .enumerate()
            .filter(|(_, opt)| {
                if needle.is_empty() {
                    true
                } else {
                    opt.label.to_lowercase().contains(&needle)
                }
            })
            .collect()
    }

    /// Apply an action and return any resulting outcome.
    pub fn apply_action(&mut self, action: ComboboxAction) -> Option<ComboboxOutcome> {
        match action {
            ComboboxAction::Open => {
                if !self.is_open {
                    self.is_open = true;
                    self.highlight_first_enabled();
                    Some(ComboboxOutcome::Opened)
                } else {
                    None
                }
            }
            ComboboxAction::Close => {
                if self.is_open {
                    self.is_open = false;
                    self.highlighted_index = None;
                    Some(ComboboxOutcome::Closed)
                } else {
                    None
                }
            }
            ComboboxAction::Toggle => {
                self.is_open = !self.is_open;
                if self.is_open {
                    self.highlight_first_enabled();
                    Some(ComboboxOutcome::Opened)
                } else {
                    self.highlighted_index = None;
                    Some(ComboboxOutcome::Closed)
                }
            }
            ComboboxAction::SetFilter(filter) => {
                self.filter = filter.clone();
                self.highlight_first_enabled();
                Some(ComboboxOutcome::FilterChanged(filter))
            }
            ComboboxAction::Next => {
                if !self.is_open {
                    self.is_open = true;
                    self.highlight_first_enabled();
                    return Some(ComboboxOutcome::Opened);
                }
                let filtered = self.filtered_options();
                if filtered.is_empty() {
                    self.highlighted_index = None;
                    return None;
                }
                let current = self.highlighted_index.unwrap_or(0);
                for i in 1..=filtered.len() {
                    let next_idx = (current + i) % filtered.len();
                    if !filtered[next_idx].1.disabled {
                        self.highlighted_index = Some(next_idx);
                        break;
                    }
                }
                None
            }
            ComboboxAction::Previous => {
                if !self.is_open {
                    self.is_open = true;
                    self.highlight_first_enabled();
                    return Some(ComboboxOutcome::Opened);
                }
                let filtered = self.filtered_options();
                if filtered.is_empty() {
                    self.highlighted_index = None;
                    return None;
                }
                let current = self.highlighted_index.unwrap_or(0);
                for i in 1..=filtered.len() {
                    let prev_idx = (current + filtered.len() - i) % filtered.len();
                    if !filtered[prev_idx].1.disabled {
                        self.highlighted_index = Some(prev_idx);
                        break;
                    }
                }
                None
            }
            ComboboxAction::First => {
                self.highlight_first_enabled();
                None
            }
            ComboboxAction::Last => {
                let filtered = self.filtered_options();
                for (idx, (_, opt)) in filtered.iter().enumerate().rev() {
                    if !opt.disabled {
                        self.highlighted_index = Some(idx);
                        break;
                    }
                }
                None
            }
            ComboboxAction::SelectHighlighted => {
                if !self.is_open {
                    return None;
                }
                let filtered = self.filtered_options();
                let high_idx = self.highlighted_index?;
                let (_, opt) = filtered.get(high_idx)?;
                if opt.disabled {
                    return None;
                }
                let id = opt.id.clone();
                let label = opt.label.clone();
                self.selected_id = Some(id.clone());
                self.is_open = false;
                self.highlighted_index = None;
                Some(ComboboxOutcome::Selected { id, label })
            }
            ComboboxAction::SelectId(id) => {
                if let Some(opt) = self.options.iter().find(|o| o.id == id)
                    && !opt.disabled
                {
                    let label = opt.label.clone();
                    self.selected_id = Some(id.clone());
                    self.is_open = false;
                    self.highlighted_index = None;
                    return Some(ComboboxOutcome::Selected { id, label });
                }
                None
            }
        }
    }

    fn highlight_first_enabled(&mut self) {
        let filtered = self.filtered_options();
        self.highlighted_index = filtered
            .iter()
            .enumerate()
            .find(|(_, (_, opt))| !opt.disabled)
            .map(|(idx, _)| idx);
    }
}

/// Message to dispatch a combobox action to a specific entity.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ComboboxNavEvent {
    pub combobox: Entity,
    pub action: ComboboxAction,
}

/// Message emitted on combobox outcome.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ComboboxOutcomeEvent {
    pub combobox: Entity,
    pub outcome: ComboboxOutcome,
}

/// Marker component for combobox root container.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboboxRoot;

/// Component storing combobox state on the root entity.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ComboboxStateComp(pub ComboboxState);

/// Marker on the combobox trigger button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComboboxTrigger;

/// Marker on the combobox label text displaying current selection or placeholder.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComboboxLabel;

/// Marker on the combobox dropdown floating panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComboboxDropdownPanel;

/// Marker on an individual option item in the dropdown list carrying its filtered index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComboboxOptionItem(pub usize);

/// Construct the closed/idle combobox trigger scene.
pub fn combobox_scene(
    placeholder: String,
    selected_label: Option<String>,
    is_open: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = if is_open {
        palette.accent
    } else {
        palette.border
    };
    let display_text = selected_label.unwrap_or(placeholder);

    bsn! {
        Node {
            width: percent(100),
            height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(metrics::HAIRLINE)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        Button
        ComboboxRoot
        ComboboxTrigger
        Children [
            ( Text(display_text) TextRole(Role::Body) ComboboxLabel ),
            ( { icon_tile_scene(IconId::ArrowDown, 16.0, palette) } ),
        ]
    }
}

/// Construct the floating dropdown popover panel for combobox options.
pub fn combobox_dropdown_scene(
    hint: AnchorHint,
    options: &[ComboboxOption],
    highlighted: Option<usize>,
    selected_id: Option<&str>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rect = placement(hint, ANCHOR_GAP_PX);
    let edge = palette.border;

    let items: Vec<Box<dyn Scene>> = options
        .iter()
        .enumerate()
        .map(|(idx, opt)| {
            let is_high = highlighted == Some(idx);
            let is_sel = selected_id == Some(&opt.id);
            let bg_color = if is_high {
                palette.hover_bg
            } else if is_sel {
                palette.accent_container
            } else {
                Color::NONE
            };
            let lbl = opt.label.clone();

            Box::new(bsn! {
                Node {
                    width: percent(100),
                    height: px(palette.control_height_px * 0.85),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.5)),
                }
                BackgroundColor({ bg_color })
                Button
                ComboboxOptionItem(idx)
                Children [
                    ( Text(lbl) TextRole(Role::Body) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(rect.x),
            top: px(rect.y),
            width: px(rect.w),
            max_height: px(rect.h),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
            padding: UiRect::all(Val::Px(space::S8)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ComboboxDropdownPanel
        Children [
            { items },
        ]
    }
}

/// System to advance combobox navigation and selection from events.
pub fn advance_combobox(
    mut events: MessageReader<ComboboxNavEvent>,
    mut combos: Query<(Entity, &mut ComboboxStateComp)>,
    mut commands: Commands,
) {
    for event in events.read() {
        if let Ok((entity, mut state_comp)) = combos.get_mut(event.combobox)
            && let Some(outcome) = state_comp.0.apply_action(event.action.clone())
        {
            commands.write_message(ComboboxOutcomeEvent {
                combobox: entity,
                outcome,
            });
        }
    }
}

/// System to sync combobox trigger label and outline from live state.
pub fn sync_combobox_visuals(
    palette: Res<UiPalette>,
    mut combos: Query<
        (&ComboboxStateComp, &Children, &mut BorderColor),
        Changed<ComboboxStateComp>,
    >,
    mut labels: Query<(&ComboboxLabel, &mut Text, &mut TextColor)>,
) {
    for (state_comp, children, mut border) in &mut combos {
        let edge = if state_comp.0.is_open() {
            palette.accent
        } else {
            palette.border
        };
        if border.top != edge {
            border.set_all(edge);
        }

        let display_text = state_comp.0.selected_label().unwrap_or("Select...");
        for child in children.iter() {
            if let Ok((_, mut text, mut color)) = labels.get_mut(*child) {
                if text.0 != display_text {
                    text.0 = display_text.to_owned();
                }
                let target_color = if state_comp.0.selected_id().is_some() {
                    palette.ink
                } else {
                    palette.ink_dim
                };
                if color.0 != target_color {
                    color.0 = target_color;
                }
            }
        }
    }
}
