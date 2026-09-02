//! The DNS page (域名解析): upstream nameservers, DoH / DoT / DoQ endpoints,
//! Fake-IP filter rules, and DNS cache status.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DnsLine`],
//! [`DnsServerLatency`]). The page self-registers
//! [`apply_dns_projection`] once per world via [`DnsPageRoot`].

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
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

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

/// Marker for a DNS server's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerLatency(pub usize);

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
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Dns)
        DnsPageRoot
        Children [
            ( { header_card_scene(summary, palette) } ),
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
                            BackgroundColor({ palette.surface_elevated })
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
                    ( Text(addr) TextRole(Role::BodyStrong) ),
                    ( Text(proto_str) TextRole(Role::Caption) ),
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
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_dns_projection(
    update: On<DnsProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastDnsProjection>>,
    mut lines: Query<(&mut Text, &DnsLine), (With<DnsLine>, Without<DnsServerLatency>)>,
    mut latencies: Query<
        (&mut Text, &mut TextColor, &DnsServerLatency),
        (With<DnsServerLatency>, Without<DnsLine>),
    >,
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
        assert_eq!(proj.servers.len(), 4);
        assert_eq!(proj.fake_ip_range, "198.18.0.1/16");
    }
}
