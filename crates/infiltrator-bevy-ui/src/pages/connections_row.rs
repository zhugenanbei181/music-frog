//! Connection row scene rendering with swipe-to-action support (UI-04-02).
//!
//! The row is one flat band inside the table card: the endpoint, the process
//! tag, the rate and the actions share two lines, so the list reads as a
//! stream instead of a pile of nested cards.

use crate::pages::connections::{
    CloseConnectionButton, ConnChainHopText, ConnHostText, ConnInspectButton, ConnPortText,
    ConnProcessTagText, ConnRuleText, ConnSpeedText,
};
use crate::pages::connections_pulse::connection_pulse_scene;
use crate::pages::connections_search::ConnectionMatchTerm;
use crate::pages::connections_view::ConnectionRow;
use crate::pages::overview::format_rate;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonSize, ButtonVariant, button_with_label_scene};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::responsive::TouchHitbox;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsInk};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use zhuge_ui_core::text::SearchTextRun;

/// One-off layout value (not a ladder rung): the width of the swipe-revealed
/// action drawer that slides in from the row's trailing edge.
const SWIPE_ACTION_DRAWER_WIDTH_PX: f32 = 88.0;

/// One-off layout value (not a ladder rung): the width the rate column keeps,
/// so a changing number never pulls the row's trailing edge around.
const RATE_COLUMN_WIDTH_PX: f32 = 176.0;

/// One-off layout value (not a ladder rung): the width the rate text keeps, so
/// a growing number never pushes the row's other content around.
const RATE_TEXT_WIDTH_PX: f32 = 176.0;

/// Construct a connection row scene supporting horizontal swipe-to-action.
pub fn connection_row_scene(
    idx: usize,
    conn: &ConnectionSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let endpoint = connection_view::display_endpoint(
        connection_view::connection_host(conn),
        &conn.destination_port,
    );
    let (host, port) = split_endpoint(&endpoint);
    let process = connection_view::process_display_name(&conn.process);
    let chain_scenes = connection_chain_scenes(idx, conn);
    let speed_info = format!(
        "↑ {}  ↓ {}",
        format_rate(conn.upload_bps),
        format_rate(conn.download_bps)
    );
    let conn_btn = CloseConnectionButton {
        connection_id: conn.id.clone(),
        connection_idx: idx,
    };
    let conn_btn_drawer = conn_btn.clone();
    let inspect_btn = ConnInspectButton(idx);

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                row_gap: rem(space::S8),
                padding: UiRect::all(rem(space::S12)),
                border: UiRect::bottom(Val::Px(palette.hairline_px)),
            }
            BorderColor { top: { palette.border }, right: { palette.border }, bottom: { palette.border }, left: { palette.border } }
            ConnectionRow(idx)
            SwipeToActionItem {
                offset_x: 0.0,
                max_action_width: 88.0,
            }
            SwipeToActionSpring::default()
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S6),
                }
                SwipeContentContainer
                Children [
                    Node {
                        width: percent(100), min_width: px(0.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S12),
                    }
                    Children [
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            TextRuns(vec![SearchTextRun { text: host, highlighted: false }]) ConnHostText(idx) TextRole(Role::BodyStrong)
                            --
                            TextRuns(vec![SearchTextRun { text: port, highlighted: false }]) ConnPortText(idx) TextRole(Role::Mono) TextRunsInk({ palette.ink_dim })
                            --
                            @{ process_tag_scene(idx, process, palette) }
                        ]
                        --
                        Node {
                            min_width: rem(RATE_COLUMN_WIDTH_PX),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            @{ connection_pulse_scene(idx, conn, palette) }
                            --
                            Node {
                                min_width: rem(RATE_TEXT_WIDTH_PX),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexEnd,
                            }
                            Children [
                                Text(speed_info) ConnSpeedText(idx) TextRole(Role::Mono)
                            ]
                        ]
                    ]
                    --
                    Node {
                        width: percent(100),
                        min_width: px(0.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S12),
                    }
                    Children [
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                            flex_wrap: FlexWrap::Wrap,
                            row_gap: rem(space::S4),
                        }
                        Children [
                            TextRuns(vec![SearchTextRun { text: conn.rule.clone(), highlighted: false }]) ConnRuleText(idx) TextRole(Role::Caption) TextRunsInk({ palette.accent })
                            --
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S4),
                            }
                            Children [
                                { chain_scenes }
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S4),
                        }
                        Children [
                            @{ row_action_scene("common_details", inspect_btn, palette) }
                            --
                            @{ row_action_scene("connections_disconnect_action", conn_btn, palette) }
                        ]
                    ]
                    --
                    TextRuns(Vec::new()) ConnectionMatchTerm(idx) TextRole(Role::Caption)
                ]
                --
                Node {
                    position_type: PositionType::Absolute,
                    right: px(0.0),
                    top: px(0.0),
                    bottom: px(0.0),
                    width: rem(SWIPE_ACTION_DRAWER_WIDTH_PX),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    display: Display::None,
                }
                SwipeActionDrawer
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [
                    Node {
                        padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Danger })
                    Button
                    TouchHitbox::default()
                    conn_btn_drawer
                    Children [
                        LocalizedText::plain("connections_cut_action") TextRole(Role::BodyStrong) ThemeTextColor({ Token::Surface })
                    ]
                ]
            ]
    }
}

/// The process tag: the process name in its own chip, so the rule beside it
/// reads as a rule and not as one flat text pile.
fn process_tag_scene(idx: usize, process: String, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(rem(space::S8), rem(space::S2)),
                border_radius: BorderRadius::MAX,
            }
            BackgroundColor({ palette.surface_elevated })
            TextRuns(vec![SearchTextRun { text: process, highlighted: false }]) ConnProcessTagText(idx) TextRole(Role::Caption)
    }
}

/// One quiet row action: a borderless control, so a column of them never
/// reads as a wall of buttons.
fn row_action_scene<M: Component + Clone + Unpin>(
    key: &'static str,
    marker: M,
    palette: &UiPalette,
) -> impl Scene + use<M> {
    bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain(key) TextRole(Role::Caption) },
                ButtonVariant::Ghost,
                ButtonSize::Sm,
                palette,
            ) }
            TouchHitbox::default()
            marker
    }
}

/// The endpoint split into its host and its port, so the port can carry its
/// own ink instead of being welded to the domain name.
fn split_endpoint(endpoint: &str) -> (String, String) {
    match endpoint.rfind(':') {
        Some(cut) => (endpoint[..cut].to_owned(), endpoint[cut + 1..].to_owned()),
        None => (endpoint.to_owned(), String::new()),
    }
}

/// DUAL-13-06: render one flat row's route chain as one text per hop, through
/// the shared parsed chain model (no pre-joined string).
fn connection_chain_scenes(idx: usize, conn: &ConnectionSnapshot) -> Vec<Box<dyn Scene>> {
    let chain = connection_view::route_chain(conn);
    let mut scenes: Vec<Box<dyn Scene>> = vec![Box::new(bsn! {
            LocalizedText::plain("connections_chain_label") TextRole(Role::Caption)
    }) as Box<dyn Scene>];
    if chain.is_empty() {
        scenes.push(Box::new(bsn! {
                    Text({ "DIRECT".to_owned() }) TextRole(Role::Caption)
        }) as Box<dyn Scene>);
        return scenes;
    }
    for (hop, label) in chain.hops().iter().enumerate() {
        if hop > 0 {
            scenes.push(Box::new(bsn! {
                            Text({ "→".to_owned() }) TextRole(Role::Caption)
            }) as Box<dyn Scene>);
        }
        let hop_label = label.clone();
        scenes.push(Box::new(bsn! {
                    Text(hop_label) ConnChainHopText { row: idx, hop } TextRole(Role::Caption)
        }) as Box<dyn Scene>);
    }
    scenes
}
