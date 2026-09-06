//! The Settings page (系统设置): system proxy, autostart, TUN stack mode,
//! mixed-port routing, LAN sharing, and core controller configuration.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SettingsLine`]).
//! The page self-registers [`apply_settings_projection`] and action observers
//! once per world via [`SettingsPageRoot`]. When [`SettingsProjectionUpdated`]
//! fires, texts and options restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{ParamSet, Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::tabs::segmented_control_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::service_mode::ServiceModeState;
use infiltrator_contract::tun::TunStack;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{PageRoot, Route};

#[path = "settings_core.rs"]
pub mod settings_core;
#[path = "settings_tun.rs"]
mod settings_tun;
#[path = "settings_system.rs"]
pub mod settings_system;

use settings_core::{
    CoreLogLevelButton, SettingsLine, SettingsLineKind, SettingsProjection, TunStackButton,
    TunStackButtonAvailability,
};

/// Root marker on the Settings page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_settings_page)]
pub struct SettingsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct SettingsPageBound;

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
    let summary = "系统与内核全局设置 · 统一策略中枢".to_owned();

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
        SettingsPageRoot
        Children [
            ( { tun_permission_alert_banner_scene(palette) } ),
            ( { header_card_scene(summary, palette) } ),
            ( { general_card_scene(projection, palette) } ),
            ( { tun_settings_card(projection, palette) } ),
            ( { settings_core::controller_settings_card(projection, palette) } ),
        ]
    }
}

pub fn tun_permission_alert_banner_scene(palette: &UiPalette) -> impl Scene + use<> {
    let alert_text =
        "⚡ 权限状态: 启用 TUN 前需要为 mihomo 配置平台权限；完成后请重新开启 TUN。".to_owned();
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
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                    flex_grow: 1.0,
                }
                Children [
                    ( { icon_tile_scene(IconId::Activity, 28.0, palette) } ),
                    ( Text(alert_text) TextRole(Role::Body) ),
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
                PrepareTunPermissionButton
                Button
                Children [
                    ( Text({ "准备 TUN 权限".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    }
}

fn header_card_scene(summary: String, palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("系统设置概览");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            template_value(AccessibilityNode(header_a11y))
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( { icon_tile_scene(IconId::Settings, 36.0, palette) } ),
                        ( Text(summary) SettingsLine(SettingsLineKind::Summary) TextRole(Role::Heading) ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
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
                            BackgroundColor({ palette.accent })
                            SaveSettingsButton
                            Button
                            Children [
                                ( Text({ "保存生效".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn close_to_tray_toggle_row_scene(enabled: bool, palette: &UiPalette) -> impl Scene + use<> {
    let text_str = "关闭窗口最小化到托盘 (Close to Tray)".to_owned();
    let status_str = if enabled { "已开启" } else { "已关闭" };
    let status_color = if enabled {
        palette.success
    } else {
        palette.ink_dim
    };
    let switch_bg = if enabled {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let knob_left = if enabled { Val::Px(18.0) } else { Val::Px(2.0) };
    let knob_color = if enabled {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let edge_color = if enabled {
        palette.accent
    } else {
        palette.border
    };

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S6)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(text_str) TextRole(Role::Body) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text({ status_str.to_owned() }) TextRole(Role::Caption) TextColor({ status_color }) ),
                    (
                        Node {
                            width: px(38.0),
                            height: px(22.0),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                        }
                        BackgroundColor({ switch_bg })
                        BorderColor {
                            top: edge_color,
                            right: edge_color,
                            bottom: edge_color,
                            left: edge_color,
                        }
                        CloseToTrayToggle
                        Button
                        Children [
                            (
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: { knob_left },
                                    width: px(16.0),
                                    height: px(16.0),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                }
                                BackgroundColor({ knob_color })
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

fn system_notifications_toggle_row_scene(enabled: bool, palette: &UiPalette) -> impl Scene + use<> {
    let text_str = "系统通知 (System Notifications)".to_owned();
    let status_str = if enabled { "已开启" } else { "已关闭" };
    let status_color = if enabled {
        palette.success
    } else {
        palette.ink_dim
    };
    let switch_bg = if enabled {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let knob_left = if enabled { Val::Px(18.0) } else { Val::Px(2.0) };
    let knob_color = if enabled {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let edge_color = if enabled {
        palette.accent
    } else {
        palette.border
    };

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S6)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(text_str) TextRole(Role::Body) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text({ status_str.to_owned() }) TextRole(Role::Caption) TextColor({ status_color }) ),
                    (
                        Node {
                            width: px(38.0),
                            height: px(22.0),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                        }
                        BackgroundColor({ switch_bg })
                        BorderColor {
                            top: edge_color,
                            right: edge_color,
                            bottom: edge_color,
                            left: edge_color,
                        }
                        SystemNotificationsToggle
                        Button
                        Children [
                            (
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: { knob_left },
                                    width: px(16.0),
                                    height: px(16.0),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                }
                                BackgroundColor({ knob_color })
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
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
    let mixed_port_str = format!("端口: {}", projection.mixed_port);
    let core_channel_str = format!("内核通道: {}", projection.core_channel);
    let core_versions_str = settings_core::format_core_versions(&projection.core_versions);
    let core_integrity_str = settings_core::format_integrity(&projection.core_integrity);

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "常规与系统集成 (General)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    ( { checkbox_scene("开机自动启动 (Autostart on Boot)".to_owned(), projection.autostart, palette) } ),
                    ( { settings_system::toggle_scene(projection.system_proxy, palette) } ),
                    ( { settings_system::status_row(&projection.system_proxy_snapshot, &projection.system_proxy_recovery, palette) } ),
                    ( { close_to_tray_toggle_row_scene(true, palette) } ),
                    ( { system_notifications_toggle_row_scene(true, palette) } ),
                    ( { checkbox_scene("允许局域网连接 (Allow LAN)".to_owned(), projection.allow_lan, palette) } ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "内核版本通道 (Core Channel)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(core_channel_str) SettingsLine(SettingsLineKind::CoreChannel) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    ( { settings_core::core_rollback_row_scene(projection, palette) } ),
                    ( { settings_core::offline_startup_row_scene(&projection.offline_startup, palette) } ),
                    ( { settings_core::controller_auth_row_scene(&projection.controller_auth, palette) } ),
                    ( { settings_core::service_mode_row_scene(&projection.service_mode, palette) } ),
                    ( { settings_core::port_conflicts_row_scene(&projection.port_conflicts, palette) } ),
                    ( { settings_core::core_resources_row_scene(&projection.core_resources, palette) } ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "制品完整性 (SHA-256)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(core_integrity_str) SettingsLine(SettingsLineKind::CoreIntegrity) TextRole(Role::Mono) ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "在线通道探测 (Online Probe)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(core_versions_str) SettingsLine(SettingsLineKind::CoreVersions) TextRole(Role::Mono) ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "混合代理端口 (Mixed Port)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(mixed_port_str) SettingsLine(SettingsLineKind::MixedPort) TextRole(Role::Mono) ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S6),
                            padding: UiRect::top(Val::Px(space::S4)),
                        }
                        Children [
                            ( Text({ "界面主题 (Interface Theme)".to_owned() }) TextRole(Role::Caption) ),
                            ( { segmented_control_scene(
                                vec![
                                    "浅色模式".to_owned(),
                                    "深色模式".to_owned(),
                                    "护眼森林".to_owned(),
                                    "AMOLED".to_owned(),
                                ],
                                1,
                                palette,
                            ) } ),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S6),
                            padding: UiRect::top(Val::Px(space::S4)),
                        }
                        Children [
                            ( Text({ "语言设置 (Language)".to_owned() }) TextRole(Role::Caption) ),
                            ( { segmented_control_scene(
                                vec![
                                    "zh-CN (简体中文)".to_owned(),
                                    "en-US (English)".to_owned(),
                                ],
                                0,
                                palette,
                            ) } ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

fn tun_settings_card(projection: &SettingsProjection, palette: &UiPalette) -> impl Scene + use<> {
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
                    ( { settings_core::tun_enable_toggle_scene(projection.tun_enabled, palette) } ),
                    ( { settings_core::tun_stack_selector_scene(projection, palette) } ),
                    ( { settings_core::tun_route_toggle_scene(settings_core::TunRouteToggleKind::AutoRoute, "自动路由 (Auto Route)", projection.tun_auto_route, palette) } ),
                    ( { settings_core::tun_route_toggle_scene(settings_core::TunRouteToggleKind::StrictRoute, "严格路由 (Strict Route)", projection.tun_strict_route, palette) } ),
                    ( { settings_core::mtu_row_scene(&projection.mtu, palette) } ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
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

// ---- Observer & Update Hook -----------------------------------------------

fn bind_settings_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<SettingsPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(SettingsPageBound);
    commands.add_observer(apply_settings_projection);
    commands.add_observer(on_settings_action_activated);
    commands.add_observer(settings_core::on_mtu_probe_activated);
    commands.add_observer(settings_core::on_tun_route_changed);
    commands.add_observer(settings_core::on_tun_enabled_changed);
    commands.add_observer(settings_system::on_changed);
    commands.add_observer(settings_tun::apply_tun_toggle_projection);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_settings_action_activated(
    activate: On<Activate>,
    save_buttons: Query<(), With<SaveSettingsButton>>,
    prepare_buttons: Query<(), With<PrepareTunPermissionButton>>,
    tray_toggles: Query<(), With<CloseToTrayToggle>>,
    notif_toggles: Query<(), With<SystemNotificationsToggle>>,
    rollback_buttons: Query<(), With<CoreRollbackButton>>,
    rollback_available: Query<&CoreRollbackAvailability, With<CoreRollbackButton>>,
    log_level_buttons: Query<&CoreLogLevelButton>,
    tun_stack_buttons: Query<&TunStackButton>,
    tun_stack_available: Query<&TunStackButtonAvailability, With<TunStackButton>>,
    service_buttons: Query<(), With<ServiceModeButton>>,
    service_available: Query<&ServiceModeAvailability, With<ServiceModeButton>>,
    port_buttons: Query<(), With<PortConflictButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
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
        handle.submit(UiCommand::UpdateSetting {
            key: "close_to_tray".to_owned(),
            value: "toggle".to_owned(),
        });
    } else if notif_toggles.contains(activate.entity) {
        handle.submit(UiCommand::UpdateSetting {
            key: "notifications_enabled".to_owned(),
            value: "toggle".to_owned(),
        });
    } else if rollback_buttons.contains(activate.entity)
        && rollback_available
            .get(activate.entity)
            .is_ok_and(|availability| availability.0)
    {
        handle.submit(UiCommand::RollbackCore);
    } else if let Ok(button) = log_level_buttons.get(activate.entity) {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn apply_settings_projection(
    update: On<SettingsProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastSettingsProjection>>,
    mut lines: Query<(
        &mut Text,
        Option<&SettingsLine>,
        Option<&CoreRollbackButtonLabel>,
        Option<&ServiceModeButtonLabel>,
    )>,
    mut rollback_buttons: Query<&mut CoreRollbackAvailability, With<CoreRollbackButton>>,
    mut button_queries: ParamSet<(
        Query<(&mut BackgroundColor, &CoreLogLevelButton)>,
        Query<(
            &mut BackgroundColor,
            &mut TunStackButtonAvailability,
            &TunStackButton,
        )>,
    )>,
    mut service_buttons: Query<&mut ServiceModeAvailability, With<ServiceModeButton>>,
) {
    let projection = &update.0;

    for (mut text, line, rollback_label, service_label) in &mut lines {
        if let Some(line) = line {
            match line.0 {
            SettingsLineKind::Summary => {
                text.0 = "系统与内核全局设置 · 统一策略中枢".to_owned();
            }
            SettingsLineKind::OfflineStartup => {
                text.0 = settings_core::format_offline_startup(&projection.offline_startup);
            }
            SettingsLineKind::MixedPort => {
                text.0 = format!("端口: {}", projection.mixed_port);
            }
            SettingsLineKind::TunStack => {
                text.0 = projection.tun_stack.clone();
            }
            SettingsLineKind::ControllerPort => {
                text.0 = format!("127.0.0.1:{}", projection.controller_port);
            }
            SettingsLineKind::LogLevel => {
                text.0 = projection.log_level.to_uppercase();
            }
            SettingsLineKind::CoreChannel => {
                text.0 = format!("内核通道: {}", projection.core_channel);
            }
            SettingsLineKind::CoreVersions => {
                text.0 = settings_core::format_core_versions(&projection.core_versions);
            }
            SettingsLineKind::CoreIntegrity => {
                text.0 = settings_core::format_integrity(&projection.core_integrity);
            }
            SettingsLineKind::CoreRollback => {
                text.0 = settings_core::format_rollback_target(&projection.core_versions);
            }
            SettingsLineKind::ControllerAuth => {
                text.0 = settings_core::format_controller_auth(&projection.controller_auth);
            }
            SettingsLineKind::SystemProxy => {
                text.0 = settings_system::format_status(
                    &projection.system_proxy_snapshot,
                    &projection.system_proxy_recovery,
                );
            }
            SettingsLineKind::ServiceMode => {
                text.0 = settings_core::format_service_mode(&projection.service_mode);
            }
            SettingsLineKind::PortConflicts => {
                text.0 = settings_core::format_port_conflicts(&projection.port_conflicts);
            }
            SettingsLineKind::CoreResources => {
                text.0 = settings_core::format_core_resources(&projection.core_resources);
            }
            SettingsLineKind::Mtu => {
                text.0 = settings_core::format_mtu(&projection.mtu);
            }
            }
        }
        if rollback_label.is_some() {
            text.0 = if projection.core_versions.rollback.target.is_some() {
                "立即回滚".to_owned()
            } else {
                "不可用".to_owned()
            };
        }
        if service_label.is_some() {
            text.0 = if projection.service_mode.state == ServiceModeState::Ready {
                "已就绪".to_owned()
            } else {
                "准备服务模式".to_owned()
            };
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
    let active_level = CoreLogLevel::parse(&projection.log_level);
    for (mut background, button) in &mut button_queries.p0() {
        background.0 = if active_level == Some(button.level) {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    let active_stack = TunStack::parse(&projection.tun_stack);
    for (mut background, mut availability, button) in &mut button_queries.p1() {
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
        assert_eq!(proj.mixed_port, 7890);
        assert_eq!(proj.controller_port, 9090);
        assert!(!proj.allow_lan);
        assert_eq!(proj.log_level, "info");
        assert_eq!(proj.tun_stack, "gVisor (高性能用户态协议栈)");
    }
}
