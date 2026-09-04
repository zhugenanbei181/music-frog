//! Windows UWP Loopback Exemption scene component for Bevy UI (UWP 回环豁免管理).

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

/// Marker for the UWP Exemption card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpExemptionRoot;

/// Marker for exempt all UWP apps button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExemptAllUwpButton;

/// Scene constructor for the UWP Loopback Exemption card.
pub fn uwp_exemption_scene(palette: &UiPalette) -> impl Scene + use<> {
    let sample_uwp_apps = vec![
        ("Microsoft Store", "Exempted", palette.success),
        ("Xbox App", "Exempted", palette.success),
        ("Outlook for Windows", "Isolated", palette.ink_dim),
        ("Windows Terminal", "Exempted", palette.success),
    ];

    let app_rows: Vec<Box<dyn Scene>> = sample_uwp_apps
        .into_iter()
        .map(|(name, status, status_color)| {
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ name.to_owned() }) TextRole(Role::Caption) ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S6), Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ status_color })
                        Children [
                            ( Text({ status.to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
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
                UwpExemptionRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                            ( Text({ "Windows UWP 回环隔离豁免工具 (UWP Loopback Exemption)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        ExemptAllUwpButton
                        Children [
                            ( Text({ "一键豁免全部 UWP 应用".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    { app_rows },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ "解除 Windows 应用商店与现代应用的网络隔离，允许其正常经由本地代理出站".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
