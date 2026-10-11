//! Systems synchronizing navigation items, rail morphology, and tooltips for the shell.

use crate::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, LayoutMode, RailNavTooltip, ShellLayoutState,
    SidebarExpandedOnly, SidebarNavItem,
};
use crate::route::{ActiveRoute, Route};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::{SurfaceLevel, UiTheme};
use bevy::picking::hover::PickingInteraction;
use bevy::ui::prelude::{BorderColor, Display, JustifyContent, Node, UiRect};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavLabel, nav_fill_token, nav_label_ink_token};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Sync sidebar navigation items with the active route and live palette.
pub fn sync_sidebar_nav_visuals(
    theme: Res<UiTheme>,
    active_route: Option<Res<ActiveRoute>>,
    mut commands: Commands,
    mut items: Query<(
        Entity,
        &SidebarNavItem,
        &mut NavActive,
        &Children,
        Option<&ThemeBackgroundColor>,
    )>,
    mut texts: Query<(&NavLabel, Option<&mut TextRole>)>,
    mut icons: Query<&mut IconTint>,
) {
    let current = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview);
    for (entity, item, mut active_marker, children, fill) in &mut items {
        let is_active = item.0 == current;
        if active_marker.0 != is_active {
            active_marker.0 = is_active;
        }
        let fill_token = nav_fill_token(is_active);
        if fill
            .map(|b| b.0 != fill_token.theme_token())
            .unwrap_or(true)
        {
            commands
                .entity(entity)
                .insert(ThemeBackgroundColor(fill_token.theme_token()));
        }
        let target_ink = theme.context_color(
            &nav_label_ink_token(is_active).theme_token(),
            SurfaceLevel::Base,
        );
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
            if let Ok((_, text_role)) = texts.get_mut(*child)
                && let Some(mut role) = text_role
                && role.0 != target_role
            {
                role.0 = target_role;
            }
        }
    }
}

/// Repaint bottom navigation bar and sync active states with ActiveRoute.
pub fn sync_bottom_nav_visuals(
    palette: Res<UiPalette>,
    theme: Res<UiTheme>,
    active_route: Option<Res<ActiveRoute>>,
    mut commands: Commands,
    mut bars: Query<(Entity, &mut BorderColor, Option<&ThemeBackgroundColor>), With<BottomNavBar>>,
    mut items: Query<(&BottomNavItem, &mut BottomNavActive, &Children)>,
    mut icons: Query<&mut IconTint>,
) {
    let edge = palette.border;
    for (entity, mut border, fill) in &mut bars {
        if fill
            .map(|b| b.0 != Token::Sidebar.theme_token())
            .unwrap_or(true)
        {
            commands
                .entity(entity)
                .insert(ThemeBackgroundColor(Token::Sidebar.theme_token()));
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
        let target_token = if is_active {
            Token::Accent
        } else {
            Token::InkDim
        };
        let target_ink = theme.context_color(&target_token.theme_token(), SurfaceLevel::Base);
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
        UiRect::all(rem(space::S8))
    } else {
        UiRect::horizontal(rem(space::S12))
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
