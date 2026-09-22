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
use infiltrator_contract::dns::{FakeIpMappingPool, FakeIpMappingSource};
use infiltrator_contract::dns_latency::{DnsLatencyReport, DnsLatencySummary, DnsProbeOutcome};
use infiltrator_contract::dns_self_heal::DnsSelfHealState;

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};

/// Marker on the Fake-IP search field's parent node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFakeIpSearchField;

/// DUAL-14-10: bare-Chinese latency policy copy (Bevy page convention).
pub fn latency_policy_label(report: &DnsLatencyReport) -> String {
    match report.summary() {
        DnsLatencySummary::AllMeasured {
            count,
            best_ms,
            worst_ms,
        } => format!("逐 Nameserver 延迟: {count} 个上游全部应答 ({best_ms}-{worst_ms} ms)"),
        DnsLatencySummary::Partial { measured, total } => {
            format!("逐 Nameserver 延迟: {total} 个上游中 {measured} 个应答")
        }
        DnsLatencySummary::NoneReachable { total } => {
            format!("逐 Nameserver 延迟: {total} 个上游全部无应答")
        }
        DnsLatencySummary::NotProbed => "逐 Nameserver 延迟: 本次会话尚未测速".to_owned(),
        DnsLatencySummary::Unsupported { reason } => {
            format!("逐 Nameserver 延迟: 宿主未提供探测能力 ({reason})")
        }
    }
}

/// DUAL-14-10: one line per probed nameserver, with its real outcome.
pub fn latency_result_listing(report: &DnsLatencyReport) -> String {
    if !report.status.is_ready() {
        return "无逐 Nameserver 结果: 宿主未注入探测端口".to_owned();
    }
    if report.results.is_empty() {
        return "尚无逐 Nameserver 测速结果".to_owned();
    }
    report
        .results
        .iter()
        .map(|result| {
            let tier = if result.is_fallback {
                "Fallback"
            } else {
                "主上游"
            };
            let outcome = match &result.outcome {
                DnsProbeOutcome::Measured { rtt_ms } => format!("{rtt_ms} ms"),
                DnsProbeOutcome::TimedOut => "超时无应答".to_owned(),
                DnsProbeOutcome::InvalidResponse { reason } => format!("应答未通过校验 ({reason})"),
                DnsProbeOutcome::Failed { message } => format!("探测失败 ({message})"),
                DnsProbeOutcome::NotProbed { reason } => format!("未探测 ({reason})"),
            };
            format!("{} [{tier}] {outcome}", result.address)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// DUAL-14-13: the bare-Chinese self-heal state label.
pub fn self_heal_state_label(state: DnsSelfHealState) -> &'static str {
    match state {
        DnsSelfHealState::Healthy => "正常",
        DnsSelfHealState::Warning => "注意",
        DnsSelfHealState::Critical => "故障",
        DnsSelfHealState::Unknown => "未观测",
    }
}

/// DUAL-14-13: the multi-line self-heal observation (one row per check).
pub fn self_heal_listing(
    snapshot: &infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot,
) -> String {
    use infiltrator_contract::dns_self_heal::DnsSelfHealKind;
    if snapshot.checks.is_empty() {
        return "尚未观测到 DNS 健康事实".to_owned();
    }
    snapshot
        .checks
        .iter()
        .map(|check| {
            let kind = match check.kind {
                DnsSelfHealKind::ListenPort => "监听端口 (dns.listen)",
                DnsSelfHealKind::UpstreamResolution => "上游解析可达性",
                DnsSelfHealKind::Topology => "拓扑抗泄漏审计",
            };
            let fix = check
                .fix
                .map(|fix| format!(" · 建议修复: {}", fix.key()))
                .unwrap_or_default();
            format!(
                "{kind} [{}] {}{fix}",
                self_heal_state_label(check.state),
                check.detail
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
        assert!(latency_policy_label(&report).contains("3 个上游中 1 个应答"));

        let listing = latency_result_listing(&report);
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
        assert!(latency_policy_label(&all_measured).contains("1 个上游全部应答 (31-31 ms)"));

        let none = DnsLatencyReport::measured(
            DEFAULT_PROBE_QUESTION,
            vec![DnsServerLatency {
                address: "1.1.1.1".to_owned(),
                is_fallback: false,
                transport: DnsProbeTransport::Udp,
                outcome: DnsProbeOutcome::TimedOut,
            }],
        );
        assert!(latency_policy_label(&none).contains("全部无应答"));
        assert_eq!(
            latency_policy_label(&DnsLatencyReport::default()),
            "逐 Nameserver 延迟: 宿主未提供探测能力 (this host did not inject a DNS latency prober)"
        );
    }

    #[test]
    fn the_self_heal_listing_never_claims_health_without_an_observation() {
        use infiltrator_contract::dns_self_heal::{
            DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot,
            DnsSelfHealState,
        };

        let empty = DnsSelfHealSnapshot::default();
        assert_eq!(self_heal_state_label(empty.overall_state()), "未观测");
        assert_eq!(self_heal_listing(&empty), "尚未观测到 DNS 健康事实");

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
        let listing = self_heal_listing(&snapshot);
        assert_eq!(listing.lines().count(), 2);
        assert!(listing.contains("监听端口 (dns.listen) [故障]"));
        assert!(listing.contains("建议修复: repair_dns_listen_port"));
        assert!(listing.contains("上游解析可达性 [正常]"));
        assert_eq!(self_heal_state_label(snapshot.overall_state()), "故障");
    }

    #[test]
    fn unsupported_pool_never_renders_a_binding() {
        let unsupported = FakeIpMappingPool::default();
        assert!(fake_ip_mapping_listing(&unsupported, "").contains("宿主未提供映射事实"));
        assert_eq!(
            fake_ip_mapping_count(&unsupported, ""),
            "实时连接观测: 不可用"
        );
        let report = infiltrator_contract::dns_latency::DnsLatencyReport::unsupported(
            "host injected no DNS latency prober",
        );
        assert!(latency_policy_label(&report).contains("宿主未提供探测能力"));
        assert!(
            latency_result_listing(&report).contains("宿主未注入探测端口"),
            "a host without a prober renders no fabricated result row"
        );
    }
}
