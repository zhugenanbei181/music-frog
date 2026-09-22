//! Connections detail inspection drawer (连接透视). The timing waterfall
//! (DUAL-13-04) is typed unsupported: the core exposes no DNS/TCP/TLS/TTFB
//! stages, so no fabricated bars are drawn.

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

/// Connection detail inspection drawer. The mihomo `/connections` payload does
/// not carry DNS/TCP/TLS/TTFB timings, so this drawer renders an honest
/// unsupported line instead of the previously fabricated waterfall bars
/// (DUAL-13-04).
pub fn connection_drawer_scene(palette: &UiPalette) -> impl Scene + use<> {
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
                            ( Text({ "单连接深度透视 (Deep Telemetry)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    ( Text({ "内核未提供该连接的 DNS/TCP/TLS/TTFB 耗时明细".to_owned() }) TextRole(Role::Caption) ),
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
