//! Page chrome scenes for the Proxies page: the search/toolbar card and the
//! header summary card.
//!
//! The toolbar owns the search box, the "只看可用" filter toggle, the four
//! sort-mode pills and the "测试地址" launcher; the header card owns the
//! strategy summary, the active exit node and the page-level action
//! triggers (add node, reset group order, view toggle, test all).

use crate::pages::proxies::{
    AddCustomNodeButton, FilterAliveToggle, ProxiesLine, ProxiesLineKind, ProxySortMode,
    ProxySortPill, ResetProxyGroupOrderButton, TestAllProxiesButton, ToggleViewModeButton,
};
use crate::pages::proxies_preferences::{FilterAliveIndicator, ProxyViewLabel};
use crate::pages::proxies_search::search_input_scene;
use crate::pages::proxy_probe_settings::launcher;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node, UiRect,
    Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};

/// The "只看可用" indicator chip's fixed width in baseline pixels.
const FILTER_ALIVE_INDICATOR_WIDTH_PX: f32 = 28.0;

/// Renders the toolbar card containing search box, "只看可用" toggle switch, 4 sort mode pills, and "测试地址" indicator.
pub fn search_bar_card_scene(query: &str, palette: &UiPalette) -> impl Scene + use<> {
    let search_a11y = accesskit::Node::new(accesskit::Role::Search);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: rem(space::S8),
                        column_gap: rem(space::S12),
                    }
                    AccessibilityNode(search_a11y)
                    LocalizedLabel::plain("proxies_search_placeholder")
                    Children [
                        @{ search_input_scene(query, palette) }
                        --
                        Node {
                            height: rem(palette.control_height_px),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(rem(space::S12)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            column_gap: rem(space::S8),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        FilterAliveToggle
                        Button
                        Children [
                            LocalizedText::plain("proxies_filter_alive") TextRole(Role::Caption)
                            --
                            Node {
                                width: rem(FILTER_ALIVE_INDICATOR_WIDTH_PX),
                                height: rem(space::S16),
                                border_radius: BorderRadius::all(rem(radius::SM)),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(rem(space::S2)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated }) FilterAliveIndicator
                            Children [
                                Node {
                                    width: rem(space::S12),
                                    height: rem(space::S12),
                                    border_radius: BorderRadius::all(rem(space::S6)),
                                }
                                ThemeBackgroundColor({ Token::Surface })
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            padding: UiRect::all(rem(space::S2)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            column_gap: rem(space::S4),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        Children [
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::AccentContainer })
                            ProxySortPill(ProxySortMode::LatencyAsc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_latency_ascending") TextRole(Role::Caption) ThemeTextColor({ Token::Accent })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::LatencyDesc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_latency_descending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::NameAsc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_name_ascending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::NameDesc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_name_descending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                        ]
                        --
                        @{ launcher(palette) }
                    ]
        })],
        palette,
    )
}

/// Renders the header summary card for proxy strategies with exit node and action triggers.
pub fn header_card_scene(
    summary: LocalizedText,
    active_exit: String,
    test_status: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let header_a11y = accesskit::Node::new(accesskit::Role::Header);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S16),
                    }
                    AccessibilityNode(header_a11y)
                    LocalizedLabel::plain("proxies_title")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Globe, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S4),
                            }
                            Children [
                                LocalizedText { .. { summary } } ProxiesLine(ProxiesLineKind::Summary) TextRole(Role::Heading)
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    LocalizedText::plain("proxies_active_exit_label") TextRole(Role::Caption)
                                    --
                                    Text(active_exit) ProxiesLine(ProxiesLineKind::ActiveExit) TextRole(Role::BodyStrong)
                                ]
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            AddCustomNodeButton
                            Button
                            Children [
                                @{ icon_scene(IconId::Plus, 14.0, palette.ink) }
                                --
                                LocalizedText::plain("proxies_add_node_btn") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            ResetProxyGroupOrderButton
                            Button
                            Children [
                                LocalizedText::plain("proxies_reorder_reset") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            ToggleViewModeButton
                            Button
                            Children [
                                @{ icon_scene(IconId::Activity, 14.0, palette.ink) }
                                --
                                LocalizedText::plain("proxies_grid_view") ProxyViewLabel TextRole(Role::BodyStrong)
                            ]
                            --
                            LocalizedText { .. { test_status } } ProxiesLine(ProxiesLineKind::TestStatus) TextRole(Role::Caption)
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            TestAllProxiesButton
                            Button
                            Children [
                                LocalizedText::plain("runtime_delay_test_all") TextRole(Role::BodyStrong)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}
