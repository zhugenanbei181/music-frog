//! Custom Rule Builder and Game Preset Injector scene for Bevy UI (自定义规则向导与游戏预设).

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

/// Marker for custom rule builder card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesBuilderRoot;

/// Marker for add custom rule action button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddCustomRuleButton;

/// Marker for inject game presets button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InjectGamePresetsButton;

/// Custom Rule Builder & Game Presets scene.
pub fn rules_builder_scene(palette: &UiPalette) -> impl Scene + use<> {
    let rule_types = vec!["DOMAIN-SUFFIX", "DOMAIN", "IP-CIDR", "GEOIP"];
    let targets = vec!["DIRECT", "REJECT", "PROXY"];

    let type_chips: Vec<Box<dyn Scene>> = rule_types
        .into_iter()
        .map(|t| {
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
                    ( Text({ t.to_owned() }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    let target_chips: Vec<Box<dyn Scene>> = targets
        .into_iter()
        .map(|tgt| {
            Box::new(bsn! {
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                Children [
                    ( Text({ tgt.to_owned() }) TextRole(Role::Caption) ),
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
                RulesBuilderRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Zap, 24.0, palette) } ),
                            ( Text({ "添加自定义规则向导 (Add Custom Rule)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            column_gap: Val::Px(space::S8),
                            align_items: AlignItems::Center,
                        }
                        Children [
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
                                InjectGamePresetsButton
                                Children [
                                    ( Text({ "一键注入游戏分流预设".to_owned() }) TextRole(Role::BodyStrong) ),
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
                                AddCustomRuleButton
                                Children [
                                    ( Text({ "+ 确认添加规则".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
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
                            column_gap: Val::Px(space::S6),
                        }
                        Children [ { type_chips } ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [ { target_chips } ]
                    ),
                ]
            }),
        ],
        palette,
    )
}
