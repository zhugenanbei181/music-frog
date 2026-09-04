//! 3-Way Merge sync conflict component (字段级三向冲突差异合并).

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

/// Marker on the 3-Way Merge card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncMergeRoot;

/// Marker for accepting local configuration button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptLocalButton;

/// Marker for accepting cloud configuration button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptCloudButton;

/// Marker for merging both configurations button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeBothButton;

/// Construct declarative scene for 3-Way Merge & Conflict Resolver card.
pub fn sync_three_way_merge_scene(palette: &UiPalette) -> impl Scene + use<> {
    let conflict_items = vec![
        "[规则差异] 本地: 142 条规则 vs 云端: 168 条规则",
        "[节点差异] 本地: SS-Tokyo-01 vs 云端: VLESS-Reality-01",
    ];

    let conflict_rows: Vec<Box<dyn Scene>> = conflict_items
        .into_iter()
        .map(|desc| {
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ desc.to_owned() }) TextRole(Role::Body) ),
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
                SyncMergeRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Network, 24.0, palette) } ),
                            ( Text({ "字段级三向冲突差异合并 (3-Way Merge & Conflict Resolver)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                Children [
                    { conflict_rows },
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
                    (
                        Node {
                            min_height: px(palette.control_height_px * 0.85),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        AcceptLocalButton
                        Children [
                            ( Text({ "以本地为准".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px * 0.85),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        AcceptCloudButton
                        Children [
                            ( Text({ "以云端为准".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px * 0.85),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        Button
                        MergeBothButton
                        Children [
                            ( Text({ "智能合并两者".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ "精确到单个策略组与规则条目的三向合并，杜绝无脑覆盖".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
