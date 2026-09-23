//! DUAL-14-08: the Bevy DNS leak cross-source card.
//!
//! The card renders the shared `DnsLeakReport`: one row per configured probe
//! source with the resolver identity the echo authority observed, plus the
//! typed conclusion. A divergent report lists the observed facts and never
//! picks one; a host without a fact source renders the typed unsupported copy
//! (the old hardcoded country/ISP values are gone). Texts are restamped in
//! place by `apply_dns_projection` ([`DnsLineKind::Leak`]).

use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns_leak::{DnsLeakConclusion, DnsLeakObservationOutcome, DnsLeakReport};

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};

/// DUAL-14-08: bare-Chinese conclusion copy (Bevy page convention).
pub fn leak_conclusion_label(report: &DnsLeakReport) -> String {
    match report.conclusion() {
        DnsLeakConclusion::Consistent { identity, facts } => format!(
            "DNS 泄漏交叉探测: {} 个来源观测到同一解析器身份 {identity}",
            facts.len()
        ),
        DnsLeakConclusion::Divergent { facts } => {
            let identities: std::collections::BTreeSet<&str> =
                facts.iter().map(|fact| fact.identity.as_str()).collect();
            format!(
                "DNS 泄漏交叉探测: 观测到 {} 个不同解析器身份 (仅列事实)",
                identities.len()
            )
        }
        DnsLeakConclusion::Unsupported { reason } => {
            format!("DNS 泄漏交叉探测: 宿主未提供事实源 ({reason})")
        }
        DnsLeakConclusion::Failed { reason } => {
            format!("DNS 泄漏交叉探测: 全部探测源失败 ({reason})")
        }
        DnsLeakConclusion::Unknown => {
            "DNS 泄漏交叉探测: 暂无交叉结论 (不足两个观测事实)".to_owned()
        }
    }
}

/// DUAL-14-08: one line per attempted source, with its real outcome.
pub fn leak_observation_listing(report: &DnsLeakReport) -> String {
    if report.observations.is_empty() {
        return "尚无泄漏探测观测结果".to_owned();
    }
    report
        .observations
        .iter()
        .map(|observation| {
            let outcome = match &observation.outcome {
                DnsLeakObservationOutcome::Observed { identity } => {
                    format!("观测身份 {identity}")
                }
                DnsLeakObservationOutcome::TimedOut => "超时无应答".to_owned(),
                DnsLeakObservationOutcome::InvalidResponse { reason } => {
                    format!("应答未通过校验 ({reason})")
                }
                DnsLeakObservationOutcome::Failed { message } => {
                    format!("探测失败 ({message})")
                }
                DnsLeakObservationOutcome::NotProbed { reason } => {
                    format!("未探测 ({reason})")
                }
            };
            format!(
                "{} → {} {outcome}",
                observation.resolver, observation.authority
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The configured source count of the shared report.
pub fn leak_source_count_label(report: &DnsLeakReport) -> String {
    format!("已配置探测源 {} 个", report.sources.len())
}

/// DUAL-14-08: the conclusion ink (agreed / divergent / typed refusal).
pub fn leak_conclusion_color(report: &DnsLeakReport, palette: &UiPalette) -> bevy::prelude::Color {
    match report.conclusion() {
        DnsLeakConclusion::Consistent { .. } => palette.success,
        DnsLeakConclusion::Divergent { .. } => palette.danger,
        DnsLeakConclusion::Failed { .. } => palette.warning,
        DnsLeakConclusion::Unknown | DnsLeakConclusion::Unsupported { .. } => palette.ink_dim,
    }
}

/// The DNS leak cross-source card (conclusion + per-source listing).
pub fn dns_leak_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let conclusion = leak_conclusion_label(&projection.leak);
    let sources = leak_source_count_label(&projection.leak);
    let listing = leak_observation_listing(&projection.leak);
    let conclusion_color = leak_conclusion_color(&projection.leak, palette);

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
                        Text(conclusion)
                        DnsLine(DnsLineKind::LeakConclusion)
                        TextRole(Role::BodyStrong)
                        TextColor(conclusion_color)
                    ),
                    ( Text(sources) TextRole(Role::Caption) ),
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
                        DnsLine(DnsLineKind::Leak)
                        TextRole(Role::Mono)
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// Per-frame safety net: restamp the leak row after a theme or projection
/// replay that did not run the observer.
pub fn sync_dns_leak_line(
    last: Option<Res<LastDnsProjection>>,
    mut lines: Query<(&mut Text, &DnsLine)>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let listing = leak_observation_listing(&projection.leak);
    let conclusion = leak_conclusion_label(&projection.leak);
    for (mut text, line) in &mut lines {
        match line.0 {
            DnsLineKind::Leak if text.0 != listing => text.0 = listing.clone(),
            DnsLineKind::LeakConclusion if text.0 != conclusion => text.0 = conclusion.clone(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_leak::{
        DnsLeakEchoRecord, DnsLeakObservation, DnsLeakObservationOutcome, DnsLeakProbeSource,
        DnsLeakProbeTransport, DnsLeakReport,
    };

    fn report(identities: &[(&str, &str)]) -> DnsLeakReport {
        let observations = identities
            .iter()
            .enumerate()
            .map(|(index, (authority, identity))| DnsLeakObservation {
                resolver: "1.1.1.1".to_owned(),
                authority: (*authority).to_owned(),
                question: format!("l{index}.{authority}"),
                transport: DnsLeakProbeTransport::Udp,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: (*identity).to_owned(),
                },
            })
            .collect();
        DnsLeakReport::observed(
            vec![DnsLeakProbeSource::new("1.1.1.1", "a.echo.example.org")],
            observations,
        )
    }

    #[test]
    fn the_card_renders_the_configured_txt_sources() {
        let report = DnsLeakReport::observed(
            vec![
                DnsLeakProbeSource::exact(
                    "system",
                    "whoami.ds.akahelp.net",
                    DnsLeakEchoRecord::TxtKeyedValue {
                        key: "ip".to_owned(),
                    },
                ),
                DnsLeakProbeSource::exact(
                    "system",
                    "o-o.myaddr.l.google.com",
                    DnsLeakEchoRecord::TxtFirstIpAddress,
                ),
            ],
            vec![
                DnsLeakObservation {
                    resolver: "system".to_owned(),
                    authority: "whoami.ds.akahelp.net".to_owned(),
                    question: "whoami.ds.akahelp.net".to_owned(),
                    transport: DnsLeakProbeTransport::System,
                    outcome: DnsLeakObservationOutcome::Observed {
                        identity: "203.0.113.9".to_owned(),
                    },
                },
                DnsLeakObservation {
                    resolver: "system".to_owned(),
                    authority: "o-o.myaddr.l.google.com".to_owned(),
                    question: "o-o.myaddr.l.google.com".to_owned(),
                    transport: DnsLeakProbeTransport::System,
                    outcome: DnsLeakObservationOutcome::Observed {
                        identity: "203.0.113.9".to_owned(),
                    },
                },
            ],
        );
        assert_eq!(report.sources.len(), 2);
        let listing = leak_observation_listing(&report);
        assert!(
            listing.contains("system → whoami.ds.akahelp.net 观测身份 203.0.113.9"),
            "{listing}"
        );
        assert!(
            listing.contains("system → o-o.myaddr.l.google.com 观测身份 203.0.113.9"),
            "{listing}"
        );
        assert_eq!(leak_source_count_label(&report), "已配置探测源 2 个");
        assert!(leak_conclusion_label(&report).contains("2 个来源观测到同一解析器身份"));
    }

    #[test]
    fn the_card_lists_every_observed_fact_without_choosing_one() {
        let divergent = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "198.51.100.7"),
        ]);
        let label = leak_conclusion_label(&divergent);
        assert!(label.contains("2 个不同解析器身份"), "{label}");
        assert!(label.contains("仅列事实"), "{label}");
        let listing = leak_observation_listing(&divergent);
        assert_eq!(listing.lines().count(), 2);
        assert!(listing.contains("1.1.1.1 → a.echo.example.org 观测身份 203.0.113.9"));
        assert!(listing.contains("1.1.1.1 → b.echo.example.org 观测身份 198.51.100.7"));

        let consistent = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "203.0.113.9"),
        ]);
        let label = leak_conclusion_label(&consistent);
        assert!(
            label.contains("2 个来源观测到同一解析器身份 203.0.113.9"),
            "{label}"
        );
        assert!(leak_source_count_label(&consistent).contains("1 个"));
    }

    #[test]
    fn a_host_without_a_fact_source_renders_the_typed_unsupported_copy() {
        let unsupported = DnsLeakReport::unsupported("no echo authority is configured");
        let label = leak_conclusion_label(&unsupported);
        assert!(label.contains("宿主未提供事实源"), "{label}");
        assert_eq!(
            leak_observation_listing(&unsupported),
            "尚无泄漏探测观测结果"
        );
        assert_eq!(leak_source_count_label(&unsupported), "已配置探测源 0 个");
        assert!(!label.contains("一致"));
        assert!(!label.contains("US"));
        assert!(!label.contains("Cloudflare"));

        // Nothing reported yet is not a verdict either.
        let pending = DnsLeakReport::default();
        let pending_label = leak_conclusion_label(&pending);
        assert!(pending_label.contains("暂无交叉结论"), "{pending_label}");
        assert!(!pending_label.contains("宿主未提供"), "{pending_label}");
    }

    #[test]
    fn the_card_scene_builds_for_every_conclusion_shape() {
        let palette = UiPalette::new(&infiltrator_bevy_widgets::theme::Theme::dark());
        let mut projection = DnsProjection {
            mode: infiltrator_contract::dns::DnsEnhancedMode::default(),
            cache_entries: 0,
            fake_ip_range: String::new(),
            servers: Vec::new(),
            switches: infiltrator_contract::dns::DnsCoreSwitches::default(),
            filter_mode: infiltrator_contract::dns::DnsFakeIpFilterMode::default(),
            form: infiltrator_contract::dns_form::DnsWorkbenchForm::default(),
            cache_flush: infiltrator_contract::dns::DnsCacheFlushReport::default(),
            fake_ip_pool: infiltrator_contract::dns::FakeIpMappingPool::default(),
            latency: infiltrator_contract::dns_latency::DnsLatencyReport::default(),
            leak: DnsLeakReport::default(),
            self_heal: infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot::default(),
            hosts: Vec::new(),
        };
        let _ = dns_leak_card_scene(&projection, &palette);

        projection.leak = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "198.51.100.7"),
        ]);
        let _ = dns_leak_card_scene(&projection, &palette);

        projection.leak = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "203.0.113.9"),
        ]);
        let _ = dns_leak_card_scene(&projection, &palette);
    }
}
