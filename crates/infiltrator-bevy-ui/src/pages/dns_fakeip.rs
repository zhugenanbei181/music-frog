//! DUAL-14-06 / 14-10: the Bevy Fake-IP mapping pool card and the honest
//! per-nameserver latency policy line.
//!
//! The mapping listing is the observed subset published by the shared read
//! model (`host` ↔ `destinationIP` of connections resolved in Fake-IP mode).
//! The search box is a local view filter over that shared fact; the card never
//! authors a binding of its own.

use crate::localized_widgets::localized_field_scene;
use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::dns_mapping_projection::project_mappings;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;

/// Marker on the Fake-IP search field's parent node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFakeIpSearchField;

/// The Fake-IP mapping pool card (search + observed listing).
pub fn dns_fakeip_pool_card_scene(
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let display = project_mappings(&projection.fake_ip_pool, "", UiLocale::default().code());
    let listing = display.listing();
    let count = display.count;

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
                                LocalizedText::plain("dns_fakeip_pool_title")
                                TextRole(Role::BodyStrong)
                                --
                                Text(count) DnsLine(DnsLineKind::FakeIpMappingCount) TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            DnsFakeIpSearchField
                            Children [
                                @{ (localized_field_scene(String::new(), LocalizedText::plain("dns_fakeip_pool_search"), palette), bsn! { NativeTextField(0) }) }
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
                                Text(listing)
                                DnsLine(DnsLineKind::FakeIpMapping)
                                TextRole(Role::Mono)
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
    locale: Res<UiLocale>,
    mut lines: Query<(&mut Text, &DnsLine)>,
) {
    let Some(pool) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .map(|projection| &projection.fake_ip_pool)
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
    let display = project_mappings(pool, &query, locale.code());
    let listing = display.listing();
    let count = display.count;
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
    use infiltrator_application::dns_health_projection::project_dns_health;
    use infiltrator_application::dns_latency_projection::project_dns_latency;
    use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};
    use infiltrator_contract::dns_latency::DnsLatencyReport;

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
        let full = project_mappings(&pool, "", "zh-CN").listing();
        assert!(full.contains("198.18.0.5 ↔ music.example.org"));
        assert_eq!(full.lines().count(), 2);
        let filtered = project_mappings(&pool, "cdn", "zh-CN").listing();
        assert_eq!(filtered, "198.18.0.7 ↔ cdn.example.net");
        assert_eq!(
            project_mappings(&pool, "nothing", "zh-CN").listing(),
            "没有匹配的映射"
        );
        assert!(
            project_mappings(&pool, "", "zh-CN")
                .count
                .contains("显示 2 / 共观测 2 条")
        );
        assert!(
            project_mappings(&pool, "cdn", "zh-CN")
                .count
                .contains("显示 1 / 共观测 2 条")
        );
    }

    #[test]
    fn the_latency_rows_carry_the_real_probe_outcomes() {
        use infiltrator_contract::dns_latency::{
            DEFAULT_PROBE_QUESTION, DnsLatencyReport, DnsLatencySummary, DnsProbeOutcome,
            DnsProbeTransport, DnsServerLatency,
        };

        let report = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![
                DnsServerLatency {
                    address: "223.5.5.5".to_owned(),
                    is_fallback: false,
                    transport: DnsProbeTransport::Udp,
                    outcome: DnsProbeOutcome::Measured { rtt_ms: 12 },
                },
                DnsServerLatency {
                    address: "8.8.8.8".to_owned(),
                    is_fallback: false,
                    transport: DnsProbeTransport::Udp,
                    outcome: DnsProbeOutcome::TimedOut,
                },
                DnsServerLatency {
                    address: "tls://1.0.0.1:853".to_owned(),
                    is_fallback: true,
                    transport: DnsProbeTransport::Undrivable {
                        reason: "DNS over TLS is not probed by this host".to_owned(),
                    },
                    outcome: DnsProbeOutcome::NotProbed {
                        reason: "DNS over TLS is not probed by this host".to_owned(),
                    },
                },
            ],
        );
        assert!(matches!(
            report.summary(),
            DnsLatencySummary::Partial {
                measured: 1,
                total: 3
            }
        ));
        assert!(
            project_dns_latency(&report, "zh-CN")
                .summary
                .contains("3 个上游中仅 1 个应答")
        );

        let listing = project_dns_latency(&report, "zh-CN").listing();
        assert_eq!(listing.lines().count(), 3);
        assert!(listing.contains("223.5.5.5 [主上游] 12 ms"));
        assert!(listing.contains("8.8.8.8 [主上游] 超时无应答"));
        assert!(listing.contains("tls://1.0.0.1:853 [Fallback] 未探测"));

        let all_measured = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![DnsServerLatency {
                address: "1.1.1.1".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Doh,
                outcome: DnsProbeOutcome::Measured { rtt_ms: 31 },
            }],
        );
        assert!(
            project_dns_latency(&all_measured, "zh-CN")
                .summary
                .contains("全部 1 个上游均应答 (31-31 ms)")
        );

        let none = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![DnsServerLatency {
                address: "1.1.1.1".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Udp,
                outcome: DnsProbeOutcome::TimedOut,
            }],
        );
        assert!(
            project_dns_latency(&none, "zh-CN")
                .summary
                .contains("全部无应答")
        );
        assert_eq!(
            project_dns_latency(&DnsLatencyReport::default(), "zh-CN").summary,
            "宿主未提供逐 Nameserver 延迟事实，不填充假延迟 (this host did not inject a DNS latency prober)"
        );
    }

    #[test]
    fn the_self_heal_listing_never_claims_health_without_an_observation() {
        use infiltrator_contract::dns_self_heal::{
            DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot,
            DnsSelfHealState,
        };

        let empty = DnsSelfHealSnapshot::default();
        assert_eq!(project_dns_health(&empty, "zh-CN").overall, "未观测");
        assert_eq!(
            project_dns_health(&empty, "zh-CN").listing(),
            "尚未观测到 DNS 健康事实"
        );

        let snapshot = DnsSelfHealSnapshot::new(vec![
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::ListenPort,
                state: DnsSelfHealState::Critical,
                detail: "dns.listen port 1053 is already bound by dig (pid 4242)".to_owned(),
                fix: Some(DnsSelfHealFix::RepairDnsListenPort),
            },
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::UpstreamResolution,
                state: DnsSelfHealState::Healthy,
                detail: "all 1 upstreams answered (9-9 ms)".to_owned(),
                fix: None,
            },
        ]);
        let listing = project_dns_health(&snapshot, "zh-CN").listing();
        assert_eq!(listing.lines().count(), 2);
        assert!(listing.contains("监听端口 (dns.listen) [故障]"));
        assert!(listing.contains("建议修复: 修复 DNS 监听端口"));
        assert!(listing.contains("上游解析可达性 [正常]"));
        assert_eq!(project_dns_health(&snapshot, "zh-CN").overall, "故障");
    }

    #[test]
    fn unsupported_pool_never_renders_a_binding() {
        let unsupported = FakeIpMappingPool::default();
        assert!(
            project_mappings(&unsupported, "", "zh-CN")
                .listing()
                .contains("宿主未提供映射事实")
        );
        assert_eq!(
            project_mappings(&unsupported, "", "zh-CN").count,
            "宿主未提供映射事实 (no running core connection feed on this host)"
        );
        let report = DnsLatencyReport::unsupported("host injected no DNS latency prober");
        assert!(
            project_dns_latency(&report, "zh-CN")
                .summary
                .contains("宿主未提供逐 Nameserver 延迟事实")
        );
        assert!(
            project_dns_latency(&report, "zh-CN")
                .listing()
                .contains("宿主未提供逐 Nameserver 延迟事实"),
            "a host without a prober renders no fabricated result row"
        );
    }
}
