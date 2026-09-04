//! MRS binary rule-set governance and deconstruction component (MRS 二进制规则集治理与解构).

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

/// Marker for the MRS ruleset engine card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesMrsRoot;

/// Marker for the unpack rule provider button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnpackRuleProviderButton;

/// Scene constructor for the MRS Ruleset Engine card.
pub fn rules_mrs_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mrs_items = [
        "geoip.mrs (14,200 条目 · IPCIDR · 高性能 mmap 索引)",
        "geosite-cn.mrs (28,500 条目 · Domain · 二进制缓存正常)",
    ];

    let item_scenes: Vec<Box<dyn Scene>> = mrs_items
        .into_iter()
        .map(|item| {
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::all(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    ( Text({ item.to_owned() }) TextRole(Role::Body) ),
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
                RulesMrsRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "MRS 二进制规则集治理与解构 (MRS Ruleset Engine)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
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
                        UnpackRuleProviderButton
                        Children [
                            ( Text({ "一键解构导入为本地规则".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    { item_scenes },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ "支持本地 .mrs 二进制规则集秒级索引与 diff 比对".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
