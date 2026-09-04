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
use bevy::ecs::system::{Query, Res, ResMut};
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

use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{PageRoot, Route};

/// Root marker on the Settings page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_settings_page)]
pub struct SettingsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct SettingsPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsLine(pub SettingsLineKind);

/// Different text lines on the settings page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsLineKind {
    /// Overview summary.
    #[default]
    Summary,
    /// Mixed port text.
    MixedPort,
    /// TUN stack text.
    TunStack,
    /// Controller port text.
    ControllerPort,
    /// Log level text.
    LogLevel,
}

/// Marker for "Save Settings" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveSettingsButton;

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

/// Snapshot of the Settings domain.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsProjection {
    pub autostart: bool,
    pub system_proxy: bool,
    pub mixed_port: u16,
    pub allow_lan: bool,
    pub tun_enabled: bool,
    pub tun_stack: String,
    pub controller_port: u16,
    pub log_level: String,
}

impl SettingsProjection {
    /// Believable demo fixture for the Settings page.
    pub fn demo() -> Self {
        Self {
            autostart: true,
            system_proxy: true,
            mixed_port: 7890,
            allow_lan: false,
            tun_enabled: true,
            tun_stack: "gVisor (高性能用户态协议栈)".to_owned(),
            controller_port: 9090,
            log_level: "info".to_owned(),
        }
    }
}

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
            ( { controller_settings_card(projection, palette) } ),
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
                    ( { checkbox_scene("设置系统代理 (Set System Proxy)".to_owned(), projection.system_proxy, palette) } ),
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
                    ( { checkbox_scene("启用 TUN 虚拟网卡接管 (Enable TUN Device)".to_owned(), projection.tun_enabled, palette) } ),
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

fn controller_settings_card(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let ctrl_port_str = format!("127.0.0.1:{}", projection.controller_port);
    let log_level_str = projection.log_level.to_uppercase();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "外部控制器与核心 (Controller)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
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
                            ( Text({ "外部控制端口 (External Controller API)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(ctrl_port_str) SettingsLine(SettingsLineKind::ControllerPort) TextRole(Role::Mono) ),
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
                            ( Text({ "核心日志级别 (Core Log Level)".to_owned() }) TextRole(Role::Body) ),
                            ( Text(log_level_str) SettingsLine(SettingsLineKind::LogLevel) TextRole(Role::BodyStrong) ),
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
}

pub(crate) fn on_settings_action_activated(
    activate: On<Activate>,
    save_buttons: Query<(), With<SaveSettingsButton>>,
    prepare_buttons: Query<(), With<PrepareTunPermissionButton>>,
    tray_toggles: Query<(), With<CloseToTrayToggle>>,
    notif_toggles: Query<(), With<SystemNotificationsToggle>>,
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
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_settings_projection(
    update: On<SettingsProjectionUpdated>,
    mut last: Option<ResMut<LastSettingsProjection>>,
    mut lines: Query<(&mut Text, &SettingsLine)>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            SettingsLineKind::Summary => {
                text.0 = "系统与内核全局设置 · 统一策略中枢".to_owned();
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
        }
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
