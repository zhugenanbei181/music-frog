//! Custom Node Editor & Universal URI Codec scene component (自定义节点与 URI 编解码).

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

/// Marker for custom node editor card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeEditorRoot;

/// Marker for import URI button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportUriButton;

/// Marker for save custom node button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveCustomNodeButton;

/// Custom Node Editor scene.
pub fn custom_node_scene(palette: &UiPalette) -> impl Scene + use<> {
    let protocols = vec!["VLESS Reality", "Shadowsocks 2022", "Hysteria 2", "Trojan"];

    let proto_chips: Vec<Box<dyn Scene>> = protocols
        .into_iter()
        .map(|proto| {
            Box::new(bsn! {
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Button
                Children [
                    ( Text({ proto.to_owned() }) TextRole(Role::Caption) ),
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
                CustomNodeEditorRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Plus, 24.0, palette) } ),
                            ( Text({ "自定义节点与分享链接 (Custom Node & URI Codec)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        ImportUriButton
                        Children [
                            ( Text({ "解析剪贴板 URI".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    { proto_chips },
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
                    ( Text({ "支持 vless://, ss://, trojan://, hysteria2:// 快速录入与生成".to_owned() }) TextRole(Role::Caption) ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.success })
                        Button
                        SaveCustomNodeButton
                        Children [
                            ( Text({ "保存为自定义节点".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}
