//! The Settings page (系统设置): system proxy, autostart, TUN stack mode,
//! mixed-port routing, LAN sharing, and core controller configuration.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SettingsLine`]).
//! [`SettingsPagePlugin`] registers [`apply_settings_projection`] and action observers
//! once at product assembly. When [`SettingsProjectionUpdated`]
//! fires, texts and options restamp in place without tree rebuilds.

#[path = "settings_query_access.rs"]
pub mod query_access;
use self::query_access::{
    LogLevelControlItem, SettingsActionControls, SettingsProjectionTargets, SettingsTextOutputItem,
    TunStackControlItem,
};

use crate::appearance::{refresh_tonal_ladder_preview, tonal_ladder_preview_scene};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::{localized_checkbox_scene, localized_segmented_scene};
use crate::pages::settings::settings_network_roaming::{
    NetworkRoamingInterfacesLine, NetworkRoamingRouteLine, NetworkRoamingStatusLine,
};
use crate::pages::settings_language::language_card;
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::settings_status_projection::{format_core_versions, format_integrity};
use infiltrator_bevy_widgets::button::sync_button_disabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::service_mode::ServiceModeState;
use infiltrator_contract::tun::TunStack;
use settings_preferences::{PreferenceKind, preference_row_scene};
use settings_runtime::{RuntimeField, RuntimePolicy};

#[path = "settings_copy.rs"]
pub mod settings_copy;
#[path = "settings_core.rs"]
pub mod settings_core;
#[path = "settings_ipv6.rs"]
pub mod settings_ipv6;
#[path = "settings_lan.rs"]
pub mod settings_lan;
#[path = "settings_network_roaming.rs"]
pub mod settings_network_roaming;
#[path = "settings_offline_startup.rs"]
pub mod settings_offline_startup;
#[path = "settings_pac.rs"]
pub mod settings_pac;
#[path = "settings_preferences.rs"]
pub mod settings_preferences;
#[path = "settings_privileged_network.rs"]
pub mod settings_privileged_network;
#[path = "settings_projection_defaults.rs"]
mod settings_projection_defaults;
#[path = "settings_render_strategy.rs"]
pub mod settings_render_strategy;
#[path = "settings_runtime.rs"]
pub mod settings_runtime;
#[path = "settings_system.rs"]
pub mod settings_system;
#[path = "settings_tun.rs"]
mod settings_tun;
#[path = "settings_vpn.rs"]
pub mod settings_vpn;

use settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};

/// Root marker on the Settings page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SettingsPageRoot;

/// Marker for "Save Settings" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveSettingsButton;

/// Marker for the live local-core rollback action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackButton;

/// Live availability attached to the rollback control and restamped from the
/// shared projection so a later surface update can enable the existing node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackAvailability(pub bool);

/// Marker for the rollback button's mutable label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreRollbackButtonLabel;

/// Marker for the host-owned privileged service-mode action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceModeButton;

/// Live availability attached to the service-mode control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceModeAvailability(pub bool);

/// Marker for the mutable service-mode button label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceModeButtonLabel;

/// Marker for the safe port-conflict repair action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortConflictButton;

/// Marker for "Prepare TUN Permission" button in the alert banner.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrepareTunPermissionButton;

/// Marker for TUN permission warning banner card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunPermissionAlertBanner;

/// Marker for "Close to Tray" toggle button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseToTrayToggle;

/// Marker for "System Notifications" toggle button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemNotificationsToggle;

/// The typed event dispatched when settings data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct SettingsProjectionUpdated(pub SettingsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastSettingsProjection(pub Option<SettingsProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn settings_page(projection: &SettingsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = LocalizedText::plain("settings_summary").render(&UiLocale::default());

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Settings)
            SettingsPageRoot ScrollArea
            Children [
                @{ language_card(palette) }
                --
                @{ tun_permission_alert_banner_scene(palette) }
                --
                @{ header_card_scene(summary, palette) }
                --
                @{ general_card_scene(projection, palette) }
                --
                @{ settings_tun::card(projection, palette) }
                --
                @{ settings_core::controller_settings_card(projection, palette) }
            ]
    }
}

pub fn tun_permission_alert_banner_scene(palette: &UiPalette) -> impl Scene + use<> {
    let border_color = palette.warning;

    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(space::S16), Val::Px(space::S12)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                column_gap: Val::Px(space::S12),
                row_gap: Val::Px(space::S8),
                flex_wrap: FlexWrap::Wrap,
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor {
                top: border_color,
                right: border_color,
                bottom: border_color,
                left: border_color,
            }
            TunPermissionAlertBanner
            Children [
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                    flex_grow: 1.0,
                }
                Children [
                    @{ icon_tile_scene(IconId::Activity, 28.0, palette) }
                    --
                    LocalizedText::plain("settings_tun_permission_hint") TextRole(Role::Body)
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
                PrepareTunPermissionButton
                Button
                Children [
                    LocalizedText::plain("settings_tun_prepare_perm_btn") TextRole(Role::BodyStrong)
                ]
            ]
    }
}

fn header_card_scene(summary: String, palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label(UiLocale::default().text("settings_summary"));

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("settings_summary")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Settings, 36.0, palette) }
                            --
                            Text(summary) SettingsLine(SettingsLineKind::Summary) TextRole(Role::Heading)
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            SaveSettingsButton
                            Button
                            Children [
                                LocalizedText::plain("common_save_apply") TextRole(Role::BodyStrong)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

pub fn general_settings_card(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    general_card_scene(projection, palette)
}

pub fn general_card_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mixed_port_str = settings_copy::line(
        SettingsLineKind::MixedPort,
        projection,
        UiLocale::default().code(),
    )
    .expect("mixed-port copy");
    let core_channel_str = settings_copy::line(
        SettingsLineKind::CoreChannel,
        projection,
        UiLocale::default().code(),
    )
    .expect("core channel copy");
    let core_versions_str =
        format_core_versions(&projection.core_versions, UiLocale::default().code());
    let core_integrity_str =
        format_integrity(&projection.core_integrity, UiLocale::default().code());

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("settings_general_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                @{ localized_checkbox_scene(LocalizedText::plain("settings_autostart_on_boot"), projection.autostart, palette) }
                                --
                                @{ settings_system::toggle_scene(projection.system_proxy, palette) }
                                --
                                @{ settings_system::status_row(&projection.system_proxy_snapshot, &projection.system_proxy_recovery, palette) }
                                --
                                @{ preference_row_scene(PreferenceKind::Tray, projection.close_to_tray, &projection.preference_status, palette) }
                                --
                                @{ preference_row_scene(PreferenceKind::Notifications, projection.notifications_enabled, &projection.preference_status, palette) }
                                --
                                @{ settings_lan::scene(projection, palette) }
                                --
                                @{ settings_pac::scene(projection, palette) }
                                --
                                @{ settings_network_roaming::scene(projection, palette) }
                                --
                                @{ settings_vpn::scene(projection, palette) }
                                --
                                @{ settings_privileged_network::scene(projection, palette) }
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_core_channel_title") TextRole(Role::Body)
                                    --
                                    Text(core_channel_str) SettingsLine(SettingsLineKind::CoreChannel) TextRole(Role::BodyStrong)
                                ]
                                --
                                @{ settings_core::core_rollback_row_scene(projection, palette) }
                                --
                                @{ settings_offline_startup::offline_startup_row_scene(&projection.offline_startup, palette) }
                                --
                                @{ settings_core::controller_auth_row_scene(&projection.controller_auth, palette) }
                                --
                                @{ settings_core::service_mode_row_scene(&projection.service_mode, palette) }
                                --
                                @{ settings_core::port_conflicts_row_scene(&projection.port_conflicts, palette) }
                                --
                                @{ settings_core::core_resources_row_scene(&projection.core_resources, palette) }
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_artifact_integrity_title") TextRole(Role::Body)
                                    --
                                    Text(core_integrity_str) SettingsLine(SettingsLineKind::CoreIntegrity) TextRole(Role::Mono)
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_channel_probe_title") TextRole(Role::Body)
                                    --
                                    Text(core_versions_str) SettingsLine(SettingsLineKind::CoreVersions) TextRole(Role::Mono)
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_mixed_port_label") TextRole(Role::Body)
                                    --
                                    Text(mixed_port_str) SettingsLine(SettingsLineKind::MixedPort) TextRole(Role::Mono)
                                ]
                                --
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S6),
                                    padding: UiRect::top(Val::Px(space::S4)),
                                }
                                Children [
                                    LocalizedText::plain("settings_interface_theme_title") TextRole(Role::Caption)
                                    --
                                    @{ localized_segmented_scene(
                                            &["theme_light", "theme_dark", "theme_forest", "theme_amoled"],
                                            1,
                                            palette,
                                    ) }
                                    --
                                    @{ tonal_ladder_preview_scene(palette) }
                                    --
                                    @{ settings_render_strategy::render_strategy_row_scene(palette) }
                                ]

                            ]
            }),
        ],
        palette,
    )
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct SettingsPagePlugin;

impl Plugin for SettingsPagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (settings_runtime::sync, ApplyDeferred)
                .chain()
                .before(sync_button_disabled),
        );
        app.init_resource::<settings_lan::LanFieldObservations>();
        app.add_systems(Update, refresh_tonal_ladder_preview);
        settings_render_strategy::register(app);
        app.add_observer(apply_settings_projection);
        app.add_observer(on_settings_action_activated);
        app.add_observer(settings_core::on_mtu_probe_activated);
        app.add_observer(settings_core::on_tun_route_changed);
        app.add_observer(settings_core::on_tun_enabled_changed);
        app.add_observer(settings_system::on_changed);
        app.add_observer(settings_tun::apply_tun_toggle_projection);
        app.add_observer(settings_lan::on_toggle_changed);
        app.add_observer(settings_lan::on_apply_activated);
        app.add_observer(settings_lan::on_security_apply_activated);
        app.add_observer(settings_lan::apply_projection);
        app.add_observer(settings_ipv6::on_changed);
        app.add_observer(settings_ipv6::apply_projection);
        app.add_observer(settings_pac::on_changed);
        app.add_observer(settings_pac::on_apply_activated);
        app.add_observer(settings_pac::apply_projection);
        app.add_observer(settings_network_roaming::on_action_activated);
        app.add_systems(Update, settings_network_roaming::replay);
        app.add_observer(settings_network_roaming::initialize::<NetworkRoamingStatusLine>);
        app.add_observer(settings_network_roaming::initialize::<NetworkRoamingInterfacesLine>);
        app.add_observer(settings_network_roaming::initialize::<NetworkRoamingRouteLine>);
        app.add_systems(
            PostUpdate,
            (
                settings_preferences::replay,
                settings_preferences::replay_background,
            )
                .before(sync_button_disabled),
        );
        app.add_systems(Update, settings_preferences::replay_stack_labels);
        app.add_observer(settings_vpn::on_action_activated);
        app.add_systems(
            PostUpdate,
            settings_vpn::replay.before(sync_button_disabled),
        );
        app.add_observer(settings_privileged_network::on_action_activated);
        app.add_systems(
            PostUpdate,
            settings_privileged_network::replay.before(sync_button_disabled),
        );
    }
}

pub(crate) fn on_settings_action_activated(
    activate: On<Activate>,
    handle: Option<Res<CommandSinkHandle>>,
    policy: RuntimePolicy,
    targets: SettingsActionControls,
) {
    let SettingsActionControls {
        save_buttons,
        prepare_buttons,
        tray_toggles,
        notif_toggles,
        preference_disabled,
        rollback_buttons,
        rollback_available,
        log_level_buttons,
        tun_stack_buttons,
        tun_stack_available,
        service_buttons,
        service_available,
        port_buttons,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if preference_disabled
        .get(activate.entity)
        .is_ok_and(|disabled| disabled.0)
    {
        return;
    }
    if save_buttons.contains(activate.entity) {
        handle.submit(UiCommand::UpdateSetting {
            key: "apply".to_owned(),
            value: "true".to_owned(),
        });
    } else if prepare_buttons.contains(activate.entity) {
        handle.submit(UiCommand::UpdateSetting {
            key: "tun_privilege".to_owned(),
            value: "prepare".to_owned(),
        });
    } else if tray_toggles.contains(activate.entity) {
        if let Some(value) = policy.preference("close_to_tray") {
            handle.submit(UiCommand::UpdateSetting {
                key: "close_to_tray".to_owned(),
                value: (!value).to_string(),
            });
        }
    } else if notif_toggles.contains(activate.entity) {
        if let Some(value) = policy.preference("notifications_enabled") {
            handle.submit(UiCommand::UpdateSetting {
                key: "notifications_enabled".to_owned(),
                value: (!value).to_string(),
            });
        }
    } else if rollback_buttons.contains(activate.entity)
        && rollback_available
            .get(activate.entity)
            .is_ok_and(|availability| availability.0)
    {
        handle.submit(UiCommand::RollbackCore);
    } else if let Ok(button) = log_level_buttons.get(activate.entity)
        && policy.available(RuntimeField::LogLevel)
    {
        handle.submit(UiCommand::SetCoreLogLevel(button.level));
    } else if tun_stack_buttons.contains(activate.entity)
        && tun_stack_available
            .get(activate.entity)
            .is_ok_and(|availability| availability.0)
        && let Ok(button) = tun_stack_buttons.get(activate.entity)
    {
        handle.submit(UiCommand::SetTunStack(button.stack));
    } else if service_buttons.contains(activate.entity)
        && service_available
            .get(activate.entity)
            .is_ok_and(|availability| availability.0)
    {
        handle.submit(UiCommand::PrepareServiceMode);
    } else if port_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RepairPortConflicts);
    }
}

pub(crate) fn apply_settings_projection(
    update: On<SettingsProjectionUpdated>,
    palette: Res<UiPalette>,
    locale: Option<Res<UiLocale>>,
    mut last: Option<ResMut<LastSettingsProjection>>,

    targets: SettingsProjectionTargets,
) {
    let SettingsProjectionTargets {
        mut button_queries,
        mut lines,
        mut rollback_buttons,
        mut service_buttons,
    } = targets;

    let projection = &update.0;
    let fallback = UiLocale::default();
    let code = locale.as_deref().unwrap_or(&fallback).code();

    for SettingsTextOutputItem {
        mut text,
        line,
        rollback_label,
        service_label,
    } in &mut lines
    {
        if let Some(line) = line
            && let Some(value) = settings_copy::line(line.0, projection, code)
        {
            text.0 = value;
        }
        if rollback_label.is_some() {
            text.0 = settings_copy::rollback_label(projection, code);
        }
        if service_label.is_some() {
            text.0 = settings_copy::service_label(projection, code);
        }
    }

    let rollback_available = projection.core_versions.rollback.target.is_some();
    for mut availability in &mut rollback_buttons {
        availability.0 = rollback_available;
    }
    let service_ready = projection.service_mode.state == ServiceModeState::Ready;
    for mut availability in &mut service_buttons {
        availability.0 = !service_ready;
    }
    let active_level = projection
        .log_level
        .as_deref()
        .and_then(CoreLogLevel::parse);
    for LogLevelControlItem {
        mut background,
        button,
    } in &mut button_queries.p0()
    {
        background.0 = if active_level == Some(button.level) {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    let active_stack = projection.tun_stack.as_deref().and_then(TunStack::parse);
    for TunStackControlItem {
        mut background,
        mut availability,
        button,
    } in &mut button_queries.p1()
    {
        availability.0 = button.stack.is_live_supported();
        background.0 = if availability.0 && active_stack == Some(button.stack) {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_settings_fixture() {
        let proj = SettingsProjection::demo();
        assert!(proj.autostart);
        assert!(proj.system_proxy);
        assert_eq!(proj.mixed_port, Some(7890));
        assert_eq!(proj.controller_port, Some(9090));
        assert_eq!(proj.allow_lan, Some(false));
        assert_eq!(proj.log_level.as_deref(), Some("info"));
        assert_eq!(proj.tun_stack.as_deref(), Some("gvisor"));
    }
}
