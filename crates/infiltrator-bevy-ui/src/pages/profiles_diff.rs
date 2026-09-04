//! Configuration Snapshot Visual Diff & Rollback scene component (快照比对与秒级回滚).

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

/// Marker for snapshot diff root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotDiffRoot;

/// Marker for snapshot rollback button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollbackSnapshotButton;

/// Snapshot Diff & Rollback scene.
pub fn snapshot_diff_scene(palette: &UiPalette) -> impl Scene + use<> {
    let diff_items = vec![
        (
            "+ 新增",
            "proxies: [SS-Tokyo, VLESS-Reality-US, HK-01]",
            palette.success,
        ),
        (
            "- 移除",
            "rules: [DOMAIN-SUFFIX,google.com,DIRECT]",
            palette.danger,
        ),
        (
            "~ 修改",
            "dns.fallback-filter.geoip-code: CN -> US",
            palette.warning,
        ),
    ];

    let diff_rows: Vec<Box<dyn Scene>> = diff_items
        .into_iter()
        .map(|(tag, desc, color)| {
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
                            min_width: px(44.0),
                            padding: UiRect::axes(Val::Px(space::S6), Val::Px(2.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ color })
                        Children [
                            ( Text({ tag.to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text({ desc.to_owned() }) TextRole(Role::Caption) ),
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
                SnapshotDiffRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            ( Text({ "配置历史快照比对 (Snapshot Visual Diff)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        BackgroundColor({ palette.danger })
                        Button
                        RollbackSnapshotButton
                        Children [
                            ( Text({ "一键安全还原此快照".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    { diff_rows },
                ]
            }),
        ],
        palette,
    )
}
