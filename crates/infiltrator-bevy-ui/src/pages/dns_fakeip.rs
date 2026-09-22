//! DUAL-14-06 / 14-10: the Bevy Fake-IP mapping pool card and the honest
//! per-nameserver latency policy line.
//!
//! The mapping listing is the observed subset published by the shared read
//! model (`host` ↔ `destinationIP` of connections resolved in Fake-IP mode).
//! The search box is a local view filter over that shared fact; the card never
//! authors a binding of its own.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{DnsLatencyStatus, FakeIpMappingPool, FakeIpMappingSource};

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};

/// Marker on the Fake-IP search field's parent node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFakeIpSearchField;

/// DUAL-14-10: bare-Chinese latency policy copy (Bevy page convention).
pub fn latency_policy_label(status: DnsLatencyStatus) -> String {
    match status {
        DnsLatencyStatus::Ready => "逐 Nameserver 延迟: 宿主提供真实事实".to_owned(),
        DnsLatencyStatus::Unsupported => {
            "逐 Nameserver 延迟: 宿主无该事实源，不填充假延迟".to_owned()
        }
    }
}

/// The multi-line listing of the bindings matching `query`.
pub fn fake_ip_mapping_listing(pool: &FakeIpMappingPool, query: &str) -> String {
    if !pool.is_observed_subset() {
        return match &pool.source {
            FakeIpMappingSource::Unsupported { reason } => {
                format!("宿主未提供映射事实 ({reason})")
            }
            FakeIpMappingSource::Unavailable { reason } => {
                format!("内核连接表不可用 ({reason})")
            }
            FakeIpMappingSource::LiveConnections => String::new(),
        };
    }
    let matches = pool.filter(query);
    if matches.is_empty() {
        return if pool.entries.is_empty() {
            "当前没有可观察的 Fake-IP 绑定".to_owned()
        } else {
            "没有匹配的映射".to_owned()
        };
    }
    matches
        .iter()
        .map(|entry| format!("{} ↔ {}", entry.address, entry.domain))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The shown/total counter of the mapping listing.
pub fn fake_ip_mapping_count(pool: &FakeIpMappingPool, query: &str) -> String {
    if !pool.is_observed_subset() {
        return "实时连接观测: 不可用".to_owned();
    }
    format!(
        "实时连接观测: 显示 {} / 共 {} 条 (网段 {})",
        pool.filter(query).len(),
        pool.total,
        pool.range
    )
}

/// The Fake-IP mapping pool card (search + observed listing).
pub fn dns_fakeip_pool_card_scene(
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let listing = fake_ip_mapping_listing(&projection.fake_ip_pool, "");
    let count = fake_ip_mapping_count(&projection.fake_ip_pool, "");

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
                    (
                        Text({ "Fake-IP 映射池实时检索 (DUAL-14-06)".to_owned() })
                        TextRole(Role::BodyStrong)
                    ),
                    ( Text(count) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node { width: percent(100) }
                DnsFakeIpSearchField
                Children [
                    (
                        { text_field_with_placeholder_scene(
                            String::new(),
                            "搜索域名或虚拟 IP".to_owned(),
                            palette,
                        ) }
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    min_height: px(48.0),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(
                        palette.control_radius_px,
                    )),
                    flex_direction: FlexDirection::Column,
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    (
                        Text(listing)
                        DnsLine(DnsLineKind::FakeIpMapping)
                        TextRole(Role::Mono)
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    (
                        Text({ String::new() })
                        DnsLine(DnsLineKind::FakeIpMappingCount)
                        TextRole(Role::Caption)
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// Per-frame search filter: restamp the shared listing with the typed query.
pub fn sync_dns_fake_ip_filter(
    fields: Query<(&Children, &DnsFakeIpSearchField)>,
    text_fields: Query<&TextField>,
    last: Option<Res<LastDnsProjection>>,
    mut lines: Query<(&mut Text, &DnsLine)>,
) {
    let Some(pool) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .map(|projection| projection.fake_ip_pool.clone())
    else {
        return;
    };
    let mut query = String::new();
    for (children, _) in &fields {
        for child in children.iter() {
            if let Ok(field) = text_fields.get(*child) {
                query = field.0.text().to_owned();
            }
        }
    }
    let listing = fake_ip_mapping_listing(&pool, &query);
    let count = fake_ip_mapping_count(&pool, &query);
    for (mut text, line) in &mut lines {
        let wanted = match line.0 {
            DnsLineKind::FakeIpMapping => Some(&listing),
            DnsLineKind::FakeIpMappingCount => Some(&count),
            _ => None,
        };
        if let Some(wanted) = wanted
            && &text.0 != wanted
        {
            text.0 = wanted.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns::FakeIpMappingEntry;

    fn pool() -> FakeIpMappingPool {
        FakeIpMappingPool {
            source: FakeIpMappingSource::LiveConnections,
            range: "198.18.0.1/16".to_owned(),
            total: 2,
            entries: vec![
                FakeIpMappingEntry {
                    domain: "music.example.org".to_owned(),
                    address: "198.18.0.5".to_owned(),
                },
                FakeIpMappingEntry {
                    domain: "cdn.example.net".to_owned(),
                    address: "198.18.0.7".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn listing_and_count_follow_the_local_query() {
        let pool = pool();
        let full = fake_ip_mapping_listing(&pool, "");
        assert!(full.contains("198.18.0.5 ↔ music.example.org"));
        assert_eq!(full.lines().count(), 2);
        let filtered = fake_ip_mapping_listing(&pool, "cdn");
        assert_eq!(filtered, "198.18.0.7 ↔ cdn.example.net");
        assert_eq!(fake_ip_mapping_listing(&pool, "nothing"), "没有匹配的映射");
        assert!(fake_ip_mapping_count(&pool, "").contains("显示 2 / 共 2 条"));
        assert!(fake_ip_mapping_count(&pool, "cdn").contains("显示 1 / 共 2 条"));
    }

    #[test]
    fn unsupported_pool_never_renders_a_binding() {
        let unsupported = FakeIpMappingPool::default();
        assert!(fake_ip_mapping_listing(&unsupported, "").contains("宿主未提供映射事实"));
        assert_eq!(
            fake_ip_mapping_count(&unsupported, ""),
            "实时连接观测: 不可用"
        );
        assert_eq!(
            latency_policy_label(DnsLatencyStatus::Unsupported),
            "逐 Nameserver 延迟: 宿主无该事实源，不填充假延迟"
        );
    }
}
