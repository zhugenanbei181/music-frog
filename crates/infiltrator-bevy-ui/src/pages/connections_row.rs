//! Connection row scene rendering with swipe-to-action support (UI-04-02).

use crate::pages::connections::{
    CloseConnectionButton, ConnChainHopText, ConnHostText, ConnInspectButton, ConnProcessText,
    ConnSpeedText,
};
use crate::pages::connections_pulse::connection_pulse_scene;
use crate::pages::connections_search::ConnectionMatchTerm;
use crate::pages::connections_view::ConnectionRow;
use crate::pages::overview::format_rate;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, Overflow, PositionType,
    UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::responsive::TouchHitbox;
use infiltrator_bevy_widgets::surface::SurfacePanel;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_runs::TextRuns;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use zhuge_ui_core::text::SearchTextRun;

/// One-off layout value (not a ladder rung): the width of the swipe-revealed
/// action drawer that slides in from the row's trailing edge.
const SWIPE_ACTION_DRAWER_WIDTH_PX: f32 = 88.0;

/// Construct a connection row scene supporting horizontal swipe-to-action.
pub fn connection_row_scene(
    idx: usize,
    conn: &ConnectionSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let host = conn.host.clone();
    let process_info = format!("{} · {}", conn.process, conn.rule);
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
                padding: UiRect::all(rem(space::S16)),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                overflow: Overflow::clip(),
            }
            ThemeBackgroundColor({ Token::Surface })
            SurfacePanel
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
                    row_gap: rem(space::S8),
                }
                SwipeContentContainer
                Children [
                    Node {
                        width: percent(100), min_width: px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S4),
                    }
                    Children [
                        Node {
                            width: percent(100), min_width: px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(space::S4),
                        }
                        Children [
                            TextRuns(vec![SearchTextRun { text: host, highlighted: false }]) ConnHostText(idx) TextRole(Role::BodyStrong)
                            --
                            TextRuns(vec![SearchTextRun { text: process_info, highlighted: false }]) ConnProcessText(idx) TextRole(Role::Caption)
                        ]
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
                    TextRuns(Vec::new()) ConnectionMatchTerm(idx) TextRole(Role::Caption)
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S12),
                    }
                    Children [
                        Text(speed_info) ConnSpeedText(idx) TextRole(Role::Mono)
                        --
                        @{ connection_pulse_scene(idx, conn, palette) }
                        --
                        Node {
                            min_height: px(palette.control_height_px * 0.8),
                            padding: UiRect::horizontal(rem(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        Button
                        TouchHitbox::default()
                        inspect_btn
                        Children [
                            LocalizedText::plain("common_details") TextRole(Role::Caption)
                        ]
                        --
                        Node {
                            min_height: px(palette.control_height_px * 0.8),
                            padding: UiRect::horizontal(rem(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        Button
                        TouchHitbox::default()
                        conn_btn
                        Children [
                            LocalizedText::plain("connections_disconnect_action") TextRole(Role::Caption)
                        ]
                    ]
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
