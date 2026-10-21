//! Systems synchronizing navigation items, rail morphology, hover fill and
//! tooltips for the shell.

use crate::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, LayoutMode, RailNavTooltip, ShellLayoutState,
    SidebarExpandedOnly, SidebarNavItem,
};
use crate::host_capabilities::{HostPreferences, MotionPreference};
use crate::route::{ActiveRoute, Route};
use crate::shell_scene::nav::NavHoverFill;
use bevy::color::Alpha;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::{SurfaceLevel, UiTheme};
use bevy::picking::hover::PickingInteraction;
use bevy::time::Time;
use bevy::ui::prelude::{BackgroundColor, BorderColor, Display, JustifyContent, Node, UiRect};
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::motion::lerp_color;
use infiltrator_bevy_widgets::nav::{NavActive, NavLabel, nav_fill_token, nav_label_ink_token};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::popover::Rect;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_bevy_widgets::tooltip::{TooltipPosition, compute_tooltip_rect};

/// The gap between a route row and the tooltip that names it.
const TOOLTIP_GAP_PX: f32 = space::S8;

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
        UiRect::all(rem(space::S6))
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

/// Ease the hover fill of the idle route rows in and out.
///
/// The pointer's position is the official binary fact; the spring is the
/// eased value that follows it. The host's reduce-motion preference turns the
/// ease into an instant state change — the fill still appears, it simply does
/// not travel.
pub fn sync_nav_hover_fill(
    time: Res<Time>,
    preferences: Res<HostPreferences>,
    palette: Res<UiPalette>,
    items: Query<(Option<&PickingInteraction>, &NavActive, &Children), With<SidebarNavItem>>,
    mut fills: Query<(&mut NavHoverFill, &mut BackgroundColor), With<NavHoverFill>>,
) {
    let reduced = preferences.effective_motion() == MotionPreference::Reduced;
    let delta = time.delta_secs();
    let rest = palette.hover_bg.with_alpha(0.0);
    for (interaction, active, children) in &items {
        let hovered = !active.0 && matches!(interaction, Some(PickingInteraction::Hovered));
        let target = if hovered { 1.0 } else { 0.0 };
        for child in children.iter() {
            if let Ok((mut fill, mut background)) = fills.get_mut(*child) {
                if reduced {
                    fill.0.value = target;
                    fill.0.velocity = 0.0;
                } else {
                    fill.0.target = target;
                }
                let progress = fill.0.step(delta).clamp(0.0, 1.0);
                if fill.0.is_settled(0.01) {
                    fill.0.value = target;
                    fill.0.velocity = 0.0;
                }
                let color = lerp_color(rest, palette.hover_bg, progress);
                if background.0 != color {
                    background.0 = color;
                }
            }
        }
    }
}

/// Display the floating tooltip bubble next to a rail icon when hovered.
///
/// The bubble's placement is the shared tooltip owner's: the row's measured
/// box is the anchor, the owner clamps the bubble to the viewport, and the
/// shell only expresses the owner's answer in the row's own coordinates.
pub fn sync_rail_nav_tooltips(
    layout: Res<ShellLayoutState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    items: Query<
        (
            Option<&PickingInteraction>,
            &ComputedNode,
            &UiGlobalTransform,
            &Children,
        ),
        With<SidebarNavItem>,
    >,
    mut tooltips: Query<(&RailNavTooltip, &mut Node, Option<&ComputedNode>), With<RailNavTooltip>>,
) {
    let is_rail = layout.mode == LayoutMode::Rail;
    let viewport = windows.iter().next().map(|window| Rect {
        x: 0.0,
        y: 0.0,
        w: window.width(),
        h: window.height(),
    });
    for (interaction, computed, transform, children) in &items {
        let hovered = is_rail && matches!(interaction, Some(PickingInteraction::Hovered));
        let size = computed.size();
        let origin = transform.affine().translation - size / 2.0;
        let target = Rect {
            x: origin.x,
            y: origin.y,
            w: size.x,
            h: size.y,
        };
        for child in children.iter() {
            if let Ok((_marker, mut node, bubble)) = tooltips.get_mut(*child) {
                let display = if hovered {
                    Display::Flex
                } else {
                    Display::None
                };
                if node.display != display {
                    node.display = display;
                }
                if !hovered {
                    continue;
                }
                let (Some(viewport), Some(bubble)) = (viewport, bubble) else {
                    continue;
                };
                let bubble_size = bubble.size();
                let placed = compute_tooltip_rect(
                    target,
                    (bubble_size.x, bubble_size.y),
                    viewport,
                    TooltipPosition::Right,
                    TOOLTIP_GAP_PX,
                );
                let left = rem(placed.x - origin.x);
                let top = rem(placed.y - origin.y);
                if node.left != left {
                    node.left = left;
                }
                if node.top != top {
                    node.top = top;
                }
            }
        }
    }
}
