//! Multi-Profile Aggregator scene component (多订阅节点聚合器).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Marker for the profile aggregator card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileAggregatorRoot;

/// Marker for execute aggregate profiles button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregateProfilesButton;

/// Multi-profile aggregator card scene.
pub fn profiles_aggregator_scene(palette: &UiPalette) -> impl Scene + use<> {
    let regional_groups = vec![
        "🇭🇰 香港自动测速 (12 节点)",
        "🇯🇵 日本自动测速 (18 节点)",
        "🇺🇸 美国自动测速 (14 节点)",
        "🇸🇬 新加坡自动测速 (8 节点)",
    ];

    let group_chips: Vec<Box<dyn Scene>> = regional_groups
        .into_iter()
        .map(|grp| {
            Box::new(bsn! {
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    ( Text({ grp.to_owned() }) TextRole(Role::Caption) ),
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
                ProfileAggregatorRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            ( Text({ "多订阅节点聚合器 (Profile Aggregator)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        AggregateProfilesButton
                        Children [
                            ( Text({ "一键聚合为新配置".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    { group_chips },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ "自动提取全部订阅源节点，清洗去重并按国家生成自动测速策略组".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
