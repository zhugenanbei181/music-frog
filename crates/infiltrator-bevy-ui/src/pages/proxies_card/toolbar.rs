//! Page chrome scenes for the Proxies page: the search/toolbar card and the
//! header summary card.
//!
//! The toolbar owns the search box, the "只看可用" filter toggle, the sort
//! strip and the "测试地址" launcher; the header card owns the strategy
//! summary, the active exit node and the page-level action triggers (add
//! node, reset group order, view toggle, test all).
//!
//! The sort strip is the OFFICIAL headless tab list (`TabList` / `Tab` /
//! `SelectedTab` / `TabActivation`), so the selection, the keyboard
//! navigation and the accessibility roles belong to the widget layer; the
//! page only owns the four sort orders it offers.

use crate::pages::proxies::{
    AddCustomNodeButton, FilterAliveToggle, ProxiesLine, ProxiesLineKind, ProxySortAdopted,
    ProxySortMode, ProxySortPill, ProxySortTabs, ResetProxyGroupOrderButton, TestAllProxiesButton,
    ToggleViewModeButton,
};
use crate::pages::proxies_preferences::{FilterAliveIndicator, ProxyViewLabel};
use crate::pages::proxies_search::{search_actions_scene, search_input_scene};
use crate::pages::proxy_probe_settings::launcher;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Button, ControlOrientation, Tab, TabActivation, TabList};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::tabs::{
    SegmentedControl, SegmentedPillIndicator, SegmentedSelectionSeed, SegmentedTab,
    SegmentedTabLabel, capsule_metrics,
};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use lurker_core_contract::outbound::preferences::OutboundSortOrder;

/// The "只看可用" indicator chip's fixed width in baseline pixels.
const FILTER_ALIVE_INDICATOR_WIDTH_PX: f32 = 28.0;

/// The sort strip's fixed width in baseline pixels: four equal tabs, so the
/// official capsule indicator lands exactly on the selected tab.
const SORT_STRIP_WIDTH_PX: f32 = 440.0;

/// The sort strip's tab share of the strip, in percent.
const SORT_TAB_WIDTH_PERCENT: f32 = 25.0;

/// The sort tab's height, as a share of the shared control height.
const SORT_TAB_HEIGHT_RATIO: f32 = 0.8;

/// The sort orders the strip offers, in the shared control order.
const SORT_KEYS: [&str; 4] = [
    "outbound_sort_latency_ascending",
    "outbound_sort_latency_descending",
    "outbound_sort_name_ascending",
    "outbound_sort_name_descending",
];

/// The sort mode each strip position selects.
const SORT_MODES: [ProxySortMode; 4] = [
    ProxySortMode::LatencyAsc,
    ProxySortMode::LatencyDesc,
    ProxySortMode::NameAsc,
    ProxySortMode::NameDesc,
];

/// Renders the toolbar card containing the search box, the "只看可用" filter
/// toggle, the sort strip and the "测试地址" launcher.
pub fn search_bar_card_scene(query: &str, palette: &UiPalette) -> impl Scene + use<> {
    let search_a11y = accesskit::Node::new(accesskit::Role::Search);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    AccessibilityNode(search_a11y)
                    LocalizedLabel::plain("proxies_search_placeholder")
                    Children [
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                            flex_wrap: FlexWrap::Wrap,
                            row_gap: rem(space::S8),
                        }
                        Children [
                            @{ search_input_scene(query, palette) }
                            --
                            @{ search_actions_scene(palette) }
                        ]
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                            flex_wrap: FlexWrap::Wrap,
                            row_gap: rem(space::S8),
                        }
                        Children [
                            @{ sort_strip_scene(palette) }
                            --
                            @{ alive_only_filter(palette) }
                            --
                            @{ launcher(palette) }
                        ]
                    ]
        })],
        palette,
    )
}

/// The "只看可用" health filter: a labelled toggle that sits on the same
/// baseline as the rest of the toolbar row.
fn alive_only_filter(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            height: rem(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(rem(space::S12)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            column_gap: rem(space::S8),
            flex_shrink: 0.0,
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
    }
}

/// The sort strip: the official headless tab list over the four shared sort
/// orders, so the selection, the keyboard navigation and the accessibility
/// roles are the widget layer's.
fn sort_strip_scene(palette: &UiPalette) -> impl Scene + use<> {
    let tabs: Vec<Box<dyn Scene>> = SORT_KEYS
        .iter()
        .zip(SORT_MODES.iter())
        .enumerate()
        .map(|(index, (key, mode))| {
            let copy = LocalizedText::plain(*key);
            let mode = *mode;
            Box::new(bsn! {
                Node {
                    width: percent(SORT_TAB_WIDTH_PERCENT),
                    height: rem(palette.control_height_px * SORT_TAB_HEIGHT_RATIO),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::horizontal(rem(space::S8)),
                    border_radius: BorderRadius::MAX,
                    flex_shrink: 0.0,
                }
                Tab
                SegmentedTab(index)
                ProxySortPill(mode)
                Children [
                    LocalizedText { key: { copy.key }, params: { copy.params } }
                    TextRole(Role::Caption)
                    SegmentedTabLabel
                ]
            }) as Box<dyn Scene>
        })
        .collect();
    let (left_pct, width_pct) = capsule_metrics(0, tabs.len());

    bsn! {
        Node {
            position_type: PositionType::Relative,
            width: rem(SORT_STRIP_WIDTH_PX),
            height: rem(palette.control_height_px * SORT_TAB_HEIGHT_RATIO + space::S4 * 2.0),
            margin: UiRect::horizontal(rem(space::S4)),
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
        }
        ThemeBackgroundColor({ Token::SurfaceElevated })
        BorderColor {
            top: { palette.border },
            right: { palette.border },
            bottom: { palette.border },
            left: { palette.border },
        }
        TabList {
            orientation: ControlOrientation::Horizontal,
            activation: TabActivation::Automatic,
        }
        SegmentedControl
        SegmentedSelectionSeed(0)
        ProxySortTabs
        ProxySortAdopted(OutboundSortOrder::LatencyAscending)
        Children [
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(left_pct),
                width: Val::Percent(width_pct),
                top: rem(space::S4),
                height: rem(palette.control_height_px * SORT_TAB_HEIGHT_RATIO),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ Token::Accent })
            SegmentedPillIndicator
            --
            { tabs }
        ]
    }
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
                            flex_shrink: 1.0,
                            min_width: px(0.0),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Globe, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S4),
                                min_width: px(0.0),
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
                        @{ action_group(test_status, palette) }
                    ]
        })],
        palette,
    )
}

/// The page-level action group: the secondary triggers, then the latency
/// state and the test trigger that acts on it.
///
/// The group shares one frame, so the buttons read as one toolbar instead of
/// a row of loose blocks, and every label stays on a single line.
fn action_group(test_status: LocalizedText, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: rem(space::S4),
            row_gap: rem(space::S4),
            flex_wrap: FlexWrap::Wrap,
            padding: UiRect::all(rem(space::S4)),
            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            flex_shrink: 0.0,
        }
        ThemeBackgroundColor({ Token::Surface })
        BorderColor {
            top: { palette.border },
            right: { palette.border },
            bottom: { palette.border },
            left: { palette.border },
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
                flex_shrink: 0.0,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            AddCustomNodeButton
            Button
            Children [
                @{ icon_scene(IconId::Plus, 14.0, palette.ink) }
                --
                LocalizedText::plain("proxies_add_node_btn") TextRole(Role::Caption)
            ]
            --
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S12)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                flex_shrink: 0.0,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            ResetProxyGroupOrderButton
            Button
            Children [
                LocalizedText::plain("proxies_reorder_reset") TextRole(Role::Caption)
            ]
            --
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S12)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                flex_shrink: 0.0,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            ToggleViewModeButton
            Button
            Children [
                @{ icon_scene(IconId::Activity, 14.0, palette.ink) }
                --
                LocalizedText::plain("proxies_grid_view") ProxyViewLabel TextRole(Role::Caption)
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
                flex_shrink: 0.0,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            BorderColor {
                top: { palette.accent },
                right: { palette.accent },
                bottom: { palette.accent },
                left: { palette.accent },
            }
            TestAllProxiesButton
            Button
            Children [
                LocalizedText::plain("runtime_delay_test_all") TextRole(Role::Caption) ThemeTextColor({ Token::Accent })
            ]
            --
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::MAX,
                flex_shrink: 0.0,
            }
            ThemeBackgroundColor({ Token::Surface })
            Children [
                LocalizedText { .. { test_status } } ProxiesLine(ProxiesLineKind::TestStatus) TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
            ]
        ]
    }
}
