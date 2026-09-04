//! Tabs & SegmentedControl: sliding pill capsule navigation with token-driven styling.
//!
//! **Pure Geometry & State Core**: [`SegmentedControlState`], [`capsule_metrics`], and
//! [`capsule_px_bounds`] compute the exact percentage/pixel travel and width of the
//! sliding capsule highlight without any layout measurement or framework dependency.
//!
//! **Scene Adapters**: [`segmented_control_scene`] and [`tabs_scene`] construct declarative
//! token-spaced pill containers over official buttons with high-contrast label switching.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Changed, With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Pure state of a segmented control or tabs bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedControlState {
    pub selected_index: usize,
    pub count: usize,
}

impl SegmentedControlState {
    pub fn new(count: usize, selected_index: usize) -> Self {
        Self {
            count,
            selected_index: selected_index.min(count.saturating_sub(1)),
        }
    }

    /// Select index with clamping. Returns true if changed.
    pub fn select(&mut self, index: usize) -> bool {
        let clamped = index.min(self.count.saturating_sub(1));
        if self.selected_index != clamped {
            self.selected_index = clamped;
            true
        } else {
            false
        }
    }
}

/// Compute percentage metrics `(left_percent, width_percent)` for the sliding capsule indicator.
/// Pure function — headless-testable.
pub fn capsule_metrics(selected_index: usize, count: usize) -> (f32, f32) {
    if count == 0 {
        return (0.0, 100.0);
    }
    let width_pct = 100.0 / count as f32;
    let idx = selected_index.min(count - 1);
    let left_pct = idx as f32 * width_pct;
    (left_pct, width_pct)
}

/// Compute pixel bounds `(left_px, width_px)` for the sliding capsule indicator within a fixed box.
/// Pure function.
pub fn capsule_px_bounds(
    selected_index: usize,
    count: usize,
    total_width_px: f32,
    padding_px: f32,
) -> (f32, f32) {
    if count == 0 {
        return (padding_px, (total_width_px - padding_px * 2.0).max(0.0));
    }
    let usable_w = (total_width_px - padding_px * 2.0).max(0.0);
    let tab_w = usable_w / count as f32;
    let idx = selected_index.min(count - 1);
    let left_px = padding_px + idx as f32 * tab_w;
    (left_px, tab_w)
}

/// Marker component on a segmented control root container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedControl;

/// Component storing active selected index on segmented control container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedControlValue(pub usize);

/// Component storing total tabs count on segmented control container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedControlCount(pub usize);

/// Marker on the sliding capsule highlight indicator node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedPillIndicator;

/// Component on each tab item carrying its index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedTab(pub usize);

/// Marker on tab text label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedTabLabel;

/// Message requesting selection change on a segmented control.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabSelectEvent {
    pub container: Entity,
    pub selected_index: usize,
}

/// Segmented control scene with sliding capsule highlight.
pub fn segmented_control_scene(
    tabs: Vec<String>,
    selected_index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let count = tabs.len();
    let (left_pct, width_pct) = capsule_metrics(selected_index, count);
    let edge = palette.border;

    let tab_scenes: Vec<Box<dyn Scene>> = tabs
        .into_iter()
        .enumerate()
        .map(|(idx, title)| {
            let is_selected = idx == selected_index;
            let ink = if is_selected {
                palette.on_accent
            } else {
                palette.ink_dim
            };

            Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    height: px(palette.control_height_px * 0.8),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.75)),
                }
                Button
                SegmentedTab(idx)
                Children [
                    (
                        Text(title)
                        TextRole(Role::Caption)
                        TextColor({ ink })
                        SegmentedTabLabel
                    ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Relative,
            width: percent(100),
            height: px(palette.control_height_px * 0.9),
            padding: UiRect::all(Val::Px(space::S4)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        SegmentedControl
        SegmentedControlValue(selected_index)
        SegmentedControlCount(count)
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    width: Val::Percent(width_pct),
                    height: px(palette.control_height_px * 0.8),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.75)),
                }
                BackgroundColor({ palette.accent })
                SegmentedPillIndicator
            ),
            (
                Node {
                    position_type: PositionType::Relative,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                }
                Children [
                    { tab_scenes },
                ]
            ),
        ]
    }
}

/// Tabs bar scene with standard body size tabs.
pub fn tabs_scene(
    tabs: Vec<String>,
    selected_index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let count = tabs.len();
    let (left_pct, width_pct) = capsule_metrics(selected_index, count);
    let edge = palette.border;

    let tab_scenes: Vec<Box<dyn Scene>> = tabs
        .into_iter()
        .enumerate()
        .map(|(idx, title)| {
            let is_selected = idx == selected_index;
            let ink = if is_selected {
                palette.on_accent
            } else {
                palette.ink_dim
            };

            Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    height: px(palette.control_height_px),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                Button
                SegmentedTab(idx)
                Children [
                    (
                        Text(title)
                        TextRole(Role::Body)
                        TextColor({ ink })
                        SegmentedTabLabel
                    ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Relative,
            width: percent(100),
            height: px(palette.control_height_px + space::S8),
            padding: UiRect::all(Val::Px(space::S4)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        SegmentedControl
        SegmentedControlValue(selected_index)
        SegmentedControlCount(count)
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    width: Val::Percent(width_pct),
                    height: px(palette.control_height_px),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.accent })
                SegmentedPillIndicator
            ),
            (
                Node {
                    position_type: PositionType::Relative,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                }
                Children [
                    { tab_scenes },
                ]
            ),
        ]
    }
}

/// Advance segmented control state machine from select events.
pub fn advance_segmented_control(
    mut events: MessageReader<TabSelectEvent>,
    mut controls: Query<&mut SegmentedControlValue>,
) {
    for event in events.read() {
        if let Ok(mut val) = controls.get_mut(event.container)
            && val.0 != event.selected_index
        {
            val.0 = event.selected_index;
        }
    }
}

/// Repaint capsule position and label inks whenever [`SegmentedControlValue`] changes.
#[allow(clippy::type_complexity)]
pub fn sync_segmented_control_visuals(
    palette: Res<UiPalette>,
    controls: Query<
        (
            Entity,
            &SegmentedControlValue,
            &SegmentedControlCount,
            &Children,
        ),
        Changed<SegmentedControlValue>,
    >,
    groups: Query<&Children>,
    mut indicators: Query<&mut Node, With<SegmentedPillIndicator>>,
    tabs: Query<(&SegmentedTab, &Children), Without<SegmentedPillIndicator>>,
    mut labels: Query<(&SegmentedTabLabel, &mut TextColor)>,
) {
    for (entity, val, count_comp, children) in &controls {
        let (left_pct, width_pct) = capsule_metrics(val.0, count_comp.0);

        for child in children.iter() {
            if let Ok(mut node) = indicators.get_mut(*child) {
                node.left = Val::Percent(left_pct);
                node.width = Val::Percent(width_pct);
            }
        }

        if !groups.contains(entity) {
            continue;
        }

        for descendant in groups.iter_descendants(entity) {
            if let Ok((tab, tab_children)) = tabs.get(descendant) {
                let is_selected = tab.0 == val.0;
                let target_ink = if is_selected {
                    palette.on_accent
                } else {
                    palette.ink_dim
                };

                for tab_child in tab_children.iter() {
                    if let Ok((_, mut ink)) = labels.get_mut(*tab_child)
                        && ink.0 != target_ink
                    {
                        ink.0 = target_ink;
                    }
                }
            }
        }
    }
}
