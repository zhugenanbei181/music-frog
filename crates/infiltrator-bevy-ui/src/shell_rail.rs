//! Systems synchronizing navigation items, rail morphology, and tooltips for the shell.

use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::picking::hover::PickingInteraction;
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, BorderColor, Display, JustifyContent, Node, UiRect, Val};
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavLabel, nav_fill, nav_label_ink};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, LayoutMode, RailNavTooltip, ShellLayoutState,
    SidebarExpandedOnly, SidebarNavItem,
};
use crate::route::{ActiveRoute, Route};

/// Sync sidebar navigation items with the active route and live palette.
pub fn sync_sidebar_nav_visuals(
    palette: Res<UiPalette>,
    active_route: Option<Res<ActiveRoute>>,
    mut items: Query<(Entity, &SidebarNavItem, &mut NavActive, &Children)>,
    mut bgs: Query<&mut BackgroundColor>,
    mut texts: Query<(&NavLabel, &mut TextColor, Option<&mut TextRole>)>,
    mut icons: Query<&mut IconTint>,
) {
    let current = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview);
    for (entity, item, mut active_marker, children) in &mut items {
        let is_active = item.0 == current;
        if active_marker.0 != is_active {
            active_marker.0 = is_active;
        }
        let target_fill = nav_fill(is_active, &palette);
        if let Ok(mut bg) = bgs.get_mut(entity)
            && bg.0 != target_fill
        {
            bg.0 = target_fill;
        }
        let target_ink = nav_label_ink(is_active, &palette);
        let target_role = if is_active {
            Role::BodyStrong
        } else {
            Role::Body
        };
        for child in children.iter() {
            if let Ok(mut icon_tint) = icons.get_mut(*child)
                && icon_tint.0 != target_ink
            {
                icon_tint.0 = target_ink;
            }
            if let Ok((_, mut text_color, text_role)) = texts.get_mut(*child) {
                if text_color.0 != target_ink {
                    text_color.0 = target_ink;
                }
                if let Some(mut role) = text_role
                    && role.0 != target_role
                {
                    role.0 = target_role;
                }
            }
        }
    }
}

/// Repaint bottom navigation bar and sync active states with ActiveRoute.
pub fn sync_bottom_nav_visuals(
    palette: Res<UiPalette>,
    active_route: Option<Res<ActiveRoute>>,
    mut bars: Query<(&mut BackgroundColor, &mut BorderColor), With<BottomNavBar>>,
    mut items: Query<(&BottomNavItem, &mut BottomNavActive, &Children)>,
    mut icons: Query<&mut IconTint>,
) {
    let edge = palette.border;
    for (mut fill, mut border) in &mut bars {
        if fill.0 != palette.sidebar {
            fill.0 = palette.sidebar;
        }
        if border.top != edge {
            border.top = edge;
        }
    }

    let current = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview);
    for (item, mut active_marker, children) in &mut items {
        let is_active = item.0 == current;
        if active_marker.0 != is_active {
            active_marker.0 = is_active;
        }
        let target_ink = if is_active {
            palette.accent
        } else {
            palette.ink_dim
        };
        for child in children.iter() {
            if let Ok(mut tint) = icons.get_mut(*child)
                && tint.0 != target_ink
            {
                tint.0 = target_ink;
            }
        }
    }
}

/// Synchronize internal sidebar element morphology between 64px Rail and Standard/Wide modes.
pub fn sync_sidebar_rail_morphology(
    layout: Res<ShellLayoutState>,
    mut nav_items: Query<&mut Node, (With<SidebarNavItem>, Without<SidebarExpandedOnly>)>,
    mut expanded_only: Query<&mut Node, (With<SidebarExpandedOnly>, Without<SidebarNavItem>)>,
) {
    let is_rail = layout.mode == LayoutMode::Rail;

    let nav_justify = if is_rail {
        JustifyContent::Center
    } else {
        JustifyContent::FlexStart
    };
    let nav_padding = if is_rail {
        UiRect::all(Val::Px(space::S8))
    } else {
        UiRect::horizontal(Val::Px(space::S12))
    };
    for mut node in &mut nav_items {
        if node.justify_content != nav_justify {
            node.justify_content = nav_justify;
        }
        if node.padding != nav_padding {
            node.padding = nav_padding;
        }
    }

    let card_display = if is_rail {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut expanded_only {
        if node.display != card_display {
            node.display = card_display;
        }
    }
}

/// Display floating tooltip bubble next to rail icon when hovered in Rail mode.
pub fn sync_rail_nav_tooltips(
    layout: Res<ShellLayoutState>,
    nav_items: Query<(&SidebarNavItem, Option<&PickingInteraction>, &Children)>,
    mut tooltips: Query<(&RailNavTooltip, &mut Node)>,
) {
    let is_rail = layout.mode == LayoutMode::Rail;
    for (_item, interaction, children) in &nav_items {
        let is_hovered = is_rail && matches!(interaction, Some(PickingInteraction::Hovered));
        let target_display = if is_hovered {
            Display::Flex
        } else {
            Display::None
        };
        for child in children.iter() {
            if let Ok((_tooltip, mut node)) = tooltips.get_mut(*child)
                && node.display != target_display
            {
                node.display = target_display;
            }
        }
    }
}
