//! Telemetry and topology cards for the Overview page.
//!
//! Subtree scenes composing the topology chain, subscription quota,
//! active exit node card (BEVY-GAP-019), and system proxy / TUN master cards (BEVY-GAP-021).

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::pages::overview::{
    AccentContainerFill, AccentFill, BorderFill, SurfaceElevatedFill, mode_label,
};
use crate::pages::overview_restamp::{
    ActiveExitText, ActiveExitTextKind, OverviewMasterSwitchButton, OverviewMasterSwitchText,
    OverviewMasterSwitchTextKind, SubscriptionQuotaCard, SubscriptionQuotaProgress,
    SubscriptionQuotaText, SubscriptionQuotaTextKind, active_exit_text_value,
    master_switch_text_value, subscription_quota_text_value,
};
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_contract::subscription_quota::{
    SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
};
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot};

/// Marker on active exit node card (BEVY-GAP-019).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveExitNodeCard;

/// Marker on system proxy master switch card (BEVY-GAP-021).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemProxyMasterCard;

/// Marker on TUN master switch card (BEVY-GAP-021).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunMasterCard;

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
pub fn subscription_quota_scene(palette: &UiPalette) -> impl Scene + use<> {
    subscription_quota_scene_with_snapshot(&SubscriptionQuotaSnapshot::demo_fixture(), palette)
}

/// Subscription quota dashboard projected from the active profile snapshot.
pub fn subscription_quota_scene_with_snapshot(
    snapshot: &SubscriptionQuotaSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut quota_a11y = accesskit::Node::new(accesskit::Role::Region);
    quota_a11y.set_label("订阅配额");
    let profile = subscription_quota_text_value(snapshot, SubscriptionQuotaTextKind::Profile);
    let expiry = subscription_quota_text_value(snapshot, SubscriptionQuotaTextKind::Expiry);
    let metrics = subscription_quota_text_value(snapshot, SubscriptionQuotaTextKind::Metrics);
    let reset = subscription_quota_text_value(snapshot, SubscriptionQuotaTextKind::Reset);
    let status = subscription_quota_text_value(snapshot, SubscriptionQuotaTextKind::Status);
    let status_color = quota_status_color(snapshot.status, palette);
    let progress_percent = snapshot.usage_fraction() * 100.0;

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            template_value(AccessibilityNode(quota_a11y))
            SubscriptionQuotaCard
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        ( { icon_scene(IconId::FileText, 16.0, palette.accent) } ),
                        ( Text({ "订阅配额".to_owned() }) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        ( Text({ profile }) SubscriptionQuotaText(SubscriptionQuotaTextKind::Profile) TextRole(Role::Heading) ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ expiry }) SubscriptionQuotaText(SubscriptionQuotaTextKind::Expiry) TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                    }
                    Children [
                        ( Text({ metrics }) SubscriptionQuotaText(SubscriptionQuotaTextKind::Metrics) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        height: px(8.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        overflow: Overflow::clip(),
                    }
                    BackgroundColor({ palette.border })
                    BorderFill
                    Children [
                        (
                            Node {
                                width: percent(progress_percent),
                                height: percent(100),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                            }
                            BackgroundColor({ palette.accent })
                            AccentFill
                            SubscriptionQuotaProgress
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        ( Text({ reset }) SubscriptionQuotaText(SubscriptionQuotaTextKind::Reset) TextRole(Role::Caption) ),
                        ( Text({ status }) SubscriptionQuotaText(SubscriptionQuotaTextKind::Status) TextRole(Role::Caption) TextColor({ status_color }) ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

pub(crate) fn quota_status_color(status: SubscriptionQuotaStatus, palette: &UiPalette) -> Color {
    match status {
        SubscriptionQuotaStatus::Critical
        | SubscriptionQuotaStatus::Exhausted
        | SubscriptionQuotaStatus::Expired => palette.danger,
        SubscriptionQuotaStatus::Warning | SubscriptionQuotaStatus::ExpiringSoon => palette.warning,
        SubscriptionQuotaStatus::Ready => palette.success,
        SubscriptionQuotaStatus::Unknown
        | SubscriptionQuotaStatus::Empty
        | SubscriptionQuotaStatus::Unsupported
        | SubscriptionQuotaStatus::Failed => palette.ink_dim,
    }
}

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
pub fn active_exit_node_scene(palette: &UiPalette) -> impl Scene + use<> {
    active_exit_node_scene_with_snapshot(&ActiveExitSnapshot::demo_fixture(), palette)
}

/// Active exit node card with flag, protocol, latency and selected group.
/// Production callers pass the application-owned snapshot.
pub fn active_exit_node_scene_with_snapshot(
    snapshot: &ActiveExitSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut a11y = accesskit::Node::new(accesskit::Role::Region);
    a11y.set_label("当前主出口节点");
    let delay_color = if snapshot.delay_ms.is_some() {
        palette.success
    } else {
        palette.ink_dim
    };
    let flag = active_exit_text_value(snapshot, ActiveExitTextKind::Flag);
    let name = active_exit_text_value(snapshot, ActiveExitTextKind::Name);
    let protocol = active_exit_text_value(snapshot, ActiveExitTextKind::Protocol);
    let delay = active_exit_text_value(snapshot, ActiveExitTextKind::Delay);
    let group = active_exit_text_value(snapshot, ActiveExitTextKind::Group);
    let status = active_exit_text_value(snapshot, ActiveExitTextKind::Status);

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            template_value(AccessibilityNode(a11y))
            ActiveExitNodeCard
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(IconId::Globe, 16.0, palette.accent) } ),
                                ( Text({ "当前主出口节点 (Active Exit Node)".to_owned() }) TextRole(Role::Heading) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ delay }) ActiveExitText(ActiveExitTextKind::Delay) TextRole(Role::Caption) TextColor({ delay_color }) ),
                            ]
                        ),
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
                    SurfaceElevatedFill
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( Text({ flag }) ActiveExitText(ActiveExitTextKind::Flag) TextRole(Role::BodyStrong) ),
                                ( Text({ name }) ActiveExitText(ActiveExitTextKind::Name) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S6),
                            }
                            Children [
                                ( Text({ protocol }) ActiveExitText(ActiveExitTextKind::Protocol) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                ( Text({ group }) ActiveExitText(ActiveExitTextKind::Group) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                    }
                    Children [
                        ( Text({ status }) ActiveExitText(ActiveExitTextKind::Status) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
pub fn master_switches_scene(palette: &UiPalette) -> impl Scene + use<> {
    master_switches_scene_with_snapshot(
        &SystemToggleSnapshot::from_legacy(true, Some(false), 1),
        palette,
    )
}

/// Dual system proxy and TUN master switch cards projected from the shared
/// toggle snapshot. Their action buttons are wired by the Overview observer.
pub fn master_switches_scene_with_snapshot(
    snapshot: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(space::S12),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(space::S8),
        }
        Children [
            (
                { single_master_card_scene("系统代理 (System Proxy)", "接管系统 HTTP/SOCKS 端口", IconId::Settings, SystemToggle::SystemProxy, snapshot, palette) }
                SystemProxyMasterCard
            ),
            (
                { single_master_card_scene("TUN 模式 (TUN Virtual Interface)", "gVisor 虚拟网卡全量接管", IconId::Network, SystemToggle::Tun, snapshot, palette) }
                TunMasterCard
            ),
        ]
    }
}

fn single_master_card_scene(
    title: &'static str,
    desc: &'static str,
    icon: IconId,
    toggle: SystemToggle,
    snapshot: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let state = snapshot.state(toggle);
    let enabled = state.is_enabled();
    let can_toggle = state.can_toggle();
    let status_text =
        master_switch_text_value(snapshot, toggle, OverviewMasterSwitchTextKind::Status);
    let action_text =
        master_switch_text_value(snapshot, toggle, OverviewMasterSwitchTextKind::Action);
    let status_color =
        crate::pages::overview_restamp::master_switch_status_color(snapshot, toggle, palette);
    let dot_color = if enabled {
        palette.success
    } else {
        palette.border
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: px(280.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(icon, 16.0, palette.accent) } ),
                                ( Text({ title.to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S4),
                            }
                            Children [
                                (
                                    Node {
                                        width: px(6.0),
                                        height: px(6.0),
                                        border_radius: BorderRadius::all(Val::Px(3.0)),
                                    }
                                    BackgroundColor({ dot_color })
                                ),
                                ( Text({ status_text }) OverviewMasterSwitchText { toggle, kind: OverviewMasterSwitchTextKind::Status } TextRole(Role::Caption) TextColor({ status_color }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        ( Text({ desc.to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            OverviewMasterSwitchButton { toggle, enabled, can_toggle }
                            Button
                            BackgroundColor({ palette.accent_container })
                            Children [
                                ( Text({ action_text }) OverviewMasterSwitchText { toggle, kind: OverviewMasterSwitchTextKind::Action } TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Marker on the proxy mode segmented controller card (BEVY-GAP-022).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxyModeSegmentCard;

/// Marker on mode segment pills in the Overview card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewModeSegmentPill(pub ProxyMode);

/// Marker on mode segment text in the Overview card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewModeSegmentText(pub ProxyMode);

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
pub fn mode_segmented_controller_scene(palette: &UiPalette) -> impl Scene + use<> {
    mode_segmented_controller_scene_with_snapshot(&ProxyModeSnapshot::demo_fixture(), palette)
}

/// The production 4-segment proxy mode controller card (DUAL-03-08).
pub fn mode_segmented_controller_scene_with_snapshot(
    snapshot: &ProxyModeSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Region);
    header_a11y.set_label("代理运行模式");

    let status_str = match snapshot.status {
        ProxyModeStatus::Ready => "就绪",
        ProxyModeStatus::Pending => "切换中",
        ProxyModeStatus::Unsupported => "不可用",
        ProxyModeStatus::Failed => "失败",
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S12),
            }
            template_value(AccessibilityNode(header_a11y))
            ProxyModeSegmentCard
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(IconId::Settings, 16.0, palette.accent) } ),
                                ( Text({ "代理运行模式 (Proxy Mode)".to_owned() }) TextRole(Role::Heading) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ format!("{} · {}", mode_label(snapshot.current), status_str) }) TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(space::S8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        ( { single_mode_pill_scene(ProxyMode::Rule, snapshot, palette) } ),
                        ( { single_mode_pill_scene(ProxyMode::Global, snapshot, palette) } ),
                        ( { single_mode_pill_scene(ProxyMode::Direct, snapshot, palette) } ),
                        ( { single_mode_pill_scene(ProxyMode::Script, snapshot, palette) } ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn single_mode_pill_scene(
    mode: ProxyMode,
    snapshot: &ProxyModeSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let is_current = snapshot.current == mode;
    let selectable = snapshot.is_mode_selectable(mode);
    let bg = if is_current {
        palette.accent
    } else if selectable {
        palette.accent_container
    } else {
        palette.surface_elevated
    };
    let ink = if is_current {
        palette.on_accent
    } else if selectable {
        palette.accent
    } else {
        palette.ink_dim
    };
    let label = mode_label(mode).to_owned();

    bsn! {
        Node {
            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S6)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        OverviewModeSegmentPill(mode)
        Button
        BackgroundColor({ bg })
        Children [
            ( Text({ label }) OverviewModeSegmentText(mode) TextRole(Role::Body) TextColor({ ink }) )
        ]
    }
}
