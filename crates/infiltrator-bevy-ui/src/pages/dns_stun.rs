//! DUAL-14-09 (re-scoped): the Bevy STUN UDP-egress card.
//!
//! The card renders the shared `StunProbeReport`: the configured STUN server,
//! the typed status and the public UDP mapping (IP:port) the server observed,
//! plus the fact comparison against the expected proxied egress. It never
//! claims a browser WebRTC result; the honest boundary is part of the copy.
//! Texts are restamped in place by `apply_dns_projection`
//! ([`DnsLineKind::StunConclusion`] / [`DnsLineKind::Stun`]).

use bevy::ecs::hierarchy::Children;
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
use infiltrator_contract::stun_probe::{StunEgressComparison, StunProbeReport, StunProbeStatus};

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection};

/// DUAL-14-09: bare-Chinese status headline (Bevy page convention).
pub fn stun_conclusion_label(report: &StunProbeReport) -> String {
    match &report.status {
        StunProbeStatus::Observed => {
            let mapping = report
                .mapping()
                .map(|mapping| mapping.display())
                .unwrap_or_default();
            format!("STUN UDP 出网映射: 观测 {mapping}")
        }
        StunProbeStatus::TimedOut => "STUN UDP 出网映射: 超时（服务器未在期限内应答）".to_owned(),
        StunProbeStatus::Failed { message } => {
            format!("STUN UDP 出网映射: 探测失败 ({message})")
        }
        StunProbeStatus::Unsupported { reason } => {
            format!("STUN UDP 出网映射: 宿主未提供 STUN 探测能力 ({reason})")
        }
        StunProbeStatus::Unknown => "STUN UDP 出网映射: 尚未探测".to_owned(),
    }
}

/// The comparison against the expected proxied egress, as facts only.
pub fn stun_comparison_label(report: &StunProbeReport) -> String {
    match report.comparison() {
        StunEgressComparison::Consistent => "与期望代理出网一致".to_owned(),
        StunEgressComparison::Divergent => {
            let observed = report
                .mapping()
                .map(|mapping| mapping.display())
                .unwrap_or_default();
            let expected = report.expected_egress_ip().unwrap_or_default();
            format!("与期望代理出网不一致: 观测 {observed} ≠ 期望 {expected} (仅列事实)")
        }
        StunEgressComparison::Unknown { reason } => format!("无法比较: {reason}"),
    }
}

/// The server line plus the comparison and the honest boundary.
pub fn stun_mapping_listing(report: &StunProbeReport) -> String {
    [
        format!("服务器 {}", report.server),
        stun_comparison_label(report),
        "边界: 这是本机/本进程的 UDP 出网映射, 不是浏览器 WebRTC 穿透结论".to_owned(),
    ]
    .join("\n")
}

/// DUAL-14-09: the status ink (observed / timeout / failure / typed refusal).
pub fn stun_conclusion_color(
    report: &StunProbeReport,
    palette: &UiPalette,
) -> bevy::prelude::Color {
    match report.status {
        StunProbeStatus::Observed => palette.success,
        StunProbeStatus::Failed { .. } => palette.danger,
        StunProbeStatus::TimedOut => palette.warning,
        StunProbeStatus::Unsupported { .. } | StunProbeStatus::Unknown => palette.ink_dim,
    }
}

/// The STUN UDP-egress card (status + mapping / comparison listing).
pub fn dns_stun_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let conclusion = stun_conclusion_label(&projection.stun);
    let listing = stun_mapping_listing(&projection.stun);
    let conclusion_color = stun_conclusion_color(&projection.stun, palette);

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
                        DnsLine(DnsLineKind::StunConclusion)
                        TextRole(Role::BodyStrong)
                        TextColor(conclusion_color)
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
                        DnsLine(DnsLineKind::Stun)
                        TextRole(Role::Mono)
                    ),
                ]
            }),
        ],
        palette,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::stun_probe::{StunMappedAddress, StunProbeObservation};

    fn observed(ip: &str, expected: Option<&str>) -> StunProbeReport {
        StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun.l.google.com:19302",
                StunMappedAddress::new(ip, 51234),
            ),
            expected.map(|ip| StunMappedAddress::new(ip, 0)),
        )
    }

    #[test]
    fn the_card_renders_the_observed_mapping_and_never_a_webrtc_verdict() {
        let consistent = observed("203.0.113.9", Some("203.0.113.9"));
        let label = stun_conclusion_label(&consistent);
        assert!(label.contains("203.0.113.9:51234"), "{label}");
        let listing = stun_mapping_listing(&consistent);
        assert!(listing.contains("stun.l.google.com:19302"), "{listing}");
        assert!(listing.contains("与期望代理出网一致"), "{listing}");
        assert!(listing.contains("不是浏览器 WebRTC"), "{listing}");
        assert!(!listing.contains("泄漏结论"), "{listing}");

        let divergent = observed("198.51.100.7", Some("203.0.113.9"));
        let listing = stun_mapping_listing(&divergent);
        assert!(listing.contains("不一致"), "{listing}");
        assert!(listing.contains("198.51.100.7:51234"), "{listing}");
        assert!(listing.contains("203.0.113.9"), "{listing}");
        assert!(listing.contains("仅列事实"), "{listing}");

        // No expected egress is an explicit unknown, never an implied "clean".
        let unknown = observed("203.0.113.9", None);
        let listing = stun_mapping_listing(&unknown);
        assert!(listing.contains("无法比较"), "{listing}");
    }

    #[test]
    fn a_host_without_a_prober_renders_the_typed_unsupported_copy() {
        let unsupported = StunProbeReport::unsupported("no STUN prober is composed");
        let label = stun_conclusion_label(&unsupported);
        assert!(label.contains("宿主未提供 STUN 探测能力"), "{label}");
        assert!(!label.contains("观测 "), "{label}");

        let timed_out = StunProbeReport::from_observation(
            StunProbeObservation::timed_out("stun.l.google.com:19302"),
            None,
        );
        assert!(stun_conclusion_label(&timed_out).contains("超时"));

        let failed = StunProbeReport::from_observation(
            StunProbeObservation::failed("stun.l.google.com:19302", "network unreachable"),
            None,
        );
        assert!(stun_conclusion_label(&failed).contains("探测失败"));

        // Nothing reported yet is not a result either.
        let pending = StunProbeReport::default();
        assert!(stun_conclusion_label(&pending).contains("尚未探测"));
    }

    #[test]
    fn the_card_scene_builds_for_every_status_shape() {
        let palette = UiPalette::new(&infiltrator_bevy_widgets::theme::Theme::dark());
        let projection = |report| DnsProjection {
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
            leak: infiltrator_contract::dns_leak::DnsLeakReport::default(),
            stun: report,
            self_heal: infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot::default(),
            hosts: Vec::new(),
        };
        for report in [
            StunProbeReport::default(),
            StunProbeReport::unsupported("no STUN prober is composed"),
            observed("203.0.113.9", Some("203.0.113.9")),
            observed("198.51.100.7", Some("203.0.113.9")),
            observed("203.0.113.9", None),
            StunProbeReport::from_observation(
                StunProbeObservation::timed_out("stun.l.google.com:19302"),
                None,
            ),
        ] {
            let _ = dns_stun_card_scene(&projection(report), &palette);
        }
    }
}
