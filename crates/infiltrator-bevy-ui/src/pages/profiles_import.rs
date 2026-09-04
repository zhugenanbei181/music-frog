//! Local profile import & subscription User-Agent configuration scene component.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Marker for the profiles import card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfilesImportRoot;

/// Marker for choosing local file button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChooseLocalFileButton;

/// Marker for importing local file button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportLocalFileButton;

/// Marker for saving user-agent button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveUserAgentButton;

/// Profiles import and User-Agent customization card scene.
pub fn profiles_import_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    surface_scene(
        vec![
            // Section 1: "导入本地配置文件 (Import Local Config)" Header
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S4)),
                }
                ProfilesImportRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            ( Text({ "导入本地配置文件 (Import Local Config)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            // Section 1: Row with path hint box + ChooseLocalFileButton + ImportLocalFileButton
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
                            flex_grow: 1.0,
                            min_height: px(palette.control_height_px),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                        }
                        BackgroundColor({ palette.window_clear })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        Children [
                            ( Text({ "选择或输入本地配置文件路径 (*.yaml, *.yml)...".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        ChooseLocalFileButton
                        Children [
                            ( Text({ "选择文件".to_owned() }) TextRole(Role::Body) ),
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
                        ImportLocalFileButton
                        Children [
                            ( Text({ "+ 导入本地文件".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
                        ]
                    ),
                ]
            }),
            // Section 1: Toggle switch "导入后立即激活"
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S6)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ "导入后立即激活".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ "已开启".to_owned() }) TextRole(Role::Caption) TextColor({ palette.success }) ),
                            (
                                Node {
                                    width: px(38.0),
                                    height: px(22.0),
                                    border: UiRect::all(Val::Px(palette.hairline_px)),
                                    border_radius: BorderRadius::all(Val::Px(11.0)),
                                    position_type: PositionType::Relative,
                                    align_items: AlignItems::Center,
                                }
                                BackgroundColor({ palette.accent })
                                BorderColor {
                                    top: { palette.accent },
                                    right: { palette.accent },
                                    bottom: { palette.accent },
                                    left: { palette.accent },
                                }
                                Children [
                                    (
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(18.0),
                                            width: px(16.0),
                                            height: px(16.0),
                                            border_radius: BorderRadius::all(Val::Px(8.0)),
                                        }
                                        BackgroundColor({ palette.on_accent })
                                    ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            // Section 2: "订阅请求设置 (Subscription User-Agent)" Header
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                    ( Text({ "订阅请求设置 (Subscription User-Agent)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            // Section 2: Custom User-Agent input hint + SaveUserAgentButton
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
                            flex_grow: 1.0,
                            min_height: px(palette.control_height_px),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                        }
                        BackgroundColor({ palette.window_clear })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        Children [
                            ( Text({ "ClashforWindows/0.20.39 Clash.Meta".to_owned() }) TextRole(Role::Mono) TextColor({ palette.ink_dim }) ),
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
                        SaveUserAgentButton
                        Children [
                            ( Text({ "保存 UA 设置".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
                        ]
                    ),
                ]
            }),
            // Section 2: Preset UA badges / description
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S2)),
                }
                Children [
                    ( Text({ "预设 UA:".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "Clash.Meta".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "ClashVerge".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "Shadowrocket".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}
