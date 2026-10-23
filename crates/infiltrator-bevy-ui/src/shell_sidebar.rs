//! The sidebar's cards: identity, routing intent, system toggles, the active
//! profile, the shortcut matrix and the live speed footer.
//!
//! The cards are the rail's content column. Their layout follows the Iced
//! surface's shell (the same order, the same two-column toggle cards, the same
//! icon-above-label tiles), and every measurement travels through the rem
//! channel so one root font size scales the whole rail.

use crate::a11y::{button_semantic_node, semantic_node, switch_node, sync_switch_state};
use crate::app::shell_focus::ShellFocusable;
use crate::app::{
    IDENTITY_TILE_PX, SidebarActiveProfileCard, SidebarExpandedOnly, SidebarIdentityText,
    SidebarRoutingIntentSegment, SidebarShortcutMatrix, SidebarShortcutTile, SidebarSpeedFooter,
    SidebarSystemProxyCard, SidebarSystemProxyToggle, SidebarToggleProjection, SidebarTunCard,
    SidebarTunToggle,
};
use crate::localized_widgets::localized_pill_scene;
use crate::pages::overview::OverviewRoutingIntentPill;
use crate::route::{Route, page_id_for_route};
use crate::shell_readout::{ShellQuotaFill, ShellQuotaTrack, ShellReadoutText};
use crate::shell_scene::SHELL_WAVEFORM_WIDTH_PX;
use crate::shell_waveform::ShellWaveform;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    Overflow, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::switch::{
    SwitchColors, SwitchKnob, SwitchPosition, SwitchTrack, switch_control_scene,
};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_presentation::routing_intent_projection::routing_intent_copy_key;
use lurker_core_contract::routing_intent::RoutingIntent;

/// The toggle card's icon edge in baseline pixels.
const TOGGLE_CARD_ICON_PX: f32 = 14.0;

/// The shortcut tile's icon edge in baseline pixels.
const SHORTCUT_ICON_PX: f32 = 14.0;

/// The speed footer's rate icon edge in baseline pixels.
const RATE_ICON_PX: f32 = 10.0;

/// The count chip's horizontal padding in baseline pixels.
const COUNT_CHIP_PADDING_PX: f32 = 6.0;

/// App identity block: logo plate + "MusicFrog" title + version label.
pub fn identity_scene(palette: &UiPalette) -> impl Scene + use<> {
    let version = concat!("v", env!("CARGO_PKG_VERSION"));
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: rem(space::S12),
            }
            Children [
                @{ icon_tile_scene(IconId::Network, IDENTITY_TILE_PX, palette) }
                --
                Node {
                    flex_direction: FlexDirection::Column,
                }
                SidebarIdentityText
                SidebarExpandedOnly
                Children [
                    Text({ "MusicFrog".to_owned() }) TextRole(Role::BodyStrong)
                    --
                    Text({ version.to_owned() }) TextRole(Role::Caption)
                ]
            ]
    }
}

/// The shared routing intent segment: the three routing choices.
///
/// The segment is a column, not a row: the choices carry full sentences
/// ("Route all traffic through proxy"), and a row of three sentences in a
/// 216-pixel rail wraps every label out of its own pill. Stacked, each pill
/// is as wide as its label, so no choice wraps and none of them overlaps the
/// identity block above.
pub fn routing_intent_segment_scene(
    intent: Option<&RoutingIntent>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let segment_node = semantic_node(ShellA11yNode::RoutingIntentSegment);
    let rule_node = button_semantic_node("");
    let global_node = button_semantic_node("");
    let direct_node = button_semantic_node("");
    bsn! {
            Node {
                align_items: AlignItems::FlexStart,
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
            }
            SidebarRoutingIntentSegment
            SidebarExpandedOnly
            segment_node
            Children [
                @{ localized_pill_scene(LocalizedText::plain(routing_intent_copy_key(&RoutingIntent::FollowRules)), intent == Some(&RoutingIntent::FollowRules), palette) }
                ButtonDisabled(true)
                OverviewRoutingIntentPill(RoutingIntent::FollowRules)
                rule_node
                --
                @{ localized_pill_scene(LocalizedText::plain(routing_intent_copy_key(&RoutingIntent::RouteAllThroughProxy)), intent == Some(&RoutingIntent::RouteAllThroughProxy), palette) }
                ButtonDisabled(true)
                OverviewRoutingIntentPill(RoutingIntent::RouteAllThroughProxy)
                global_node
                --
                @{ localized_pill_scene(LocalizedText::plain(routing_intent_copy_key(&RoutingIntent::BypassProxy)), intent == Some(&RoutingIntent::BypassProxy), palette) }
                ButtonDisabled(true)
                OverviewRoutingIntentPill(RoutingIntent::BypassProxy)
                direct_node
            ]
    }
}

/// Double system toggle cards in sidebar: 系统代理 and TUN 模式.
///
/// The cards carry the shared switch control instead of a text badge: the
/// switch states its own truth, so the card no longer spends a blue pill on
/// the word "On" and the label below it keeps its own line.
pub fn sidebar_system_toggles_scene(
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let proxy_selected = toggles.system_proxy.is_enabled();
    let tun_selected = toggles.tun.is_enabled();
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: rem(space::S6),
            }
            Children [
                @{ system_toggle_card_scene(
                    IconId::Network,
                    "settings_sys_proxy",
                    proxy_selected,
                    SidebarSystemProxyCard,
                    SidebarSystemProxyToggle,
                    switch_node(ShellA11yNode::SystemProxySwitch, proxy_selected),
                    palette,
                ) }
                --
                @{ system_toggle_card_scene(
                    IconId::Zap,
                    "tun_mode",
                    tun_selected,
                    SidebarTunCard,
                    SidebarTunToggle,
                    switch_node(ShellA11yNode::TunSwitch, tun_selected),
                    palette,
                ) }
            ]
    }
}

/// One system toggle card: the icon, the shared switch and the label.
fn system_toggle_card_scene<C: Component + Clone + Unpin, T: Component + Clone + Unpin>(
    icon: IconId,
    copy: &'static str,
    selected: bool,
    card: C,
    toggle: T,
    semantic: AccessibilityNode,
    palette: &UiPalette,
) -> impl Scene + use<C, T> {
    let edge = palette.border;
    bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: percent(50),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S8)),
                row_gap: rem(space::S6),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            card
            SidebarExpandedOnly
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    @{ icon_scene(icon, TOGGLE_CARD_ICON_PX, palette.accent) }
                    --
                    @{ switch_control_scene(palette, switch_colors(selected, palette), SwitchPosition::from_enabled(selected)) }
                    Button
                    toggle
                    ShellFocusable
                    semantic
                ]
                --
                LocalizedText::plain(copy)
                TextRole(Role::Caption)
                ThemeTextColor({ Token::Ink })
            ]
    }
}

/// The switch's colours for one state: the accent track when the toggle is
/// on, the quiet track when it is off.
pub fn switch_colors(selected: bool, palette: &UiPalette) -> SwitchColors {
    if selected {
        SwitchColors {
            track_fill: palette.accent,
            track_edge: palette.accent,
            knob_fill: palette.on_accent,
        }
    } else {
        SwitchColors {
            track_fill: palette.surface_elevated,
            track_edge: palette.border,
            knob_fill: palette.ink,
        }
    }
}

/// Active profile card in sidebar showing subscription name, usage progress bar and percentage.
///
/// The name and the kind are two lines of one block, so a long name wraps
/// inside its own line and never drags the kind onto a broken baseline.
pub fn sidebar_profile_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let semantic = button_semantic_node("nav_profiles");
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S8)),
                row_gap: rem(space::S6),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            SidebarActiveProfileCard
            Button SidebarShortcutTile(Route::Profiles)
            ShellFocusable
            semantic LocalizedLabel::plain("nav_profiles")
            SidebarExpandedOnly
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S8),
                }
                Children [
                    @{ icon_scene(IconId::FileText, TOGGLE_CARD_ICON_PX, palette.accent) }
                    --
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S2),
                    }
                    Children [
                        Text({ String::new() }) ShellReadoutText::ProfileName TextRole(Role::BodyStrong)
                        --
                        Text({ String::new() }) ShellReadoutText::ProfileKind TextRole(Role::Caption)
                        ThemeTextColor({ Token::InkDim })
                    ]
                ]
                --
                Node {
                    width: percent(100),
                    height: rem(space::S4),
                    border_radius: BorderRadius::all(rem(space::S2)),
                    overflow: Overflow::clip(),
                }
                ThemeBackgroundColor({ Token::AccentContainer }) ShellQuotaTrack
                Children [
                    Node {
                        width: percent(0),
                        height: percent(100),
                        border_radius: BorderRadius::all(rem(space::S2)),
                    }
                    ThemeBackgroundColor({ Token::Accent }) ShellQuotaFill
                ]
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Text({ String::new() }) ShellReadoutText::ProfileUsage
                    TextRole(Role::Caption)
                    ThemeTextColor({ Token::InkDim })
                    --
                    Text({ String::new() }) ShellReadoutText::ProfilePercent
                    TextRole(Role::Caption)
                    ThemeTextColor({ Token::InkDim })
                ]
            ]
    }
}

/// Navigation counts replay the complete application observation, independent of list windows.
pub fn sidebar_shortcut_matrix_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S6),
            }
            SidebarShortcutMatrix
            SidebarExpandedOnly
            Children [
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: rem(space::S6),
                }
                Children [
                    @{ shortcut_tile_scene(IconId::Globe, "nav_proxies", Route::Proxies, palette) }
                    --
                    @{ shortcut_tile_scene(IconId::FileText, "nav_rules", Route::Rules, palette) }
                ]
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: rem(space::S6),
                }
                Children [
                    @{ shortcut_tile_scene(IconId::Activity, "nav_connections", Route::Connections, palette) }
                    --
                    @{ shortcut_tile_scene(IconId::Network, "nav_dns", Route::Dns, palette) }
                ]
            ]
    }
}

/// One shortcut tile: the route icon, the live count and the route label.
fn shortcut_tile_scene(
    icon: IconId,
    label: &'static str,
    route: Route,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let semantic = button_semantic_node(label);
    bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: percent(50),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S8)),
                row_gap: rem(space::S4),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Button
            SidebarShortcutTile(route)
            ShellFocusable
            semantic LocalizedLabel::plain(label)
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    @{ icon_scene(icon, SHORTCUT_ICON_PX, palette.accent) }
                    --
                    @{ count_chip_scene(page_id_for_route(route)) }
                ]
                --
                LocalizedText::plain(label)
                TextRole(Role::Caption)
                ThemeTextColor({ Token::Ink })
            ]
    }
}

/// The count chip: a quiet capsule that sizes itself around the number, so a
/// four-digit count never crowds the tile it belongs to.
///
/// The chip sits one step *above* the card it belongs to, and it carries no
/// accent: a count is a quantity, not a state.
fn count_chip_scene(page: PageId) -> impl Scene + use<> {
    bsn! {
            Node {
                padding: UiRect::horizontal(rem(COUNT_CHIP_PADDING_PX)),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            Children [
                Text({ String::new() }) ShellReadoutText::Count(page)
                TextRole(Role::Caption)
                ThemeTextColor({ Token::InkDim })
            ]
    }
}

/// The toggle controls' live projection: the query the sidebar's own
/// restamp reads.
type SidebarToggleQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static mut ControlVisual>,
        &'static Children,
        Option<&'static SidebarSystemProxyToggle>,
        Option<&'static SidebarTunToggle>,
        Option<&'static mut AccessibilityNode>,
    ),
>;

type SwitchTrackQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut BackgroundColor, &'static mut BorderColor),
    (With<SwitchTrack>, Without<SwitchKnob>),
>;

type SwitchKnobQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut SwitchKnob, &'static mut BackgroundColor),
    (With<SwitchKnob>, Without<SwitchTrack>),
>;

/// Restamp the two sidebar switches from the shared projection.
///
/// A pending/unknown/unsupported control is visibly non-actionable and cannot
/// be toggled by the observers above: the switch states the observed truth,
/// and the accessibility row publishes the same truth.
pub fn sync_sidebar_toggle_visuals(
    projection: Res<SidebarToggleProjection>,
    palette: Res<UiPalette>,
    mut toggles: SidebarToggleQuery,
    mut tracks: SwitchTrackQuery,
    mut knobs: SwitchKnobQuery,
    mut commands: Commands,
) {
    for (entity, mut visual, children, proxy_marker, tun_marker, mut semantic) in &mut toggles {
        let Some(state) = proxy_marker
            .map(|_| &projection.0.system_proxy)
            .or_else(|| tun_marker.map(|_| &projection.0.tun))
        else {
            continue;
        };
        let enabled = state.is_enabled();
        if let Some(visual) = visual.as_deref_mut() {
            visual.0 = enabled;
        }
        if let Some(semantic) = semantic.as_deref_mut() {
            sync_switch_state(semantic, enabled);
        }
        let colors = switch_colors(enabled, &palette);
        if let Ok((mut fill, mut edge)) = tracks.get_mut(entity) {
            if fill.0 != colors.track_fill {
                fill.0 = colors.track_fill;
            }
            if edge.top != colors.track_edge {
                edge.set_all(colors.track_edge);
            }
        }
        for child in children.iter() {
            if let Ok((mut knob, mut fill)) = knobs.get_mut(*child) {
                knob.0 = SwitchPosition::from_enabled(enabled);
                if fill.0 != colors.knob_fill {
                    fill.0 = colors.knob_fill;
                }
            }
        }
        if state.can_toggle() {
            commands.entity(entity).remove::<ButtonDisabled>();
        } else {
            commands.entity(entity).insert(ButtonDisabled(true));
        }
    }
}

/// Live speed footer in sidebar with throughput and mini trend indicator.
pub fn sidebar_speed_footer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let rate_node = semantic_node(ShellA11yNode::TrafficReadout);
    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(rem(space::S8)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            SidebarSpeedFooter
            SidebarExpandedOnly
            rate_node
            Children [
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: rem(space::S2) }
                Children [
                Node { width: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween }
                Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S2),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ icon_scene(IconId::ArrowUp, RATE_ICON_PX, palette.success) }
                        --
                        Text({ String::new() }) ShellReadoutText::Upload
                        TextRole(Role::Mono)
                        ThemeTextColor({ Token::Success })
                    ]
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ icon_scene(IconId::ArrowDown, RATE_ICON_PX, palette.accent) }
                        --
                        Text({ String::new() }) ShellReadoutText::Download
                        TextRole(Role::Mono)
                        ThemeTextColor({ Token::Accent })
                    ]
                ]
                --
                Node { width: rem(SHELL_WAVEFORM_WIDTH_PX), height: rem(space::S24) }
                ShellWaveform

                ]
                --
                Text({ String::new() }) ShellReadoutText::RateStatus TextRole(Role::Caption) ThemeTextColor({ Token::Danger })
                ]
            ]
    }
}
