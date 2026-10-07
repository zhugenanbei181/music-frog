//! The DNS page's upstream-nameserver sub-feature (域名解析上游服务器).
//!
//! One row per upstream resolver, the protocol / fallback / tag listing and
//! the honest per-nameserver latency policy + result rows. The projection
//! observer in [`super::dns`] restamps these marked texts in place.

use super::dns::{DnsLine, DnsLineKind, DnsServerItem};
use crate::pages::proxies::latency_color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::dns_latency_projection::{project_dns_latency, server_tags_text};
use infiltrator_application::latency_projection::project_measured_latency;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns_latency::DnsLatencyReport;

/// Marker for a DNS server's address display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerAddress(pub usize);

/// Marker for a DNS server's protocol display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerProto(pub usize);

/// Marker for a DNS server's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerLatency(pub usize);

pub(crate) fn servers_card_scene(
    server_scenes: Vec<Box<dyn Scene>>,
    latency: &DnsLatencyReport,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let display = project_dns_latency(latency, UiLocale::default().code());
    let latency_label = display.summary.clone();
    let latency_results = display.listing();

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
                                LocalizedText::plain("dns_nameservers_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("dns_nameservers_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                Text(latency_label)
                                DnsLine(DnsLineKind::LatencyPolicy)
                                TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                min_height: px(32.0),
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                Text(latency_results)
                                DnsLine(DnsLineKind::LatencyResults)
                                TextRole(Role::Mono)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                { server_scenes }
                            ]
            }),
        ],
        palette,
    )
}

pub(crate) fn server_row_scene(
    idx: usize,
    server: &DnsServerItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let addr = server.address.clone();
    let proto = server.protocol.clone();
    let fallback_badge = if server.is_fallback {
        " [Fallback]"
    } else {
        ""
    };
    let proto_str = format!("{proto}{fallback_badge}");
    let tag_str = server_tags_text(&server.tags, UiLocale::default().code());
    let latency = project_measured_latency(server.latency_ms);
    let lat_col = latency_color(latency.band, palette);
    let caption = LocalizedText::new(latency.key, latency.params);

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
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    Text(addr) DnsServerAddress(idx) TextRole(Role::BodyStrong)
                    --
                    Text(proto_str) DnsServerProto(idx) TextRole(Role::Caption)
                    --
                    Text(tag_str) DnsLine(DnsLineKind::ServerTags(idx)) TextRole(Role::Caption)
                ]
                --
                LocalizedText::new(caption.key,caption.params)
                DnsServerLatency(idx)
                TextRole(Role::Mono)
                TextColor(lat_col)
            ]
    }
}
