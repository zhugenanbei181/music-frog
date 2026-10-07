//! Bevy Settings scene pieces for local core-version operations.
//!
//! Keeping the rollback control beside its projection-specific scene keeps
//! the main Settings page below the source-size budget without moving any
//! business decision into the widget layer.

use super::settings_runtime::{RuntimeField, RuntimePolicy};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, FlexWrap, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ValueChange};
use infiltrator_application::settings_status_projection::{
    format_controller_auth, format_core_resources, format_mtu, format_port_conflicts,
    format_rollback_target, format_runtime_status, format_service_mode, optional_copy,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::lan::LanSecuritySnapshot;
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::service_mode::{ServiceModeSnapshot, ServiceModeState};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use infiltrator_contract::tun::TunStack;
use infiltrator_contract::version::{CoreArtifactVerification, CoreVersionSnapshot};
use infiltrator_contract::vpn::VpnSessionSnapshot;

/// Marker for text lines updated by the Settings projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsLine(pub SettingsLineKind);

/// Different text lines on the settings page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsLineKind {
    /// Overview summary.
    #[default]
    Summary,
    RuntimeStatus,
    /// Local-only boot preflight and optional remote-dependency status.
    OfflineStartup,
    /// Mixed port text.
    MixedPort,
    /// Mihomo Allow-LAN bind address.
    LanBindAddress,
    /// Mihomo LAN ACL/authentication status.
    LanSecurity,
    /// Mihomo top-level IPv6 routing policy.
    Ipv6Routing,
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
    pub close_to_tray: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub preference_status: PageStatus,
    pub runtime_status: RuntimeControlStatus,
    pub autostart: bool,
    pub system_proxy: bool,
    pub system_proxy_snapshot: SystemProxySnapshot,
    pub system_proxy_recovery: SystemProxyRecoverySnapshot,
    pub mixed_port: Option<u16>,
    pub allow_lan: Option<bool>,
    pub lan_bind_address: Option<String>,
    pub lan_security: Option<LanSecuritySnapshot>,
    pub ipv6_routing: Option<Ipv6RoutingSnapshot>,
    pub pac: PacSnapshot,
    pub network_roaming: NetworkRoamingSnapshot,
    pub vpn: VpnSessionSnapshot,
    pub privileged_network: PrivilegedNetworkSnapshot,
    pub tun_enabled: Option<bool>,
    pub tun_stack: Option<String>,
    pub tun_auto_route: Option<bool>,
    pub tun_strict_route: Option<bool>,
    pub controller_port: Option<u16>,
    pub log_level: Option<String>,
    pub core_channel: String,
    pub core_versions: CoreVersionSnapshot,
    pub core_integrity: CoreArtifactVerification,
    pub controller_auth: ControllerAuthSnapshot,
    pub service_mode: ServiceModeSnapshot,
    pub port_conflicts: PortConflictSnapshot,
    pub core_resources: CoreResourceSnapshot,
    pub offline_startup: OfflineStartupSnapshot,
    pub mtu: MtuNegotiationSnapshot,
    /// DUAL-15-04: the persisted Mini HUD placement from the shared settings
    /// snapshot.
    pub mini_hud: MiniHudPlacement,
}

pub(super) fn controller_settings_card(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let ctrl_port_str = projection
        .controller_port
        .map(|port| format!("127.0.0.1:{port}"))
        .unwrap_or_else(|| optional_copy::<u16>(None, UiLocale::default().code()));
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("settings_controller_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::all(Val::Px(space::S8)),
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    LocalizedText::plain("settings_controller_port_label") TextRole(Role::Body)
                                    --
                                    Text(ctrl_port_str) SettingsLine(SettingsLineKind::ControllerPort) TextRole(Role::Mono)
                                ]
                                --
                                @{ core_log_level_row_scene(projection, palette) }
                                --
                                Text({ format_runtime_status(&projection.runtime_status, UiLocale::default().code()) })
                                SettingsLine(SettingsLineKind::RuntimeStatus) TextRole(Role::Caption)
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
    let rollback_text =
        format_rollback_target(&projection.core_versions, UiLocale::default().code());
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
                Text({ LocalizedText::plain(if rollback_available { "core_rollback_action" } else { "common_unavailable" }).render(&UiLocale::default()) }) CoreRollbackButtonLabel TextRole(Role::BodyStrong)
            ]
    });

    let label: Box<dyn Scene> = Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                LocalizedText::plain("settings_core_rollback_title") TextRole(Role::Body)
                --
                Text(rollback_text) SettingsLine(SettingsLineKind::CoreRollback) TextRole(Role::Mono)
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
            Children [
                { children }
            ]
    })
}

pub(super) fn controller_auth_row_scene(
    snapshot: &ControllerAuthSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_controller_auth(snapshot, UiLocale::default().code());
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
                LocalizedText::plain("settings_controller_auth_title") TextRole(Role::Body)
                --
                Text(status) SettingsLine(SettingsLineKind::ControllerAuth) TextRole(Role::Mono)
            ]
    })
}

pub(super) fn core_log_level_row_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let active = projection
        .log_level
        .as_deref()
        .and_then(CoreLogLevel::parse);
    let buttons: Vec<Box<dyn Scene>> = CoreLogLevel::ALL
        .into_iter()
        .map(|level| {
            Box::new(core_log_level_button_scene(level, active, palette)) as Box<dyn Scene>
        })
        .collect();
    let current = active.map_or_else(
        || optional_copy(projection.log_level.as_ref(), UiLocale::default().code()),
        |level| level.as_str().to_uppercase(),
    );
    let label: Box<dyn Scene> = Box::new(bsn! {
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                LocalizedText::plain("settings_core_log_level_label") TextRole(Role::Body)
                --
                LocalizedText::new("settings_log_level_value", vec![("level", current)]) SettingsLine(SettingsLineKind::LogLevel) TextRole(Role::Mono)
            ]
    });
    let controls: Box<dyn Scene> = Box::new(bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S4),
            }
            Children [
                { buttons }
            ]
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
            Children [
                { children }
            ]
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
                Text({ level.as_str().to_uppercase() }) TextRole(Role::Caption)
            ]
    }
}

pub(super) fn tun_stack_selector_scene(
    projection: &SettingsProjection,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let active = projection.tun_stack.as_deref().and_then(TunStack::parse);
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
            Children [
                { buttons }
            ]
    });
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S4),
            }
            Children [
                LocalizedText::plain("settings_tun_protocol_label") TextRole(Role::Body)
                --
                @{ controls }
            ]
    })
}

pub(super) fn mtu_row_scene(
    snapshot: &MtuNegotiationSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_mtu(snapshot, UiLocale::default().code());
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
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    LocalizedText::plain("settings_mtu_title") TextRole(Role::Body)
                    --
                    Text(status) SettingsLine(SettingsLineKind::Mtu) TextRole(Role::Mono)
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
                ProbeTunMtuButton
                Button
                Children [
                    LocalizedText::plain("settings_mtu_probe_action") TextRole(Role::BodyStrong)
                ]
            ]
    })
}

pub(super) fn tun_route_toggle_scene(
    kind: TunRouteToggleKind,
    label: &'static str,
    checked: Option<bool>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::horizontal(Val::Px(space::S4)),
            }
            TunRouteToggle(kind)
            Children [
                @{ localized_checkbox_scene(LocalizedText::plain(label), checked == Some(true), palette) }
                ButtonDisabled({ checked.is_none() })
            ]
    })
}

pub(super) fn tun_enable_toggle_scene(
    checked: Option<bool>,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::horizontal(Val::Px(space::S4)),
            }
            TunEnableToggle
            Children [
                @{ localized_checkbox_scene(LocalizedText::plain("tun_enable_device"), checked == Some(true), palette) }
                ButtonDisabled({ checked.is_none() })
            ]
    })
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
    policy: RuntimePolicy,
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
    let field = match toggle.0 {
        TunRouteToggleKind::AutoRoute => RuntimeField::AutoRoute,
        TunRouteToggleKind::StrictRoute => RuntimeField::StrictRoute,
    };
    if !policy.available(field) {
        return;
    }
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
    policy: RuntimePolicy,
) {
    if !policy.available(RuntimeField::Tun) {
        return;
    }
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

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TunStackLabel(pub TunStack);

fn tun_stack_button_scene(
    stack: TunStack,
    active: Option<TunStack>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let available = stack.is_live_supported();
    let label = if available {
        stack.label().to_owned()
    } else {
        UiLocale::default().text("settings_lwip_reference")
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
                Text(label) TextRole(Role::Caption) TunStackLabel({ stack })
            ]
    }
}

pub(super) fn service_mode_row_scene(
    snapshot: &ServiceModeSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let ready = snapshot.state == ServiceModeState::Ready;
    let status = format_service_mode(snapshot, UiLocale::default().code());
    let label = if ready {
        "core_service_ready"
    } else {
        "core_prepare_service_action"
    };
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
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    LocalizedText::plain("settings_service_mode_title") TextRole(Role::Body)
                    --
                    Text(status) SettingsLine(SettingsLineKind::ServiceMode) TextRole(Role::Mono)
                ]
                --
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
                    Text({ LocalizedText::plain(label).render(&UiLocale::default()) }) ServiceModeButtonLabel TextRole(Role::BodyStrong)
                ]
            ]
    })
}

pub(super) fn port_conflicts_row_scene(
    snapshot: &PortConflictSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_port_conflicts(snapshot, UiLocale::default().code());
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
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    LocalizedText::plain("settings_port_conflict_title") TextRole(Role::Body)
                    --
                    Text(status) SettingsLine(SettingsLineKind::PortConflicts) TextRole(Role::Mono)
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
                PortConflictButton
                Button
                Children [
                    LocalizedText::plain("settings_port_conflict_probe_action") TextRole(Role::BodyStrong)
                ]
            ]
    })
}

pub(super) fn core_resources_row_scene(
    snapshot: &CoreResourceSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_core_resources(snapshot, UiLocale::default().code());
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
                LocalizedText::plain("settings_core_resources_title") TextRole(Role::Body)
                --
                Text(status) SettingsLine(SettingsLineKind::CoreResources) TextRole(Role::Mono)
            ]
    })
}
