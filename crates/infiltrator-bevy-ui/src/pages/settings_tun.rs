//! Projection restamping for TUN checkboxes.

use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query};
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Checkbox;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use super::SettingsProjectionUpdated;
use super::settings_core::{
    SettingsLine, SettingsLineKind, SettingsProjection, TunEnableToggle, TunRouteToggle,
    TunRouteToggleKind,
};
use super::settings_ipv6;
use super::settings_system::SystemProxyToggle;

pub(super) fn card(projection: &SettingsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let stack_str = projection.tun_stack.clone();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "虚拟网卡模式 (TUN Mode)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    ( { super::settings_core::tun_enable_toggle_scene(projection.tun_enabled, palette) } ),
                    ( { super::settings_core::tun_stack_selector_scene(projection, palette) } ),
                    ( { super::settings_core::tun_route_toggle_scene(TunRouteToggleKind::AutoRoute, "自动路由 (Auto Route)", projection.tun_auto_route, palette) } ),
                    ( { super::settings_core::tun_route_toggle_scene(TunRouteToggleKind::StrictRoute, "严格路由 (Strict Route)", projection.tun_strict_route, palette) } ),
                    ( { settings_ipv6::scene(projection, palette) } ),
                    ( { super::settings_core::mtu_row_scene(&projection.mtu, palette) } ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "TUN 协议栈 (TUN Stack)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(stack_str) SettingsLine(SettingsLineKind::TunStack) TextRole(Role::Body) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

pub(super) fn apply_tun_toggle_projection(
    update: bevy::ecs::observer::On<SettingsProjectionUpdated>,
    mut route_toggles: Query<(&TunRouteToggle, &Children)>,
    mut enable_toggles: Query<&Children, With<TunEnableToggle>>,
    mut system_proxy_toggles: Query<&Children, With<SystemProxyToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    mut commands: Commands,
) {
    let projection = &update.0;
    for (toggle, children) in &mut route_toggles {
        let wanted = match toggle.0 {
            TunRouteToggleKind::AutoRoute => projection.tun_auto_route,
            TunRouteToggleKind::StrictRoute => projection.tun_strict_route,
        };
        restamp_checkbox(&mut commands, &checkboxes, children, wanted);
    }
    for children in &mut enable_toggles {
        restamp_checkbox(&mut commands, &checkboxes, children, projection.tun_enabled);
    }
    for children in &mut system_proxy_toggles {
        restamp_checkbox(
            &mut commands,
            &checkboxes,
            children,
            projection.system_proxy,
        );
    }
}

fn restamp_checkbox(
    commands: &mut Commands,
    checkboxes: &Query<(Entity, Has<Checked>), With<Checkbox>>,
    children: &Children,
    wanted: bool,
) {
    for child in children.iter() {
        if let Ok((entity, checked)) = checkboxes.get(*child)
            && checked != wanted
        {
            if wanted {
                commands.entity(entity).insert(Checked);
            } else {
                commands.entity(entity).remove::<Checked>();
            }
        }
    }
}
