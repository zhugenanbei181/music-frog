//! QuickJS Script Sandbox Console scene component (JS/TS 脚本沙箱控制台).

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

/// Marker for script sandbox root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptSandboxRoot;

/// Marker for execute script button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecuteScriptButton;

/// Script Sandbox scene.
pub fn script_sandbox_scene(palette: &UiPalette) -> impl Scene + use<> {
    let presets = vec!["国家地区自动分组", "流媒体分流拦截", "开发端口直连"];

    let preset_chips: Vec<Box<dyn Scene>> = presets
        .into_iter()
        .map(|name| {
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
                    ( Text({ name.to_owned() }) TextRole(Role::Caption) ),
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
                ScriptSandboxRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                            ( Text({ "QuickJS 扩展脚本沙箱控制台 (Script Sandbox)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        ExecuteScriptButton
                        Children [
                            ( Text({ "测试运行脚本变换".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    { preset_chips },
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
                    ( Text({ "[沙箱就绪] 内存熔断限制: 64MB · 执行超时熔断: 500ms".to_owned() }) TextRole(Role::Caption) ),
                    ( Text({ "[console.log] Config AST loaded: 32 proxies, 14 rules. Ready to patch.".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
