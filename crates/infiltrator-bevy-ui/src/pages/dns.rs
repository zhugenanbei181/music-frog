//! The DNS page (域名解析): upstream nameservers, DoH / DoT / DoQ endpoints,
//! Fake-IP filter rules, and DNS cache status.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DnsLine`],
//! [`DnsServerAddress`], [`DnsServerProto`], [`DnsServerLatency`]).
//! The page self-registers [`apply_dns_projection`] and action observers
//! once per world via [`DnsPageRoot`]. When [`DnsProjectionUpdated`] fires,
//! texts, latency inks, and servers restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::tabs::{SegmentedControlValue, segmented_control_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::proxies::{format_latency, latency_color};
use crate::route::{PageRoot, Route};

/// Root marker on the DNS page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_dns_page)]
pub struct DnsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct DnsPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsLine(pub DnsLineKind);

/// Different text lines on the DNS page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsLineKind {
    /// Overview summary: DNS mode and cache count.
    #[default]
    Summary,
    /// Fake-IP range display.
    FakeIpRange,
}

/// Marker for a DNS server's address display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerAddress(pub usize);

/// Marker for a DNS server's protocol display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerProto(pub usize);

/// Marker for a DNS server's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerLatency(pub usize);

/// Marker for the "Clear DNS Cache" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearDnsCacheButton;

/// Marker for the "Test DNS Latency" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDnsLatencyButton;

/// Marker component for the DNS 6-switch form card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsConfigCard;

/// Marker for individual DNS form switch identifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsSwitchKind {
    /// 启用 DNS 服务 (enable)
    #[default]
    Enable,
    /// IPv6 解析 (ipv6)
    Ipv6,
    /// DNS 内存缓存 (cache)
    Cache,
    /// 遵循系统 Hosts (use_hosts)
    UseHosts,
    /// 系统默认解析器 (use_system_hosts)
    UseSystemHosts,
    /// 分流规则优先 (respect_rules)
    RespectRules,
}

/// Marker component on a DNS switch button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsSwitchButton(pub DnsSwitchKind);

/// Marker component for Domain Mapping Mode segmented control root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEnhancedModeControl;

/// Marker component for Filter Mode segmented control root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFilterModeControl;

/// DNS resolution mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsMode {
    #[default]
    FakeIp,
    RedirHost,
}

impl DnsMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FakeIp => "Fake-IP 模式 (增强隐私与速度)",
            Self::RedirHost => "Redir-Host 模式 (真实 IP 解析)",
        }
    }
}

/// A DNS upstream nameserver item.
#[derive(Clone, Debug, PartialEq)]
pub struct DnsServerItem {
    pub address: String,
    pub protocol: String,
    pub latency_ms: Option<u32>,
    pub is_fallback: bool,
}

/// Snapshot of the DNS domain.
#[derive(Clone, Debug, PartialEq)]
pub struct DnsProjection {
    pub mode: DnsMode,
    pub cache_entries: usize,
    pub fake_ip_range: String,
    pub servers: Vec<DnsServerItem>,
}

impl DnsProjection {
    /// Believable demo fixture for the DNS page.
    pub fn demo() -> Self {
        Self {
            mode: DnsMode::FakeIp,
            cache_entries: 342,
            fake_ip_range: "198.18.0.1/16".to_owned(),
            servers: vec![
                DnsServerItem {
                    address: "https://1.1.1.1/dns-query".to_owned(),
                    protocol: "DoH (HTTPS)".to_owned(),
                    latency_ms: Some(28),
                    is_fallback: false,
                },
                DnsServerItem {
                    address: "tls://8.8.8.8:853".to_owned(),
                    protocol: "DoT (TLS)".to_owned(),
                    latency_ms: Some(45),
                    is_fallback: false,
                },
                DnsServerItem {
                    address: "https://dns.alidns.com/dns-query".to_owned(),
                    protocol: "DoH (Domestic)".to_owned(),
                    latency_ms: Some(18),
                    is_fallback: false,
                },
                DnsServerItem {
                    address: "https://cloudflare-dns.com/dns-query".to_owned(),
                    protocol: "DoH (Fallback)".to_owned(),
                    latency_ms: Some(35),
                    is_fallback: true,
                },
            ],
        }
    }
}

/// The typed event dispatched when DNS data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DnsProjectionUpdated(pub DnsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastDnsProjection(pub Option<DnsProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn dns_page(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "域名解析 · {} (缓存条目: {})",
        projection.mode.label(),
        projection.cache_entries
    );

    let server_scenes: Vec<Box<dyn Scene>> = projection
        .servers
        .iter()
        .enumerate()
        .map(|(idx, s)| Box::new(server_row_scene(idx, s, palette)) as Box<dyn Scene>)
        .collect();

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
        PageRoot(Route::Dns)
        DnsPageRoot
        Children [
            ( { header_card_scene(summary, palette) } ),
            ( { dns_form_card_scene(projection.mode, palette) } ),
            ( { servers_card_scene(server_scenes, palette) } ),
            ( { fake_ip_card_scene(&projection.fake_ip_range, palette) } ),
        ]
    }
}

fn header_card_scene(summary: String, palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("DNS 解析概览");

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
                        ( { icon_tile_scene(IconId::Network, 36.0, palette) } ),
                        ( Text(summary) DnsLine(DnsLineKind::Summary) TextRole(Role::Heading) ),
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
                            TestDnsLatencyButton
                            Button
                            Children [
                                ( Text({ "测速".to_owned() }) TextRole(Role::BodyStrong) ),
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
                            BackgroundColor({ palette.surface_elevated })
                            ClearDnsCacheButton
                            Button
                            Children [
                                ( Text({ "清空 DNS 缓存".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn dns_switch_row_scene(
    name: &'static str,
    label: &'static str,
    kind: DnsSwitchKind,
    enabled: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let text_str = format!("{name} ({label})");
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
                        DnsSwitchButton(kind)
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

fn dns_form_card_scene(mode: DnsMode, palette: &UiPalette) -> impl Scene + use<> {
    let mode_idx = match mode {
        DnsMode::FakeIp => 0,
        DnsMode::RedirHost => 1,
    };

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                DnsConfigCard
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                            ( Text({ "DNS 核心配置 (DNS Configuration)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    ( Text({ "6 项开关 · 域名映射 · 过滤模式".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                }
                Children [
                    ( { dns_switch_row_scene("enable", "启用 DNS 服务", DnsSwitchKind::Enable, true, palette) } ),
                    ( { dns_switch_row_scene("ipv6", "IPv6 解析", DnsSwitchKind::Ipv6, true, palette) } ),
                    ( { dns_switch_row_scene("cache", "DNS 内存缓存", DnsSwitchKind::Cache, true, palette) } ),
                    ( { dns_switch_row_scene("use_hosts", "遵循系统 Hosts", DnsSwitchKind::UseHosts, true, palette) } ),
                    ( { dns_switch_row_scene("use_system_hosts", "系统默认解析器", DnsSwitchKind::UseSystemHosts, true, palette) } ),
                    ( { dns_switch_row_scene("respect_rules", "分流规则优先", DnsSwitchKind::RespectRules, false, palette) } ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::top(Val::Px(space::S6)),
                }
                DnsEnhancedModeControl
                Children [
                    ( Text({ "域名映射模式 (enhanced_mode)".to_owned() }) TextRole(Role::Caption) ),
                    ( { segmented_control_scene(
                        vec![
                            "虚拟 IP (Fake-IP)".to_owned(),
                            "真实 IP (Redir-Host)".to_owned(),
                            "取消映射 (None)".to_owned(),
                        ],
                        mode_idx,
                        palette,
                    ) } ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                DnsFilterModeControl
                Children [
                    ( Text({ "过滤模式 (fake_ip_filter_mode)".to_owned() }) TextRole(Role::Caption) ),
                    ( { segmented_control_scene(
                        vec![
                            "黑名单 (Blacklist)".to_owned(),
                            "白名单 (Whitelist)".to_owned(),
                            "规则 (Rules)".to_owned(),
                        ],
                        0,
                        palette,
                    ) } ),
                ]
            }),
        ],
        palette,
    )
}

fn servers_card_scene(
    server_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "上游加密 DNS 服务器 (Nameservers)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "支持 DoH / DoT / DoQ".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { server_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn server_row_scene(idx: usize, server: &DnsServerItem, palette: &UiPalette) -> impl Scene + use<> {
    let addr = server.address.clone();
    let proto = server.protocol.clone();
    let fallback_badge = if server.is_fallback {
        " [Fallback]"
    } else {
        ""
    };
    let proto_str = format!("{proto}{fallback_badge}");
    let (lat_str, tier) = format_latency(server.latency_ms);
    let lat_col = latency_color(tier, palette);

    bsn! {
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
                    ( Text(addr) DnsServerAddress(idx) TextRole(Role::BodyStrong) ),
                    ( Text(proto_str) DnsServerProto(idx) TextRole(Role::Caption) ),
                ]
            ),
            (
                Text(lat_str)
                DnsServerLatency(idx)
                TextRole(Role::Mono)
                TextColor(lat_col)
            ),
        ]
    }
}

fn fake_ip_card_scene(fake_ip_range: &str, palette: &UiPalette) -> impl Scene + use<> {
    let range_str = format!("分配网段: {fake_ip_range}");

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "Fake-IP 高级设置 (Fake-IP Filter & Pool)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
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
                    ( Text(range_str) DnsLine(DnsLineKind::FakeIpRange) TextRole(Role::Body) ),
                    ( Text({ "过滤域名: *.lan, localhost".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_dns_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<DnsPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(DnsPageBound);
    commands.add_observer(apply_dns_projection);
    commands.add_observer(on_dns_action_activated);
}

pub(crate) fn on_dns_action_activated(
    activate: On<Activate>,
    clear_buttons: Query<(), With<ClearDnsCacheButton>>,
    test_buttons: Query<(), With<TestDnsLatencyButton>>,
    switch_buttons: Query<&DnsSwitchButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if clear_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ClearDnsCache);
    } else if test_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestDnsLatency);
    } else if let Ok(btn) = switch_buttons.get(activate.entity) {
        let key = match btn.0 {
            DnsSwitchKind::Enable => "dns.enable",
            DnsSwitchKind::Ipv6 => "dns.ipv6",
            DnsSwitchKind::Cache => "dns.cache",
            DnsSwitchKind::UseHosts => "dns.use_hosts",
            DnsSwitchKind::UseSystemHosts => "dns.use_system_hosts",
            DnsSwitchKind::RespectRules => "dns.respect_rules",
        };
        handle.submit(UiCommand::UpdateSetting {
            key: key.to_owned(),
            value: "toggle".to_owned(),
        });
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn apply_dns_projection(
    update: On<DnsProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastDnsProjection>>,
    mut lines: Query<
        (&mut Text, &DnsLine),
        (
            With<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerProto>,
            Without<DnsServerLatency>,
        ),
    >,
    mut addresses: Query<
        (&mut Text, &DnsServerAddress),
        (
            With<DnsServerAddress>,
            Without<DnsLine>,
            Without<DnsServerProto>,
            Without<DnsServerLatency>,
        ),
    >,
    mut protocols: Query<
        (&mut Text, &DnsServerProto),
        (
            With<DnsServerProto>,
            Without<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerLatency>,
        ),
    >,
    mut latencies: Query<
        (&mut Text, &mut TextColor, &DnsServerLatency),
        (
            With<DnsServerLatency>,
            Without<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerProto>,
        ),
    >,
    enhanced_controls: Query<&Children, With<DnsEnhancedModeControl>>,
    mut seg_values: Query<&mut SegmentedControlValue>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            DnsLineKind::Summary => {
                text.0 = format!(
                    "域名解析 · {} (缓存条目: {})",
                    projection.mode.label(),
                    projection.cache_entries
                );
            }
            DnsLineKind::FakeIpRange => {
                text.0 = format!("分配网段: {}", projection.fake_ip_range);
            }
        }
    }

    let target_mode_idx = match projection.mode {
        DnsMode::FakeIp => 0,
        DnsMode::RedirHost => 1,
    };
    for children in &enhanced_controls {
        for child in children.iter() {
            if let Ok(mut val) = seg_values.get_mut(*child)
                && val.0 != target_mode_idx
            {
                val.0 = target_mode_idx;
            }
        }
    }

    for (mut text, marker) in &mut addresses {
        if let Some(server) = projection.servers.get(marker.0) {
            text.0 = server.address.clone();
        }
    }

    for (mut text, marker) in &mut protocols {
        if let Some(server) = projection.servers.get(marker.0) {
            let fallback_badge = if server.is_fallback {
                " [Fallback]"
            } else {
                ""
            };
            text.0 = format!("{}{fallback_badge}", server.protocol);
        }
    }

    for (mut text, mut color, marker) in &mut latencies {
        if let Some(server) = projection.servers.get(marker.0) {
            let (str_val, tier) = format_latency(server.latency_ms);
            text.0 = str_val;
            color.0 = latency_color(tier, &palette);
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
    fn demo_dns_fixture() {
        let proj = DnsProjection::demo();
        assert_eq!(proj.mode, DnsMode::FakeIp);
        assert_eq!(proj.cache_entries, 342);
        assert_eq!(proj.fake_ip_range, "198.18.0.1/16");
        assert_eq!(proj.servers.len(), 4);
        assert_eq!(proj.servers[0].address, "https://1.1.1.1/dns-query");
        assert_eq!(proj.servers[0].protocol, "DoH (HTTPS)");
        assert_eq!(proj.servers[0].latency_ms, Some(28));
    }
}
