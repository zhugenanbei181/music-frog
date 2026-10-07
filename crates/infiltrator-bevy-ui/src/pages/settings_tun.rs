//! Projection restamping for TUN checkboxes.

use super::settings_core::{
    SettingsLine, SettingsLineKind, SettingsProjection, TunEnableToggle, TunRouteToggle,
    TunRouteToggleKind, mtu_row_scene, tun_enable_toggle_scene, tun_route_toggle_scene,
    tun_stack_selector_scene,
};
use super::settings_system::SystemProxyToggle;
use super::{SettingsProjectionUpdated, settings_ipv6};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query};
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Checkbox;
use infiltrator_application::settings_status_projection::optional_copy;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

pub(super) fn card(projection: &SettingsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let stack_str = optional_copy(projection.tun_stack.as_ref(), UiLocale::default().code());

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("settings_tun_mode_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                @{ tun_enable_toggle_scene(projection.tun_enabled, palette) }
                                --
                                @{ tun_stack_selector_scene(projection, palette) }
                                --
                                @{ tun_route_toggle_scene(TunRouteToggleKind::AutoRoute, "tun_auto_route", projection.tun_auto_route, palette) }
                                --
                                @{ tun_route_toggle_scene(TunRouteToggleKind::StrictRoute, "tun_strict_route", projection.tun_strict_route, palette) }
                                --
                                @{ settings_ipv6::scene(projection, palette) }
                                --
                                @{ mtu_row_scene(&projection.mtu, palette) }
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_tun_stack_title") TextRole(Role::Body)
                                    --
                                    Text(stack_str) SettingsLine(SettingsLineKind::TunStack) TextRole(Role::Body)
                                ]
                            ]
            }),
        ],
        palette,
    )
}

pub(super) fn apply_tun_toggle_projection(
    update: On<SettingsProjectionUpdated>,
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
            Some(projection.system_proxy),
        );
    }
}

fn restamp_checkbox(
    commands: &mut Commands,
    checkboxes: &Query<(Entity, Has<Checked>), With<Checkbox>>,
    children: &Children,
    wanted: Option<bool>,
) {
    for child in children.iter() {
        if let Ok((entity, checked)) = checkboxes.get(*child)
            && checked != (wanted == Some(true))
        {
            if wanted == Some(true) {
                commands.entity(entity).insert(Checked);
            } else {
                commands.entity(entity).remove::<Checked>();
            }
        }
    }
}
