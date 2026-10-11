//! Tabs & SegmentedControl: sliding pill capsule visuals over the OFFICIAL
//! headless tab strip (`bevy_ui_widgets::TabList` / `Tab` / `SelectedTab`).
//!
//! Selection, keyboard navigation (arrows / Home / End / Enter / Space),
//! roving tabindex and the TabList/Tab accessibility roles are owned by the
//! official widgets (capability norm ③); this module adds only the sliding
//! capsule geometry ([`capsule_metrics`], [`capsule_px_bounds`]) and the
//! token-styled scene adapters ([`segmented_control_scene`],
//! [`segmented_control_with_labels_scene`], [`tabs_scene`]). The tabs are
//! direct children of the list, as the official selection semantics require.

use crate::feathers_theme::Token;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Added, Changed, Or, With, Without};
use bevy::ecs::system::{Commands, Query};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, JustifyContent, Node, PositionType, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{ControlOrientation, SelectedTab, Tab, TabActivation, TabList};

/// Pure geometry: percentage metrics `(left_percent, width_percent)` for the
/// sliding capsule indicator. Pure function — headless-testable.
pub fn capsule_metrics(selected_index: usize, count: usize) -> (f32, f32) {
    if count == 0 {
        return (0.0, 100.0);
    }
    let width_pct = 100.0 / count as f32;
    let idx = selected_index.min(count - 1);
    let left_pct = idx as f32 * width_pct;
    (left_pct, width_pct)
}

/// Pure geometry: pixel bounds `(left_px, width_px)` for the sliding capsule
/// indicator within a fixed box. Pure function.
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

/// Marker component on a segmented control root (the official [`TabList`]
/// entity).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedControl;

/// The index a freshly mounted strip selects before the user interacts —
/// the declarative seed for [`SelectedTab`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedSelectionSeed(pub usize);

/// Component on each tab item carrying its index (the entity→index bridge
/// for consumers).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedTab(pub usize);

/// Marker on tab text label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedTabLabel;

/// Marker on the sliding capsule highlight indicator node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SegmentedPillIndicator;

/// Query used to find segmented controls whose selected tab or declarative
/// selection seed needs to update the capsule and label visuals.
pub type SegmentedControlVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static SelectedTab>,
        &'static SegmentedSelectionSeed,
        &'static Children,
    ),
    Or<(
        Changed<SelectedTab>,
        Added<SegmentedSelectionSeed>,
        Without<SelectedTab>,
    )>,
>;

/// Segmented control scene with sliding capsule highlight.
pub fn segmented_control_scene(
    tabs: Vec<String>,
    selected_index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let labels = tabs
        .into_iter()
        .map(|title| {
            Box::new(bsn! {
                Text(title) TextRole(Role::Caption) SegmentedTabLabel
            }) as Box<dyn Scene>
        })
        .collect();
    segmented_control_with_labels_scene(labels, selected_index, palette)
}

/// Keep each caller's native label scene and its localization identity intact.
pub fn segmented_control_with_labels_scene(
    tabs: Vec<Box<dyn Scene>>,
    selected_index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let count = tabs.len();
    let (left_pct, width_pct) = capsule_metrics(selected_index, count);
    let edge = palette.border;

    let tab_scenes: Vec<Box<dyn Scene>> = tabs
        .into_iter()
        .enumerate()
        .map(|(idx, label)| {
            Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    height: rem(palette.control_height_px * 0.8),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(rem(space::S8)),
                    border_radius: BorderRadius::MAX,
                }
                Tab
                SegmentedTab(idx)
                Children [
                    @{ label }
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
            Node {
                position_type: PositionType::Relative,
                width: percent(100),
                height: rem(palette.control_height_px * 0.9),
                padding: UiRect::all(rem(space::S4)),
                align_items: AlignItems::Center,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            TabList {
                orientation: ControlOrientation::Horizontal,
                activation: TabActivation::Automatic,
            }
            SegmentedControl
            SegmentedSelectionSeed(selected_index)
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    width: Val::Percent(width_pct),
                    height: rem(palette.control_height_px * 0.8),
                    border_radius: BorderRadius::MAX,
                }
                ThemeBackgroundColor({ Token::Accent })
                SegmentedPillIndicator
                --
                { tab_scenes }
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
            Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    height: rem(palette.control_height_px),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(rem(space::S12)),
                    border_radius: BorderRadius::MAX,
                }
                Tab
                SegmentedTab(idx)
                Children [
                    Text(title)
                    TextRole(Role::Body)
                    SegmentedTabLabel
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
            Node {
                position_type: PositionType::Relative,
                width: percent(100),
                height: rem(palette.control_height_px + space::S8),
                padding: UiRect::all(rem(space::S4)),
                align_items: AlignItems::Center,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            TabList {
                orientation: ControlOrientation::Horizontal,
                activation: TabActivation::Automatic,
            }
            SegmentedControl
            SegmentedSelectionSeed(selected_index)
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left_pct),
                    width: Val::Percent(width_pct),
                    height: rem(palette.control_height_px),
                    border_radius: BorderRadius::MAX,
                }
                ThemeBackgroundColor({ Token::Accent })
                SegmentedPillIndicator
                --
                { tab_scenes }
            ]
    }
}

/// Seed a freshly mounted strip's [`SelectedTab`] from its declarative seed,
/// then drive the capsule geometry and label inks from the official
/// selection state. Ink rides the official `ThemeTextColor` token swap; the
/// official derived-state system owns the `Selected` markers.
pub fn sync_segmented_control_visuals(
    mut commands: Commands,
    controls: SegmentedControlVisualQuery<'_, '_>,
    tabs: Query<(&SegmentedTab, &Children), With<Tab>>,
    mut indicators: Query<&mut Node, With<SegmentedPillIndicator>>,
    labels: Query<Entity, With<SegmentedTabLabel>>,
) {
    for (entity, selected, seed, children) in &controls {
        // SelectedTab is a required component, so a fresh strip carries
        // `Some(&SelectedTab(None))`: seed whenever the VALUE is unset.
        if selected.and_then(|s| s.0).is_none() {
            let target = children
                .iter()
                .copied()
                .find(|child| tabs.get(*child).is_ok_and(|(tab, _)| tab.0 == seed.0));
            if let Some(tab) = target {
                commands.entity(entity).insert(SelectedTab(Some(tab)));
            }
            continue;
        }
        let Some(selected_entity) = selected.and_then(|s| s.0) else {
            continue;
        };
        let selected_index = tabs
            .get(selected_entity)
            .ok()
            .map(|(tab, _)| tab.0)
            .or_else(|| {
                children
                    .iter()
                    .copied()
                    .find(|child| *child == selected_entity)
                    .and_then(|child| tabs.get(child).ok().map(|(tab, _)| tab.0))
            });
        let Some(selected_index) = selected_index else {
            continue;
        };
        let count = children
            .iter()
            .filter(|child| tabs.contains(**child))
            .count();
        let (left_pct, width_pct) = capsule_metrics(selected_index, count);

        for child in children.iter() {
            if let Ok(mut node) = indicators.get_mut(*child) {
                node.left = Val::Percent(left_pct);
                node.width = Val::Percent(width_pct);
            }
            let Ok((_, tab_children)) = tabs.get(*child) else {
                continue;
            };
            // Drive ink from the authoritative SelectedTab entity, not the
            // derived Selected marker: the marker lands in PostUpdate, one
            // frame after this system reads it.
            let ink_token = if selected_entity == *child {
                Token::OnAccent
            } else {
                Token::InkDim
            };
            for tab_child in tab_children.iter() {
                if labels.contains(*tab_child) {
                    commands
                        .entity(*tab_child)
                        .insert(ThemeTextColor(ink_token.theme_token()));
                }
            }
        }
    }
}

/// First-frame ink for labels: the strip's declarative seed decides which
/// tab starts selected, so a label never paints with a missing token.
pub fn stamp_initial_tab_ink(
    trigger: On<Add<SegmentedTabLabel>>,
    label_parents: Query<&ChildOf>,
    tab_marks: Query<(&SegmentedTab, &ChildOf), With<Tab>>,
    lists: Query<(), With<SegmentedSelectionSeed>>,
    mut commands: Commands,
) {
    let Ok(label_parent) = label_parents.get(trigger.event().entity) else {
        return;
    };
    let Ok((tab, list)) = tab_marks.get(label_parent.parent()) else {
        return;
    };
    if lists.contains(list.parent()) {
        let ink_token = if tab.0 == 0 {
            Token::OnAccent
        } else {
            Token::InkDim
        };
        commands
            .entity(trigger.event().entity)
            .insert(ThemeTextColor(ink_token.theme_token()));
    }
}
