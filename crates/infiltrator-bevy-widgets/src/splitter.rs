//! Splitter: resizable horizontal and vertical split panes with drag handles.
//!
//! **Pure Math Core**: [`SplitterDirection`], [`compute_pane_sizes`], and [`apply_drag_delta`]
//! calculate exact pane sizing, thickness offsets, and clamped fraction adjustments.
//! Zero-bevy and 100% headless-testable.
//!
//! **Scene Adapter**: [`splitter_scene`] builds declarative dual-pane layouts
//! separated by a token-styled interactive resize handle.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Changed, With, Without};
use bevy::ecs::system::Query;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, Val, percent, px,
};
use bevy::ui_widgets::Button;

use crate::palette::UiPalette;

/// Split orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitterDirection {
    /// Left and right panes separated by vertical splitter bar.
    #[default]
    Horizontal,
    /// Top and bottom panes separated by horizontal splitter bar.
    Vertical,
}

/// Component wrapper for splitter direction.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitterDirectionComp(pub SplitterDirection);

/// Pure state of a splitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitterState {
    pub direction: SplitterDirection,
    pub fraction: f32,
    pub min_fraction: f32,
    pub max_fraction: f32,
    pub handle_thickness_px: f32,
}

impl SplitterState {
    pub fn new(direction: SplitterDirection, fraction: f32) -> Self {
        Self {
            direction,
            fraction: fraction.clamp(0.1, 0.9),
            min_fraction: 0.15,
            max_fraction: 0.85,
            handle_thickness_px: 6.0,
        }
    }

    pub fn with_bounds(mut self, min: f32, max: f32) -> Self {
        self.min_fraction = min.clamp(0.0, 0.5);
        self.max_fraction = max.clamp(0.5, 1.0);
        self.fraction = self.fraction.clamp(self.min_fraction, self.max_fraction);
        self
    }
}

/// Compute pixel sizes `(first_px, second_px)` for the two panes. Pure function.
pub fn compute_pane_sizes(fraction: f32, total_size_px: f32, thickness_px: f32) -> (f32, f32) {
    let available = (total_size_px - thickness_px).max(0.0);
    let clamped_f = fraction.clamp(0.0, 1.0);
    let first = (available * clamped_f).round();
    let second = (available - first).max(0.0);
    (first, second)
}

/// Apply pixel drag delta to fraction within bounds. Pure function.
pub fn apply_drag_delta(
    current_fraction: f32,
    delta_px: f32,
    total_size_px: f32,
    min_fraction: f32,
    max_fraction: f32,
) -> f32 {
    if total_size_px <= 0.0 {
        return current_fraction;
    }
    let delta_f = delta_px / total_size_px;
    (current_fraction + delta_f).clamp(min_fraction, max_fraction)
}

/// Marker component on splitter root container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitterRoot;

/// Component storing split fraction on root container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SplitterFraction(pub f32);

/// Marker component on first/primary pane.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitterFirstPane;

/// Marker component on second/secondary pane.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitterSecondPane;

/// Marker component on draggable splitter handle.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SplitterHandle;

/// Message requesting to drag splitter handle.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct SplitterDragEvent {
    pub splitter: Entity,
    pub delta_px: f32,
    pub total_size_px: f32,
}

/// Construct a splitter scene with dual panes and handle.
pub fn splitter_scene(
    direction: SplitterDirection,
    fraction: f32,
    first_pane: Box<dyn Scene>,
    second_pane: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let f_clamped = fraction.clamp(0.1, 0.9);
    let first_pct = f_clamped * 100.0;
    let second_pct = (1.0 - f_clamped) * 100.0;
    let thickness = 6.0;

    let (flex_dir, handle_w, handle_h) = match direction {
        SplitterDirection::Horizontal => (FlexDirection::Row, px(thickness), percent(100)),
        SplitterDirection::Vertical => (FlexDirection::Column, percent(100), px(thickness)),
    };

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: flex_dir,
        }
        SplitterRoot
        SplitterFraction(f_clamped)
        SplitterDirectionComp(direction)
        Children [
            (
                Node {
                    flex_basis: Val::Percent(first_pct),
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                }
                SplitterFirstPane
                Children [
                    ( { first_pane } ),
                ]
            ),
            (
                Node {
                    width: handle_w,
                    height: handle_h,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ palette.surface_elevated })
                Button
                SplitterHandle
            ),
            (
                Node {
                    flex_basis: Val::Percent(second_pct),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                }
                SplitterSecondPane
                Children [
                    ( { second_pane } ),
                ]
            ),
        ]
    }
}

/// Advance splitters from drag messages.
pub fn advance_splitters(
    mut events: MessageReader<SplitterDragEvent>,
    mut splitters: Query<&mut SplitterFraction>,
) {
    for event in events.read() {
        if let Ok(mut frac) = splitters.get_mut(event.splitter) {
            frac.0 = apply_drag_delta(frac.0, event.delta_px, event.total_size_px, 0.1, 0.9);
        }
    }
}

/// Repaint pane basis when [`SplitterFraction`] changes.
#[allow(clippy::type_complexity)]
pub fn sync_splitter_visuals(
    splitters: Query<(Entity, &SplitterFraction), Changed<SplitterFraction>>,
    groups: Query<&Children>,
    mut firsts: Query<&mut Node, (With<SplitterFirstPane>, Without<SplitterSecondPane>)>,
    mut seconds: Query<&mut Node, (With<SplitterSecondPane>, Without<SplitterFirstPane>)>,
) {
    for (entity, frac) in &splitters {
        let first_pct = frac.0 * 100.0;
        let second_pct = (1.0 - frac.0) * 100.0;

        if !groups.contains(entity) {
            continue;
        }

        for child in groups.iter_descendants(entity) {
            if let Ok(mut node) = firsts.get_mut(child) {
                node.flex_basis = Val::Percent(first_pct);
            }
            if let Ok(mut node) = seconds.get_mut(child) {
                node.flex_basis = Val::Percent(second_pct);
            }
        }
    }
}

/// Standard ergonomic snap fractions (25% sidebar, 33.3% third, 50% half, 66.7% two-thirds).
pub const DEFAULT_SNAP_ANCHORS: &[f32] = &[0.25, 0.333, 0.50, 0.667, 0.75];

/// Snap drag fraction to common geometric anchor points within threshold.
pub fn apply_snap_anchors(raw_fraction: f32, snap_anchors: &[f32], threshold: f32) -> (f32, bool) {
    for &anchor in snap_anchors {
        if (raw_fraction - anchor).abs() <= threshold {
            return (anchor, true);
        }
    }
    (raw_fraction, false)
}

/// Collapsible splitter state enabling double-click collapse/expand toggles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollapsibleSplitterState {
    pub is_collapsed: bool,
    pub saved_fraction: f32,
}

impl Default for CollapsibleSplitterState {
    fn default() -> Self {
        Self {
            is_collapsed: false,
            saved_fraction: 0.5,
        }
    }
}

impl CollapsibleSplitterState {
    pub fn new(default_fraction: f32) -> Self {
        Self {
            is_collapsed: false,
            saved_fraction: default_fraction,
        }
    }

    /// Toggle collapse state on double-click: either collapses to 0.0 or restores previous fraction.
    pub fn on_double_click(&mut self, current_fraction: f32) -> f32 {
        if self.is_collapsed {
            self.is_collapsed = false;
            self.saved_fraction
        } else {
            self.saved_fraction = current_fraction;
            self.is_collapsed = true;
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitter_snap_anchors() {
        let (snapped, did_snap) = apply_snap_anchors(0.492, DEFAULT_SNAP_ANCHORS, 0.02);
        assert!(did_snap);
        assert_eq!(snapped, 0.50);

        let (unchanged, did_snap) = apply_snap_anchors(0.42, DEFAULT_SNAP_ANCHORS, 0.02);
        assert!(!did_snap);
        assert_eq!(unchanged, 0.42);
    }

    #[test]
    fn test_collapsible_splitter_toggle() {
        let mut splitter = CollapsibleSplitterState::new(0.35);
        assert!(!splitter.is_collapsed);

        // Double click collapses
        let collapsed = splitter.on_double_click(0.35);
        assert_eq!(collapsed, 0.0);
        assert!(splitter.is_collapsed);

        // Double click expands back to 0.35
        let restored = splitter.on_double_click(0.0);
        assert_eq!(restored, 0.35);
        assert!(!splitter.is_collapsed);
    }
}
