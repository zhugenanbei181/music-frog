//! Collapse / Accordion: expandable sections and accordion container.
//!
//! **Pure Core**: [`AccordionState`], [`AccordionItemState`], and [`AccordionMode`]
//! manage multi-section expansion (Single or Multiple mode) and state toggling.
//! Zero-bevy and 100% headless-testable.
//!
//! **Scene Adapters**: [`collapse_scene`] and [`accordion_scene`] build declarative
//! card sections with animated disclosure chevrons and token surface chrome.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::Changed;
use bevy::ecs::system::Query;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Expansion behavior mode for an accordion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccordionMode {
    /// Only one item can be open at a time; expanding one collapses others.
    #[default]
    Single,
    /// Any number of items can be open simultaneously.
    Multiple,
}

/// State of one item in an accordion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccordionItemState {
    pub id: usize,
    pub title: String,
    pub is_expanded: bool,
    pub disabled: bool,
}

impl AccordionItemState {
    pub fn new(id: usize, title: impl Into<String>, is_expanded: bool) -> Self {
        Self {
            id,
            title: title.into(),
            is_expanded,
            disabled: false,
        }
    }
}

/// Pure state of an accordion container.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccordionState {
    pub items: Vec<AccordionItemState>,
    pub mode: AccordionMode,
}

impl AccordionState {
    /// Create a new accordion state.
    pub fn new(items: Vec<(String, bool)>, mode: AccordionMode) -> Self {
        let item_states = items
            .into_iter()
            .enumerate()
            .map(|(idx, (title, exp))| AccordionItemState::new(idx, title, exp))
            .collect();
        Self {
            items: item_states,
            mode,
        }
    }

    /// Whether a specific item index is expanded.
    pub fn is_expanded(&self, index: usize) -> bool {
        self.items
            .get(index)
            .map(|it| it.is_expanded)
            .unwrap_or(false)
    }

    /// Toggle expansion state of an item by index. Returns true if state changed.
    pub fn toggle(&mut self, index: usize) -> bool {
        let Some(target) = self.items.get_mut(index) else {
            return false;
        };
        if target.disabled {
            return false;
        }

        let next_exp = !target.is_expanded;
        if self.mode == AccordionMode::Single && next_exp {
            for (idx, item) in self.items.iter_mut().enumerate() {
                item.is_expanded = idx == index;
            }
        } else {
            target.is_expanded = next_exp;
        }
        true
    }

    /// Expand all non-disabled accordion items (promotes mode to Multiple).
    pub fn expand_all(&mut self) {
        self.mode = AccordionMode::Multiple;
        for item in &mut self.items {
            if !item.disabled {
                item.is_expanded = true;
            }
        }
    }

    /// Collapse all accordion items.
    pub fn collapse_all(&mut self) {
        for item in &mut self.items {
            item.is_expanded = false;
        }
    }

    /// Filter items by search query and auto-expand matching sections. Returns matched count.
    pub fn filter_and_expand(&mut self, query: &str) -> usize {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return 0;
        }
        self.mode = AccordionMode::Multiple;
        let mut matched = 0;
        for item in &mut self.items {
            if item.title.to_lowercase().contains(&q) && !item.disabled {
                item.is_expanded = true;
                matched += 1;
            } else {
                item.is_expanded = false;
            }
        }
        matched
    }
}

/// Component wrapper for accordion state.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct AccordionStateComp(pub AccordionState);

/// Marker component on accordion container root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccordionRoot;

/// Marker component on an accordion header button carrying its item index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccordionHeader(pub usize);

/// Marker component on an accordion content body container carrying its item index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccordionContent(pub usize);

/// Message requesting to toggle an accordion section.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccordionToggleEvent {
    pub accordion: Entity,
    pub index: usize,
}

/// Construct a single collapsible section scene.
pub fn collapse_scene(
    title: String,
    is_expanded: bool,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let body_display = if is_expanded {
        Display::Flex
    } else {
        Display::None
    };

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        AccordionRoot
        AccordionStateComp(AccordionState::new(vec![(title.clone(), is_expanded)], AccordionMode::Multiple))
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(palette.control_height_px),
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Button
                AccordionHeader(0)
                Children [
                    ( Text(title) TextRole(Role::BodyStrong) ),
                    ( { icon_tile_scene(IconId::ArrowDown, 16.0, palette) } ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    padding: UiRect::all(Val::Px(space::S12)),
                    display: body_display,
                }
                AccordionContent(0)
                Children [
                    ( { content } ),
                ]
            ),
        ]
    }
}

/// Construct a multi-item accordion scene.
pub fn accordion_scene(
    items: Vec<(String, Box<dyn Scene>, bool)>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let state_seed: Vec<(String, bool)> = items
        .iter()
        .map(|(title, _, exp)| (title.clone(), *exp))
        .collect();

    let item_nodes: Vec<Box<dyn Scene>> = items
        .into_iter()
        .enumerate()
        .map(|(idx, (title, content, is_expanded))| {
            let body_display = if is_expanded {
                Display::Flex
            } else {
                Display::None
            };

            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::bottom(Val::Px(palette.hairline_px)),
                }
                BorderColor {
                    top: Color::NONE,
                    right: Color::NONE,
                    bottom: edge,
                    left: Color::NONE,
                }
                Children [
                    (
                        Node {
                            width: percent(100),
                            height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Button
                        AccordionHeader(idx)
                        Children [
                            ( Text(title) TextRole(Role::BodyStrong) ),
                            ( { icon_tile_scene(IconId::ArrowDown, 16.0, palette) } ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            padding: UiRect::all(Val::Px(space::S12)),
                            display: body_display,
                        }
                        AccordionContent(idx)
                        Children [
                            ( { content } ),
                        ]
                    ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        AccordionRoot
        AccordionStateComp(AccordionState::new(state_seed, AccordionMode::Single))
        Children [
            { item_nodes },
        ]
    }
}

/// Advance accordion state machine on toggle events.
pub fn advance_accordions(
    mut events: MessageReader<AccordionToggleEvent>,
    mut accordions: Query<&mut AccordionStateComp>,
) {
    for event in events.read() {
        if let Ok(mut state) = accordions.get_mut(event.accordion) {
            state.0.toggle(event.index);
        }
    }
}

/// Repaint accordion expansion bodies when [`AccordionStateComp`] changes.
#[allow(clippy::type_complexity)]
pub fn sync_accordion_visuals(
    accordions: Query<(Entity, &AccordionStateComp), Changed<AccordionStateComp>>,
    groups: Query<&Children>,
    mut contents: Query<(&AccordionContent, &mut Node)>,
) {
    for (entity, state) in &accordions {
        if !groups.contains(entity) {
            continue;
        }
        for descendant in groups.iter_descendants(entity) {
            if let Ok((content, mut node)) = contents.get_mut(descendant) {
                let is_exp = state.0.is_expanded(content.0);
                let target_disp = if is_exp { Display::Flex } else { Display::None };
                if node.display != target_disp {
                    node.display = target_disp;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accordion_batch_controls_and_filtering() {
        let mut accordion = AccordionState::new(
            vec![
                ("Proxy Groups".to_string(), false),
                ("Routing Rules".to_string(), false),
                ("DNS Settings".to_string(), false),
            ],
            AccordionMode::Single,
        );

        // Expand all
        accordion.expand_all();
        assert_eq!(accordion.mode, AccordionMode::Multiple);
        assert!(accordion.is_expanded(0));
        assert!(accordion.is_expanded(1));
        assert!(accordion.is_expanded(2));

        // Collapse all
        accordion.collapse_all();
        assert!(!accordion.is_expanded(0));
        assert!(!accordion.is_expanded(1));

        // Filter and expand by query 'dns'
        let matched = accordion.filter_and_expand("dns");
        assert_eq!(matched, 1);
        assert!(!accordion.is_expanded(0));
        assert!(accordion.is_expanded(2));
    }
}
