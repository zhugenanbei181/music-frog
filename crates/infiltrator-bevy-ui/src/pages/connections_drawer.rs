//! Connections detail inspection drawer & latency waterfall component (连接透视与耗时瀑布流).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Marker for the connection inspection drawer root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionDrawerRoot;

/// Marker for action button on connection drawer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerAddRuleButton;

/// Connection detail inspection drawer scene with DNS/TCP/TLS/TTFB waterfall.
pub fn connection_drawer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let waterfall_items = vec![
        ("DNS 解析", "18 ms", palette.accent),
        ("TCP 握手", "42 ms", palette.success),
        ("TLS 握手", "65 ms", palette.warning),
        ("TTFB 首包", "110 ms", palette.danger),
    ];

    let waterfall_bars: Vec<Box<dyn Scene>> = waterfall_items
        .into_iter()
        .map(|(label, ms, color)| {
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            (
                                Node {
                                    width: px(8.0),
                                    height: px(8.0),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ color })
                            ),
                            ( Text({ label.to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text({ ms.to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                ConnectionDrawerRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "单连接深度透视 (Deep Telemetry Waterfall)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    ( Text({ "api.github.com:443 · AS36459 GitHub, Inc.".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::vertical(Val::Px(space::S8)),
                }
                Children [
                    { waterfall_bars },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "链路: Mixed:7890 -> PROXY -> 香港专线 01 | 规则: DOMAIN-SUFFIX,github.com".to_owned() }) TextRole(Role::Caption) ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        Button
                        DrawerAddRuleButton
                        Children [
                            ( Text({ "一键添加为规则".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}
