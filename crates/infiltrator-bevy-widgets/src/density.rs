//! Density scaling node system.
//!
//! Controls and containers carrying [`AdaptiveDensityNode`] automatically adjust
//! padding, gaps, and heights when layout density changes between Compact and Comfortable.

use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::ui::prelude::{Node, UiRect, Val};

use crate::responsive::{Density, ResponsiveContext};

/// Marker component on a node that automatically scales its padding, gap, and height with Density.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveDensityNode {
    /// Base un-scaled padding in pixels.
    pub base_padding_px: f32,
    /// Base un-scaled gap in pixels.
    pub base_gap_px: f32,
    /// Optional base fixed height in pixels.
    pub base_height_px: Option<f32>,
}

impl Default for AdaptiveDensityNode {
    fn default() -> Self {
        Self {
            base_padding_px: 16.0,
            base_gap_px: 12.0,
            base_height_px: None,
        }
    }
}

impl AdaptiveDensityNode {
    pub fn new(padding: f32, gap: f32) -> Self {
        Self {
            base_padding_px: padding,
            base_gap_px: gap,
            base_height_px: None,
        }
    }

    pub fn with_height(mut self, height: f32) -> Self {
        self.base_height_px = Some(height);
        self
    }
}

/// System to sync padding, gap, and height of [`AdaptiveDensityNode`] components with active density.
pub fn sync_adaptive_density_styles(
    ctx: Option<Res<ResponsiveContext>>,
    density_res: Option<Res<Density>>,
    mut query: Query<(&mut Node, &AdaptiveDensityNode)>,
) {
    let density = ctx
        .map(|c| c.density)
        .or_else(|| density_res.map(|d| *d))
        .unwrap_or_default();

    for (mut node, adaptive) in &mut query {
        let pad = density.padding(adaptive.base_padding_px);
        let gap = density.gap(adaptive.base_gap_px);

        if node.padding != UiRect::all(Val::Px(pad)) {
            node.padding = UiRect::all(Val::Px(pad));
        }
        if node.column_gap != Val::Px(gap) {
            node.column_gap = Val::Px(gap);
        }
        if node.row_gap != Val::Px(gap) {
            node.row_gap = Val::Px(gap);
        }

        if let Some(base_h) = adaptive.base_height_px {
            let h = density.row_height(base_h);
            if node.height != Val::Px(h) {
                node.height = Val::Px(h);
            }
        }
    }
}
