//! The Settings page (系统设置): system proxy, autostart, TUN stack mode,
//! mixed-port routing, LAN sharing, and core controller configuration.
//!
//! **Update seam**: mutable nodes carry typed markers ([`SettingsLine`],
//! [`SettingsToggleMarker`]). The page self-registers
//! [`apply_settings_projection`] once per world via [`SettingsPageRoot`].

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

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
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Settings)
        SettingsPageRoot
        Children [
            ( { header_card_scene(summary, palette) } ),
            ( { general_settings_card(projection, palette) } ),
            ( { tun_settings_card(projection, palette) } ),
            ( { controller_settings_card(projection, palette) } ),
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

fn general_settings_card(
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
}

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
    }
}
