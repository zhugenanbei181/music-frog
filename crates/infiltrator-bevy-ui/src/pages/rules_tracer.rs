//! Rules Tracer sandbox view for simulating routing and sub-rule decision chains (分流追踪器沙盒).

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

/// Marker on the Rules Tracer card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTracerRoot;

/// Marker on the simulate trace button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimulateRuleTraceButton;

/// Marker for preset chips.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct TracerPresetChip(pub String);

/// Marker for trace result decision tree.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerDecisionTree;

/// Scene constructor for the Live Rule Tracer card.
pub fn rules_tracer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let presets = vec!["google.com", "github.com", "bilibili.com", "1.1.1.1"];

    let preset_chips: Vec<Box<dyn Scene>> = presets
        .into_iter()
        .map(|domain| {
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
                TracerPresetChip({ domain.to_owned() })
                Children [
                    ( Text({ domain.to_owned() }) TextRole(Role::Caption) ),
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
                RulesTracerRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "实时分流追踪器沙盒 (Live Rule Tracer)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        SimulateRuleTraceButton
                        Children [
                            ( Text({ "执行模拟追踪".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S6)),
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
                TracerDecisionTree
                Children [
                    ( Text({ "【匹配命中】规则 #42: DOMAIN-SUFFIX, github.com".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "分流决策链: Inbound(7890) -> Sniffer(TLS) -> GeoSite -> Outbound(PROXY)".to_owned() }) TextRole(Role::Caption) ),
                    ( Text({ "目标策略组: [PROXY] -> 自动测速延迟最优 [香港专线 01] (28ms)".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}
