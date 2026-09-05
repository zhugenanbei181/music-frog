//! Bevy Settings scene pieces for local core-version operations.
//!
//! Keeping the rollback control beside its projection-specific scene keeps
//! the main Settings page below the source-size budget without moving any
//! business decision into the widget layer.

use bevy::scene::{Scene, bsn};
use bevy::ecs::hierarchy::Children;
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::version::{CoreArtifactVerification, CoreChannelStatus, CoreVersionSnapshot};
use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::service_mode::{
    ServiceModePlatform, ServiceModeSnapshot, ServiceModeState,
};
use infiltrator_contract::port_conflict::PortConflictSnapshot;

pub(super) fn controller_settings_card(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let ctrl_port_str = format!("127.0.0.1:{}", projection.controller_port);
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
                    ( { core_log_level_row_scene(projection, palette) } ),
                ]
            }),
        ],
        palette,
    )
}

use super::{
    CoreLogLevelButton, CoreRollbackAvailability, CoreRollbackButton, CoreRollbackButtonLabel,
    ServiceModeAvailability, ServiceModeButton, ServiceModeButtonLabel, SettingsLine,
    SettingsLineKind, SettingsProjection, PortConflictButton,
};

pub(super) fn core_rollback_row_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let rollback_text = projection
        .core_versions
        .rollback
        .target
        .as_deref()
        .map_or_else(|| "没有可回滚的本地内核".to_owned(), |version| {
            format!("可回滚至 {version}")
        });
    let rollback_available = projection.core_versions.rollback.target.is_some();
    let action: Box<dyn Scene> = Box::new(bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ if rollback_available { palette.accent } else { palette.surface_elevated } })
        CoreRollbackButton
        CoreRollbackAvailability(rollback_available)
        Button
        Children [
            ( Text({ if rollback_available { "立即回滚".to_owned() } else { "不可用".to_owned() } }) CoreRollbackButtonLabel TextRole(Role::BodyStrong) ),
        ]
    });

    let label: Box<dyn Scene> = Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            ( Text({ "内核版本回滚 (Core Rollback)".to_owned() }) TextRole(Role::Body) ),
            ( Text(rollback_text) SettingsLine(SettingsLineKind::CoreRollback) TextRole(Role::Mono) ),
        ]
    });
    let children = vec![label, action];

    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [ { children } ]
    })
}

pub(super) fn format_integrity(verification: &CoreArtifactVerification) -> String {
    match verification {
        CoreArtifactVerification::Unknown => "未校验".to_owned(),
        CoreArtifactVerification::Verified { version } => {
            format!("已验证 ({version})")
        }
        CoreArtifactVerification::Rejected { version, failure } => {
            format!(
                "已拒绝 ({version}: {})",
                infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text(&failure.message)
            )
        }
    }
}

pub(super) fn controller_auth_row_scene(
    snapshot: &ControllerAuthSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_controller_auth(snapshot);
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            ( Text({ "控制器认证 (Controller Auth)".to_owned() }) TextRole(Role::Body) ),
            ( Text(status) SettingsLine(SettingsLineKind::ControllerAuth) TextRole(Role::Mono) ),
        ]
    })
}

pub(super) fn format_controller_auth(snapshot: &ControllerAuthSnapshot) -> String {
    match snapshot.status {
        ControllerAuthStatus::Unknown => "未探测".to_owned(),
        ControllerAuthStatus::Secured => "已保护 · Bearer".to_owned(),
        ControllerAuthStatus::Missing => "缺少 secret".to_owned(),
        ControllerAuthStatus::Unavailable => "宿主不可用".to_owned(),
    }
}

pub(super) fn format_core_versions(snapshot: &CoreVersionSnapshot) -> String {
    if snapshot.channels.is_empty() {
        return "未探测".to_owned();
    }
    snapshot
        .channels
        .iter()
        .map(|channel| match &channel.status {
            CoreChannelStatus::Ready { release } => {
                format!("{}={}", channel.channel.as_str(), release.version)
            }
            CoreChannelStatus::Failed { .. } => {
                format!("{}=不可用", channel.channel.as_str())
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(super) fn core_log_level_row_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let active = CoreLogLevel::parse(&projection.log_level);
    let buttons: Vec<Box<dyn Scene>> = CoreLogLevel::ALL
        .into_iter()
        .map(|level| Box::new(core_log_level_button_scene(level, active, palette)) as Box<dyn Scene>)
        .collect();
    let current = active.map_or_else(
        || projection.log_level.to_uppercase(),
        |level| level.as_str().to_uppercase(),
    );
    let label: Box<dyn Scene> = Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            ( Text({ "核心日志级别 (Core Log Level)".to_owned() }) TextRole(Role::Body) ),
            ( Text({ format!("当前: {current}") }) SettingsLine(SettingsLineKind::LogLevel) TextRole(Role::Mono) ),
        ]
    });
    let controls: Box<dyn Scene> = Box::new(bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
        }
        Children [ { buttons } ]
    });
    let children = vec![label, controls];

    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [ { children } ]
    })
}

fn core_log_level_button_scene(
    level: CoreLogLevel,
    active: Option<CoreLogLevel>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let selected = active == Some(level);
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ if selected { palette.accent } else { palette.surface_elevated } })
        CoreLogLevelButton { level }
        Button
        Children [
            ( Text({ level.as_str().to_uppercase() }) TextRole(Role::Caption) ),
        ]
    }
}

pub(super) fn service_mode_row_scene(
    snapshot: &ServiceModeSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let ready = snapshot.state == ServiceModeState::Ready;
    let status = format_service_mode(snapshot);
    let label = if ready { "已就绪" } else { "准备服务模式" };
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text({ "特权服务模式 (Service Mode)".to_owned() }) TextRole(Role::Body) ),
                    ( Text(status) SettingsLine(SettingsLineKind::ServiceMode) TextRole(Role::Mono) ),
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
                BackgroundColor({ if ready { palette.surface_elevated } else { palette.accent } })
                ServiceModeButton
                ServiceModeAvailability({ !ready })
                Button
                Children [
                    ( Text({ label.to_owned() }) ServiceModeButtonLabel TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    })
}

pub(super) fn format_service_mode(snapshot: &ServiceModeSnapshot) -> String {
    let platform = match snapshot.platform {
        ServiceModePlatform::WindowsService => "Windows Service",
        ServiceModePlatform::LinuxPolkit => "Linux Polkit",
        ServiceModePlatform::MacosLaunchd => "macOS launchd",
        ServiceModePlatform::Unsupported => "Unsupported host",
    };
    let state = match snapshot.state {
        ServiceModeState::Ready => "ready",
        ServiceModeState::InstalledStopped => "installed · stopped",
        ServiceModeState::NotInstalled => "not installed",
        ServiceModeState::MissingPrivilege => "missing privilege",
        ServiceModeState::Unavailable => "unavailable",
        ServiceModeState::Unsupported => "unsupported",
    };
    format!("{platform} · {state}")
}

pub(super) fn port_conflicts_row_scene(
    snapshot: &PortConflictSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_port_conflicts(snapshot);
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text({ "端口冲突 (Port Conflicts)".to_owned() }) TextRole(Role::Body) ),
                    ( Text(status) SettingsLine(SettingsLineKind::PortConflicts) TextRole(Role::Mono) ),
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
                PortConflictButton
                Button
                Children [
                    ( Text({ "检查并避让".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    })
}

pub(super) fn format_port_conflicts(snapshot: &PortConflictSnapshot) -> String {
    if snapshot.conflicts.is_empty() {
        return "未探测".to_owned();
    }
    snapshot
        .conflicts
        .iter()
        .map(|conflict| {
            let status = if conflict.available {
                "可用"
            } else {
                "占用"
            };
            let owner = conflict.owner_pid.map_or_else(
                || "owner=?".to_owned(),
                |pid| {
                    let name = conflict
                        .owner_name
                        .as_deref()
                        .map(infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text)
                        .unwrap_or_else(|| "unknown".to_owned());
                    format!("{name} pid={pid}")
                },
            );
            format!("{} {} ({status}, {owner})", conflict.binding.as_str(), conflict.port)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
