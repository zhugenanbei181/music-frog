//! Bevy projection and actions for physical-link roaming recovery.

use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::settings::LastSettingsProjection;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::network_status_projection::{
    roaming_interfaces, roaming_route, roaming_status,
};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingRefreshButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkRoamingRepairButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Text)]
pub struct NetworkRoamingStatusLine;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Text)]
pub struct NetworkRoamingInterfacesLine;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Text)]
pub struct NetworkRoamingRouteLine;

pub(super) fn scene(_projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S6),
                    }
                    Children [
                        LocalizedText::plain("settings_network_roaming_title") TextRole(Role::BodyStrong)
                        --
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S4),
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            Text({ String::new() }) NetworkRoamingStatusLine TextRole(Role::Mono)
                            --
                            Text({ String::new() }) NetworkRoamingInterfacesLine TextRole(Role::Caption)
                            --
                            Text({ String::new() }) NetworkRoamingRouteLine TextRole(Role::Mono)
                            --
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexEnd,
                                column_gap: Val::Px(space::S6),
                            }
                            Children [
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                NetworkRoamingRefreshButton
                                Button
                                Children [
                                    LocalizedText::plain("settings_network_refresh_action") TextRole(Role::Body)
                                ]
                                --
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                NetworkRoamingRepairButton
                                Button
                                Children [
                                    LocalizedText::plain("settings_tun_repair_action") TextRole(Role::BodyStrong)
                                ]
                            ]
                        ]
                    ]
        })],
        palette,
    ))
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    refresh_buttons: Query<(), With<NetworkRoamingRefreshButton>>,
    repair_buttons: Query<(), With<NetworkRoamingRepairButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if refresh_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RefreshNetworkRoaming);
    } else if repair_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RepairNetworkRoutes);
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct RoamingLine {
    text: &'static mut Text,
    status: Option<&'static NetworkRoamingStatusLine>,
    interfaces: Option<&'static NetworkRoamingInterfacesLine>,
    route: Option<&'static NetworkRoamingRouteLine>,
}
#[derive(QueryFilter)]
pub struct RoamingFilter {
    line: Or<(
        With<NetworkRoamingStatusLine>,
        With<NetworkRoamingInterfacesLine>,
        With<NetworkRoamingRouteLine>,
    )>,
}
pub(super) fn replay(
    locale: Res<UiLocale>,
    last: Res<LastSettingsProjection>,
    mut lines: Query<RoamingLine, RoamingFilter>,
) {
    if !locale.is_changed() && !last.is_changed() {
        return;
    }
    let Some(projection) = &last.0 else {
        return;
    };
    let snapshot = &projection.network_roaming;
    let status = roaming_status(&snapshot.status, locale.code());
    let interfaces = roaming_interfaces(snapshot, locale.code());
    let route = roaming_route(snapshot, locale.code());
    for mut line in &mut lines {
        let value = if line.status.is_some() {
            &status
        } else if line.interfaces.is_some() {
            &interfaces
        } else if line.route.is_some() {
            &route
        } else {
            continue;
        };
        line.text.0.clone_from(value);
    }
}

pub(super) fn initialize<T: Component>(
    insert: On<Insert<T>>,
    locale: Res<UiLocale>,
    latest: Res<LatestSurfaceSnapshot>,
    mut lines: Query<RoamingLine, RoamingFilter>,
) {
    let Ok(mut line) = lines.get_mut(insert.entity) else {
        return;
    };
    let snapshot = &latest.0.network_roaming;
    line.text.0 = if line.status.is_some() {
        roaming_status(&snapshot.status, locale.code())
    } else if line.interfaces.is_some() {
        roaming_interfaces(snapshot, locale.code())
    } else {
        roaming_route(snapshot, locale.code())
    };
}
