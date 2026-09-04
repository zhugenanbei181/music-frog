//! Mini Speed HUD floating desktop scene for Bevy UI (260x90 桌面置顶悬浮窗).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
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

/// Toggle state for Mini HUD mode.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudMode(pub bool);

/// Marker for the Mini HUD scene root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudRoot;

/// Marker for the expand button restoring normal window size.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudExpandButton;

/// Marker for the pin button (always on top).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPinButton;

/// Mini HUD scene: compact 260x90 dashboard showing duplex bandwidth and active node.
pub fn mini_hud_scene(
    up_rate_str: &str,
    down_rate_str: &str,
    node_name: &str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let up_rate = up_rate_str.to_owned();
    let down_rate = down_rate_str.to_owned();
    let node = node_name.to_owned();

    surface_scene(
        vec![
            // Header Row: title, mode capsule, pin, and expand actions
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S4)),
                }
                MiniHudRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 16.0, palette) } ),
                            ( Text({ "Mini HUD".to_owned() }) TextRole(Role::Caption) ),
                            (
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S6), Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    ( Text({ "RULE".to_owned() }) TextRole(Role::Caption) ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S4),
                        }
                        Children [
                            (
                                Node {
                                    min_height: px(20.0),
                                    padding: UiRect::horizontal(Val::Px(space::S6)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.border })
                                Button
                                MiniHudPinButton
                                Children [
                                    ( Text({ "置顶".to_owned() }) TextRole(Role::Caption) ),
                                ]
                            ),
                            (
                                Node {
                                    min_height: px(20.0),
                                    padding: UiRect::horizontal(Val::Px(space::S6)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.accent })
                                Button
                                MiniHudExpandButton
                                Children [
                                    ( Text({ "展开".to_owned() }) TextRole(Role::Caption) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            // Dual-Channel Bandwidth Rates
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::ArrowDown, 16.0, palette) } ),
                            ( Text(down_rate) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::ArrowUp, 16.0, palette) } ),
                            ( Text(up_rate) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            // Footer: Exit node pill
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text(node) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text({ "系统代理: 开启 · TUN: 开启".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::image::Image;
    use bevy::scene::{CommandsSceneExt, ScenePlugin};
    use infiltrator_bevy_widgets::palette::UiPalette;
    use infiltrator_bevy_widgets::theme::Theme;

    #[test]
    fn test_mini_hud_scene_mounting() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        let theme = Theme::dark();
        let palette = UiPalette::new(&theme);

        let scene = mini_hud_scene("120 KB/s", "2.4 MB/s", "HK-01", &palette);
        app.world_mut().commands().spawn_scene(scene);
        app.update();

        let world = app.world_mut();
        let hud_root_count = world.query::<&MiniHudRoot>().iter(world).count();
        assert_eq!(hud_root_count, 1, "MiniHudRoot must mount exactly once");

        let expand_count = world.query::<&MiniHudExpandButton>().iter(world).count();
        assert_eq!(expand_count, 1, "MiniHudExpandButton must mount");

        let pin_count = world.query::<&MiniHudPinButton>().iter(world).count();
        assert_eq!(pin_count, 1, "MiniHudPinButton must mount");
    }
}
