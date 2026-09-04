//! Fluid adaptive card grid layout widget.
//!
//! Automatically wraps cards in a responsive flex grid and recalculates column
//! counts and item basis based on the 4-tier breakpoint context.

use crate::palette::UiPalette;
use crate::responsive::ResponsiveContext;
use crate::theme::Breakpoint;
use crate::theme::space;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{FlexDirection, FlexWrap, Node, Val, percent, px};

/// Configuration for a fluid card grid.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FluidGridConfig {
    /// Minimum width of each card item in pixels.
    pub min_card_width_px: f32,
    /// Maximum number of columns allowed.
    pub max_columns: usize,
    /// Base horizontal gap between columns.
    pub gap_px: f32,
    /// Base vertical gap between rows.
    pub row_gap_px: f32,
}

impl Default for FluidGridConfig {
    fn default() -> Self {
        Self {
            min_card_width_px: 260.0,
            max_columns: 4,
            gap_px: space::S16,
            row_gap_px: space::S16,
        }
    }
}

/// Component marker on the container of a fluid card grid.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FluidCardGrid;

/// Component marker on individual card items inside a fluid card grid.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FluidGridItem;

/// Declarative scene constructor for a fluid adaptive card grid.
pub fn fluid_card_grid_scene(
    items: Vec<Box<dyn Scene>>,
    config: FluidGridConfig,
    _palette: &UiPalette,
) -> Box<dyn Scene> {
    let min_w = config.min_card_width_px;
    let max_cols = config.max_columns;
    let gap = config.gap_px;
    let r_gap = config.row_gap_px;

    let item_scenes: Vec<Box<dyn Scene>> = items
        .into_iter()
        .map(|item| {
            Box::new(bsn! {
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: percent(100),
                    min_width: px(min_w),
                }
                FluidGridItem
                Children [
                    ( { item } ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(gap),
            row_gap: Val::Px(r_gap),
        }
        FluidCardGrid
        FluidGridConfig {
            min_card_width_px: min_w,
            max_columns: max_cols,
            gap_px: gap,
            row_gap_px: r_gap,
        }
        Children [
            { item_scenes },
        ]
    })
}

/// System to sync child card flex basis and gaps based on breakpoint and density.
pub fn sync_fluid_grid_layout(
    ctx: Option<Res<ResponsiveContext>>,
    mut grids: Query<(&FluidGridConfig, &Children, &mut Node), With<FluidCardGrid>>,
    mut items: Query<&mut Node, (With<FluidGridItem>, Without<FluidCardGrid>)>,
) {
    let bp = ctx
        .as_ref()
        .map(|c| c.breakpoint)
        .unwrap_or(Breakpoint::Expanded);
    let density = ctx.as_ref().map(|c| c.density).unwrap_or_default();

    for (config, children, mut grid_node) in &mut grids {
        let gap = density.gap(config.gap_px);
        let row_gap = density.gap(config.row_gap_px);

        if grid_node.column_gap != Val::Px(gap) {
            grid_node.column_gap = Val::Px(gap);
        }
        if grid_node.row_gap != Val::Px(row_gap) {
            grid_node.row_gap = Val::Px(row_gap);
        }

        let target_basis = match bp {
            Breakpoint::Compact => percent(100),
            Breakpoint::Medium => {
                if config.max_columns >= 2 {
                    percent(48)
                } else {
                    percent(100)
                }
            }
            Breakpoint::Expanded => {
                if config.max_columns >= 3 {
                    percent(31)
                } else if config.max_columns == 2 {
                    percent(48)
                } else {
                    percent(100)
                }
            }
            Breakpoint::Ultra => {
                if config.max_columns >= 4 {
                    percent(23)
                } else if config.max_columns == 3 {
                    percent(31)
                } else if config.max_columns == 2 {
                    percent(48)
                } else {
                    percent(100)
                }
            }
        };

        for child in children.iter() {
            if let Ok(mut item_node) = items.get_mut(*child)
                && item_node.flex_basis != target_basis
            {
                item_node.flex_basis = target_basis;
            }
        }
    }
}

/// Analytical ideal column distribution result for balanced multi-column card grids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdealColumnLayout {
    pub columns: usize,
    pub item_width_px: f32,
    pub gap_px: f32,
    pub total_width_px: f32,
}

/// Compute optimal column count and exact item width to fill container width symmetrically.
pub fn compute_ideal_column_layout(
    container_width_px: f32,
    min_card_width_px: f32,
    gap_px: f32,
    max_columns: usize,
) -> IdealColumnLayout {
    let container_w = container_width_px.max(100.0);
    let min_w = min_card_width_px.max(50.0);
    let gap = gap_px.max(0.0);
    let max_cols = max_columns.max(1);

    // Calculate maximum columns that fit: (k * min_w + (k - 1) * gap) <= container_w
    // k * (min_w + gap) - gap <= container_w  ==>  k <= (container_w + gap) / (min_w + gap)
    let potential_cols = ((container_w + gap) / (min_w + gap)).floor() as usize;
    let columns = potential_cols.clamp(1, max_cols);

    // Solve exact item width so columns span exactly 100% of container_w:
    // item_width = (container_w - (columns - 1) * gap) / columns
    let total_gaps = (columns - 1) as f32 * gap;
    let item_width_px = ((container_w - total_gaps) / (columns as f32)).max(1.0);

    IdealColumnLayout {
        columns,
        item_width_px,
        gap_px: gap,
        total_width_px: container_w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_ideal_column_layout() {
        let min_card = 260.0;
        let gap = 16.0;
        let max_cols = 4;

        // 1. Mobile narrow container (360px) -> 1 column
        let mobile = compute_ideal_column_layout(360.0, min_card, gap, max_cols);
        assert_eq!(mobile.columns, 1);
        assert_eq!(mobile.item_width_px, 360.0);

        // 2. Tablet medium container (600px) -> 2 columns
        // 2 * 260 + 16 = 536 <= 600
        let tablet = compute_ideal_column_layout(600.0, min_card, gap, max_cols);
        assert_eq!(tablet.columns, 2);
        // (600 - 16) / 2 = 292.0
        assert_eq!(tablet.item_width_px, 292.0);

        // 3. Desktop wide container (1200px) -> 4 columns (capped by max_cols)
        let desktop = compute_ideal_column_layout(1200.0, min_card, gap, max_cols);
        assert_eq!(desktop.columns, 4);
        // (1200 - 3 * 16) / 4 = 1152 / 4 = 288.0
        assert_eq!(desktop.item_width_px, 288.0);
    }
}
