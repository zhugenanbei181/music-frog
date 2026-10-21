//! Declarative BSN scenes for the Bevy shell layout.
//!
//! Subtree scenes composing the window root, sidebar rail, navigation
//! items, routing intent pills, content column, title header and bottom bar.

#[path = "shell_nav.rs"]
pub mod nav;
#[path = "shell_sidebar.rs"]
pub mod sidebar;

use crate::a11y::{nav_semantic_node, semantic_node, toggle_semantic_node};
use crate::app::shell_bidi::{ShellDirectionRoot, ShellDirectionRow};
use crate::app::shell_focus::ShellFocusable;
use crate::app::{
    BOTTOM_NAV_CONTENT_HEIGHT, BottomNavActive, BottomNavBar, BottomNavItem, ContentColumn,
    ContentSlot, ContentTitleLabel, DensityToggle, HistoryBackButton, HistoryForwardButton,
    RoutingIntentStatusCapsule, SIDEBAR_WIDTH_PX, ShellHeader, ShellRoot, SidebarExpandedOnly,
    SidebarFoot, SidebarFooterRow, SidebarPanel, StatusDotImage, ThemeToggle,
};
use crate::chrome::chrome_bar_scene;
use crate::pages::connections_confirm::confirmation_scene;
use crate::pages::overview_speedtest::overview_speedtest_detail_modal_scene;
use crate::route::Route;
use crate::shell_readout::{StatusDotSlot, status_point_image, status_point_slot_scene};
use crate::shell_routing_intent_issue;
use bevy::color::Color;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ControlVisual, PillLabel, pill_caption_scene};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::responsive::TouchHitbox;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;

/// The sidebar speed footer's waveform strip width in baseline pixels.
pub const SHELL_WAVEFORM_WIDTH_PX: f32 = 60.0;

/// The history arrow button's edge in baseline pixels.
///
/// The chevron is a text glyph, so its box is sized for the glyph: a 20-pixel
/// chevron in a 28-pixel button reads as a control, not as a smudge.
const HISTORY_ARROW_PX: f32 = 28.0;

/// The bottom navigation's icon edge in baseline pixels.
const BOTTOM_NAV_ICON_PX: f32 = 18.0;

/// The root shell scene.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    shell_scene_with_toggles(title, &SystemToggleSnapshot::default(), palette)
}

/// Root shell scene with the initial shared system-toggle projection.
pub fn shell_scene_with_toggles(
    title: String,
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let window_node = semantic_node(ShellA11yNode::Window);
    let region_node = semantic_node(ShellA11yNode::ContentRegion);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0), max_width: percent(100),
                height: percent(100),
                min_height: px(0.0), max_height: percent(100),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
            }
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            ShellRoot
            window_node
            Children [
                @{ chrome_bar_scene(palette) }
                --
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    max_width: percent(100),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: px(0.0),
                    min_height: px(0.0),
                    flex_direction: FlexDirection::Row,
                    overflow: Overflow::clip(),
                }
                ShellDirectionRoot
                Children [
                    @{ sidebar_scene_with_toggles(toggles, palette) }
                    --
                    Node {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        min_width: px(0.0),
                        max_width: percent(100),
                        min_height: px(0.0),
                        height: percent(100),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(rem(space::S16)),
                        row_gap: rem(space::S16),
                        overflow: Overflow::clip(),
                    }
                    ContentColumn
                    Children [
                        @{ content_title_row(&title, palette) }
                        --
                        Node {
                            width: percent(100),
                            min_width: px(0.0),
                            max_width: percent(100),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_basis: px(0.0),
                            min_height: px(0.0),
                            overflow: Overflow::clip(),
                        }
                        ContentSlot
                        region_node
                    ]
                ]
                --
                @{ bottom_nav_scene(palette) }
                --
                @{ overview_speedtest_detail_modal_scene(palette) }
                --
                @{ confirmation_scene(palette) }
                --
                @{ nav::tooltip_overlay_scene() }
            ]
    }
}

/// The bottom navigation bar for Compact mode (<600px).
pub fn bottom_nav_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let overview_node = nav_semantic_node(&Route::Overview.label(), false);
    let proxies_node = nav_semantic_node(&Route::Proxies.label(), false);
    let profiles_node = nav_semantic_node(&Route::Profiles.label(), false);
    let settings_node = nav_semantic_node(&Route::Settings.label(), false);
    let edge = palette.border;

    Box::new(bsn! {
            Node {
                width: percent(100),
                height: rem(BOTTOM_NAV_CONTENT_HEIGHT),
                min_height: rem(BOTTOM_NAV_CONTENT_HEIGHT),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                border: UiRect::top(Val::Px(palette.hairline_px)),
                padding: UiRect::bottom(rem(space::S6)),
                display: Display::None,
            }
            ThemeBackgroundColor({ Token::Sidebar })
            BorderColor {
                top: edge,
                right: Color::NONE,
                bottom: Color::NONE,
                left: Color::NONE,
            }
            BottomNavBar
            Children [
                @{ bottom_nav_item_scene(Route::Overview, IconId::Activity, true, palette) }
                overview_node LocalizedLabel::plain(Route::Overview.label_key())
                --
                @{ bottom_nav_item_scene(Route::Proxies, IconId::Globe, false, palette) }
                proxies_node LocalizedLabel::plain(Route::Proxies.label_key())
                --
                @{ bottom_nav_item_scene(Route::Profiles, IconId::FileText, false, palette) }
                profiles_node LocalizedLabel::plain(Route::Profiles.label_key())
                --
                @{ bottom_nav_item_scene(Route::Settings, IconId::Settings, false, palette) }
                settings_node LocalizedLabel::plain(Route::Settings.label_key())
            ]
    })
}

/// One bottom navigation entry: the route icon above its label.
fn bottom_nav_item_scene(
    route: Route,
    icon: IconId,
    active: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let ink = if active {
        palette.accent
    } else {
        palette.ink_dim
    };
    let role = if active {
        Role::BodyStrong
    } else {
        Role::Caption
    };
    Box::new(bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: percent(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: rem(space::S4),
                padding: UiRect::vertical(rem(space::S4)),
            }
            Button
            BottomNavItem(route)
            BottomNavActive(active)
            ShellFocusable
            Children [
                @{ icon_scene(icon, BOTTOM_NAV_ICON_PX, ink) }
                --
                LocalizedText::plain(route.label_key()) TextRole(role)
            ]
    })
}

/// Mount the inline status point into its seat inside the title text.
///
/// The point is a *child* of the text: an inline image is an inline box, so
/// the text layout measures it, wraps with it and keeps it on the baseline.
/// Stamping the component on the text root instead would leave the dot
/// outside the paragraph, where it neither flows nor renders.
pub fn stamp_status_dot(
    trigger: On<Add<StatusDotSlot>>,
    palette: Res<UiPalette>,
    dot_image: Option<Res<StatusDotImage>>,
    mut commands: Commands,
) {
    let Some(dot_image) = dot_image else {
        return;
    };
    let handle = dot_image.0.clone();
    commands
        .entity(trigger.event().entity)
        .insert(status_point_image(handle, palette.ink_dim));
}

/// The title row with page heading, history navigation, and status indicators.
pub fn content_title_row(_title: &str, palette: &UiPalette) -> impl Scene + use<> {
    let header_node = semantic_node(ShellA11yNode::ShellHeader);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: rem(space::S8),
                flex_wrap: FlexWrap::Wrap,
                row_gap: rem(space::S4),
            }
            ShellHeader
            header_node
            ShellDirectionRow
            Children [
                @{ history_arrow_scene("‹", palette) }
                HistoryBackButton
                ShellFocusable
                --
                @{ history_arrow_scene("›", palette) }
                HistoryForwardButton
                ShellFocusable
                --
                LocalizedText::plain("nav_overview") TextRole(Role::Heading) ContentTitleLabel
                Children [
                    @{ status_point_slot_scene() }
                ]
                --
                Node { flex_grow: 1.0 }
                --
                @{ routing_status_capsule_scene(palette) }
                RoutingIntentStatusCapsule
                --
                @{ shell_routing_intent_issue::scene(palette) }
            ]
    }
}

/// The history arrow: a square control whose chevron is sized for its box.
fn history_arrow_scene(glyph: &str, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                width: rem(HISTORY_ARROW_PX),
                height: rem(HISTORY_ARROW_PX),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Button
            ControlVisual(false)
            TouchHitbox::default()
            Children [
                Text({ glyph.to_owned() }) TextRole(Role::Heading) PillLabel
            ]
    }
}

/// The header's routing intent read-out.
///
/// The chip states the live routing intent and offers nothing to press, so it
/// must not wear the skin of a disabled button: a quiet surface, a hairline
/// edge and full ink.
fn routing_status_capsule_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                padding: UiRect::horizontal(rem(space::S12)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                align_items: AlignItems::Center,
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Children [
                Text({ String::new() }) TextRole(Role::Caption) ThemeTextColor({ Token::Ink })
            ]
    }
}

/// Sidebar scene supporting Standard (240px) and polymorphic Rail/Wide modes.
pub fn sidebar_scene(palette: &UiPalette) -> impl Scene + use<> {
    sidebar_scene_with_toggles(&SystemToggleSnapshot::default(), palette)
}

/// The sidebar rail: the cards, then the route rows, then the foot.
pub fn sidebar_scene_with_toggles(
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let pill_node = semantic_node(ShellA11yNode::ThemeToggle);
    let density_node = toggle_semantic_node("Toggle layout density");
    bsn! {
            Node {
                width: rem(SIDEBAR_WIDTH_PX),
                height: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S12)),
                row_gap: rem(space::S8),
                overflow: Overflow::visible(),
            }
            ThemeBackgroundColor({ Token::Sidebar })
            SidebarPanel
            Children [
                @{ sidebar::identity_scene(palette) }
                --
                @{ sidebar::routing_intent_segment_scene(None, palette) }
                --
                @{ sidebar::sidebar_system_toggles_scene(toggles, palette) }
                --
                @{ sidebar::sidebar_profile_card_scene(palette) }
                --
                @{ sidebar::sidebar_shortcut_matrix_scene(palette) }
                --
                @{ sidebar::sidebar_speed_footer_scene(palette) }
                --
                @{ nav::nav_column_scene(palette) }
                --
                Node { flex_grow: 1.0 }
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                SidebarFooterRow
                SidebarExpandedOnly
                Children [
                    Text({ "0.30 demo".to_owned() }) TextRole(Role::Caption)
                    SidebarFoot
                    --
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: rem(space::S4),
                    }
                    Children [
                        @{ pill_caption_scene("Theme".to_owned(), false, palette) }
                        ThemeToggle
                        ShellFocusable
                        pill_node
                        --
                        @{ pill_caption_scene("Density".to_owned(), false, palette) }
                        DensityToggle
                        ShellFocusable
                        density_node
                    ]
                ]
            ]
    }
}
