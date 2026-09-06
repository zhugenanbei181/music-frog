//! Bevy Settings scene pieces for local core-version operations.
//!
//! Keeping the rollback control beside its projection-specific scene keeps
//! the main Settings page below the source-size budget without moving any
//! business decision into the widget layer.

use bevy::scene::{Scene, bsn};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, FlexWrap, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ValueChange};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
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
use infiltrator_contract::resources::{CoreGcStatus, CoreResourceSnapshot};
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, OfflineStartupState, StartupRemoteDependency,
};
use infiltrator_contract::tun::TunStack;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot;
use infiltrator_contract::system_proxy::SystemProxySnapshot;
use crate::command::{CommandSinkHandle, UiCommand};

/// Marker for text lines updated by the Settings projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsLine(pub SettingsLineKind);

/// Different text lines on the settings page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsLineKind {
    /// Overview summary.
    #[default]
    Summary,
    /// Local-only boot preflight and optional remote-dependency status.
    OfflineStartup,
    /// Mixed port text.
    MixedPort,
    /// TUN stack text.
    TunStack,
    /// Controller port text.
    ControllerPort,
    /// Log level text.
    LogLevel,
    /// Selected core release channel.
    CoreChannel,
    /// Latest result of the three-channel probe.
    CoreVersions,
    /// Latest archive-integrity result.
    CoreIntegrity,
    /// Locally available core rollback target.
    CoreRollback,
    /// Controller secret/header injection status.
    ControllerAuth,
    /// Host system proxy ownership and reconciliation status.
    SystemProxy,
    /// Host-owned privileged service mode status.
    ServiceMode,
    /// Mixed/controller port conflict observation.
    PortConflicts,
    /// Core memory/CPU and automatic GC state.
    CoreResources,
    /// Physical-link to TUN MTU negotiation state.
    Mtu,
}

/// Marker carrying the live Mihomo log-level choice.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreLogLevelButton {
    pub level: CoreLogLevel,
}

/// Marker carrying the shared TUN stack choice.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunStackButton {
    pub stack: TunStack,
}

/// Live availability for a TUN stack control; current LWIP stays disabled.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunStackButtonAvailability(pub bool);

/// Marker for the physical-link MTU probe action.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeTunMtuButton;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TunRouteToggleKind {
    #[default]
    AutoRoute,
    StrictRoute,
}

/// Parent marker for a route checkbox. The official checkbox child remains
/// responsible for focus and ValueChange semantics; this marker identifies
/// which shared command the parent row represents.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunRouteToggle(pub TunRouteToggleKind);

/// Parent marker for the TUN ingress checkbox.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunEnableToggle;

/// Snapshot of the shared Settings domain used by both scene and observer.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsProjection {
    pub autostart: bool,
    pub system_proxy: bool,
    pub system_proxy_snapshot: SystemProxySnapshot,
    pub system_proxy_recovery: SystemProxyRecoverySnapshot,
    pub mixed_port: u16,
    pub allow_lan: bool,
    pub tun_enabled: bool,
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub controller_port: u16,
    pub log_level: String,
    pub core_channel: String,
    pub core_versions: CoreVersionSnapshot,
    pub core_integrity: CoreArtifactVerification,
    pub controller_auth: ControllerAuthSnapshot,
    pub service_mode: ServiceModeSnapshot,
    pub port_conflicts: PortConflictSnapshot,
    pub core_resources: CoreResourceSnapshot,
    pub offline_startup: OfflineStartupSnapshot,
    pub mtu: MtuNegotiationSnapshot,
}

impl SettingsProjection {
    pub fn demo() -> Self {
        Self {
            autostart: true,
            system_proxy: true,
            system_proxy_snapshot: SystemProxySnapshot::default(),
            system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
            mixed_port: 7890,
            allow_lan: false,
            tun_enabled: true,
            tun_stack: "gVisor (高性能用户态协议栈)".to_owned(),
            tun_auto_route: true,
            tun_strict_route: false,
            controller_port: 9090,
            log_level: "info".to_owned(),
            core_channel: "stable".to_owned(),
            core_versions: CoreVersionSnapshot::default(),
            core_integrity: Default::default(),
            controller_auth: Default::default(),
            service_mode: Default::default(),
            port_conflicts: Default::default(),
            core_resources: Default::default(),
            offline_startup: Default::default(),
            mtu: Default::default(),
        }
    }
}

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
    CoreRollbackAvailability, CoreRollbackButton, CoreRollbackButtonLabel, PortConflictButton,
    ServiceModeAvailability, ServiceModeButton, ServiceModeButtonLabel,
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

pub(super) fn format_rollback_target(snapshot: &CoreVersionSnapshot) -> String {
    snapshot
        .rollback
        .target
        .as_deref()
        .map_or_else(|| "没有可回滚的本地内核".to_owned(), |version| {
            format!("可回滚至 {version}")
        })
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

pub(super) fn tun_stack_selector_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let active = TunStack::parse(&projection.tun_stack);
    let buttons: Vec<Box<dyn Scene>> = TunStack::ALL
        .into_iter()
        .map(|stack| Box::new(tun_stack_button_scene(stack, active, palette)) as Box<dyn Scene>)
        .collect();
    let controls: Box<dyn Scene> = Box::new(bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(space::S4),
        }
        Children [ { buttons } ]
    });
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            ( Text({ "TUN 协议栈 (Protocol Stack)".to_owned() }) TextRole(Role::Body) ),
            ( { controls } ),
        ]
    })
}

pub(super) fn mtu_row_scene(
    snapshot: &MtuNegotiationSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_mtu(snapshot);
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
                    ( Text({ "物理/虚拟网卡 MTU (MTU Negotiation)".to_owned() }) TextRole(Role::Body) ),
                    ( Text(status) SettingsLine(SettingsLineKind::Mtu) TextRole(Role::Mono) ),
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
                ProbeTunMtuButton
                Button
                Children [
                    ( Text({ "探测并协商".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    })
}

pub(super) fn tun_route_toggle_scene(
    kind: TunRouteToggleKind,
    label: &str,
    checked: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        TunRouteToggle(kind)
        Children [
            ( { checkbox_scene(label.to_owned(), checked, palette) } ),
        ]
    })
}

pub(super) fn tun_enable_toggle_scene(checked: bool, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
            padding: UiRect::horizontal(Val::Px(space::S4)),
        }
        TunEnableToggle
        Children [
            ( { checkbox_scene("启用 TUN 虚拟网卡接管 (Enable TUN Device)".to_owned(), checked, palette) } ),
        ]
    })
}

pub(super) fn format_mtu(snapshot: &MtuNegotiationSnapshot) -> String {
    match &snapshot.state {
        MtuProbeState::Unknown => "未探测".to_owned(),
        MtuProbeState::Probing => "探测中".to_owned(),
        MtuProbeState::Ready => format!(
            "{}: physical={} → TUN={} · MSS={} · overhead={} · applied={}",
            snapshot
                .physical_interface
                .as_deref()
                .unwrap_or("active-link"),
            snapshot.physical_mtu.unwrap_or_default(),
            snapshot.tun_mtu.unwrap_or_default(),
            snapshot.tcp_mss.unwrap_or_default(),
            snapshot.overhead_bytes,
            snapshot
                .applied_tun_mtu
                .map(|value| value.to_string())
                .unwrap_or_else(|| "pending".to_owned())
        ),
        MtuProbeState::Unsupported => "宿主不支持".to_owned(),
        MtuProbeState::Failed { failure } => format!(
            "失败 ({})",
            infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text(&failure.message)
        ),
    }
}

pub(super) fn on_mtu_probe_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ProbeTunMtuButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if buttons.contains(activate.entity)
        && let Some(handle) = handle
    {
        handle.submit(UiCommand::ProbeTunMtu);
    }
}

pub(super) fn on_tun_route_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<&TunRouteToggle>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    let Ok(toggle) = toggles.get(parent.0) else {
        return;
    };
    let command = match toggle.0 {
        TunRouteToggleKind::AutoRoute => UiCommand::SetTunAutoRoute(change.value),
        TunRouteToggleKind::StrictRoute => UiCommand::SetTunStrictRoute(change.value),
    };
    handle.submit(command);
}

pub(super) fn on_tun_enabled_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<TunEnableToggle>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    if toggles.get(parent.0).is_ok() {
        handle.submit(UiCommand::ToggleTun {
            enabled: change.value,
        });
    }
}

fn tun_stack_button_scene(
    stack: TunStack,
    active: Option<TunStack>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let available = stack.is_live_supported();
    let label = if available {
        stack.label().to_owned()
    } else {
        "LWIP (参考 / Reference-only)".to_owned()
    };
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ if available && active == Some(stack) { palette.accent } else { palette.surface_elevated } })
        TunStackButton { stack }
        TunStackButtonAvailability(available)
        Button
        Children [
            ( Text(label) TextRole(Role::Caption) ),
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

pub(super) fn core_resources_row_scene(
    snapshot: &CoreResourceSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_core_resources(snapshot);
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
            ( Text({ "内核资源 (Core Resources)".to_owned() }) TextRole(Role::Body) ),
            ( Text(status) SettingsLine(SettingsLineKind::CoreResources) TextRole(Role::Mono) ),
        ]
    })
}

pub(super) fn offline_startup_row_scene(
    snapshot: &OfflineStartupSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_offline_startup(snapshot);
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
            ( Text({ "离线启动 (Offline-first)".to_owned() }) TextRole(Role::Body) ),
            ( Text(status) SettingsLine(SettingsLineKind::OfflineStartup) TextRole(Role::Mono) ),
        ]
    })
}

pub(super) fn format_offline_startup(snapshot: &OfflineStartupSnapshot) -> String {
    let state = match snapshot.state {
        OfflineStartupState::Unknown => "未探测",
        OfflineStartupState::Checking => "校验中",
        OfflineStartupState::Ready => "可离线启动",
        OfflineStartupState::Degraded => "可启动但已降级",
        OfflineStartupState::Blocked => "已阻断",
    };
    let config = if snapshot.config_valid { "有效" } else { "无效" };
    let binary = if snapshot.binary_available {
        "可用"
    } else {
        "缺失"
    };
    let geoip = match snapshot.geoip {
        LocalAssetStatus::NotRequired => "不需要",
        LocalAssetStatus::Available => "本地可用",
        LocalAssetStatus::Missing => "缺失",
    };
    let remote = match snapshot.remote_dependency {
        StartupRemoteDependency::Optional => "远端可选",
    };
    let failure = snapshot.failure.as_ref().map_or_else(String::new, |failure| {
        format!(
            " · {}",
            infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text(&failure.message)
        )
    });
    format!("离线优先 · {state} · 配置={config} · 内核={binary} · GeoIP={geoip} · {remote}{failure}")
}

pub(super) fn format_core_resources(snapshot: &CoreResourceSnapshot) -> String {
    let memory = snapshot.memory_bytes.map_or_else(
        || "内存=?".to_owned(),
        |bytes| format!("内存={:.1} MiB", bytes as f64 / 1_048_576.0),
    );
    let cpu = snapshot
        .cpu_percent
        .map_or_else(|| "CPU=?".to_owned(), |percent| format!("CPU={percent:.1}%"));
    let gc = match &snapshot.gc {
        CoreGcStatus::Unknown => "GC=未采样".to_owned(),
        CoreGcStatus::NotNeeded => "GC=无需执行".to_owned(),
        CoreGcStatus::Triggered { after_bytes, .. } => after_bytes.map_or_else(
            || "GC=已触发".to_owned(),
            |bytes| format!("GC=已触发，之后={:.1} MiB", bytes as f64 / 1_048_576.0),
        ),
        CoreGcStatus::Failed { failure } => format!(
            "GC=失败 ({})",
            infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text(&failure.message)
        ),
        CoreGcStatus::Unsupported => "GC=不支持".to_owned(),
    };
    format!("{memory} · {cpu} · 上限=512 MiB · {gc}")
}
