//! DUAL-14-09 (re-scoped): the Bevy STUN UDP-egress card.
//!
//! The card renders the shared `StunProbeReport`: the configured STUN server,
//! the typed status and the public UDP mapping (IP:port) the server observed,
//! plus the fact comparison against the expected proxied egress. It never
//! claims a browser WebRTC result; the honest boundary is part of the copy.
//! Texts are restamped in place by `apply_dns_projection`
//! ([`DnsLineKind::StunConclusion`] / [`DnsLineKind::Stun`]).

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection};
use crate::pages::dns_self_heal::observation_color;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::stun_projection::project_stun;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// The STUN UDP-egress card (status + mapping / comparison listing).
pub fn dns_stun_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let display = project_stun(&projection.stun, UiLocale::default().code());
    let conclusion = display.status.clone();
    let listing = display.listing();
    let conclusion_color = observation_color(display.tone, palette);

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
                                Text(conclusion)
                                DnsLine(DnsLineKind::StunConclusion)
                                TextRole(Role::BodyStrong)
                                TextColor(conclusion_color)
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
                                DnsLine(DnsLineKind::Stun)
                                TextRole(Role::Mono)
                            ]
            }),
        ],
        palette,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use infiltrator_bevy_widgets::theme::Theme;
    #[cfg(test)]
    use infiltrator_contract::dns::DnsCoreSwitches;
    #[cfg(test)]
    use infiltrator_contract::dns::DnsEnhancedMode;
    #[cfg(test)]
    use infiltrator_contract::dns::DnsFakeIpFilterMode;
    #[cfg(test)]
    use infiltrator_contract::dns::FakeIpMappingPool;
    #[cfg(test)]
    use infiltrator_contract::dns_cache::DnsCacheFlushReport;
    #[cfg(test)]
    use infiltrator_contract::dns_form::DnsWorkbenchForm;
    #[cfg(test)]
    use infiltrator_contract::dns_latency::DnsLatencyReport;
    #[cfg(test)]
    use infiltrator_contract::dns_leak::DnsLeakReport;
    #[cfg(test)]
    use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
    use infiltrator_contract::stun_probe::{
        StunMappedAddress, StunProbeObservation, StunProbeReport,
    };

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
        let label = project_stun(&consistent, "zh-CN").status;
        assert!(label.contains("203.0.113.9:51234"), "{label}");
        let listing = project_stun(&consistent, "zh-CN").listing();
        assert!(listing.contains("stun.l.google.com:19302"), "{listing}");
        assert!(listing.contains("与期望代理出网一致"), "{listing}");
        assert!(listing.contains("不是某个浏览器实例的 WebRTC"), "{listing}");
        assert!(!listing.contains("泄漏结论"), "{listing}");

        let divergent = observed("198.51.100.7", Some("203.0.113.9"));
        let listing = project_stun(&divergent, "zh-CN").listing();
        assert!(listing.contains("不一致"), "{listing}");
        assert!(listing.contains("198.51.100.7:51234"), "{listing}");
        assert!(listing.contains("203.0.113.9"), "{listing}");
        assert!(listing.contains("仅列事实"), "{listing}");

        // No expected egress is an explicit unknown, never an implied "clean".
        let unknown = observed("203.0.113.9", None);
        let listing = project_stun(&unknown, "zh-CN").listing();
        assert!(listing.contains("无法比较"), "{listing}");
    }

    #[test]
    fn a_host_without_a_prober_renders_the_typed_unsupported_copy() {
        let unsupported = StunProbeReport::unsupported("no STUN prober is composed");
        let label = project_stun(&unsupported, "zh-CN").status;
        assert!(label.contains("宿主未提供 STUN 探测能力"), "{label}");
        assert!(!label.contains("观测 "), "{label}");

        let timed_out = StunProbeReport::from_observation(
            StunProbeObservation::timed_out("stun.l.google.com:19302"),
            None,
        );
        assert!(project_stun(&timed_out, "zh-CN").status.contains("超时"));

        let failed = StunProbeReport::from_observation(
            StunProbeObservation::failed("stun.l.google.com:19302", "network unreachable"),
            None,
        );
        assert!(project_stun(&failed, "zh-CN").status.contains("探测失败"));

        // Nothing reported yet is not a result either.
        let pending = StunProbeReport::default();
        assert!(project_stun(&pending, "zh-CN").status.contains("尚未探测"));
    }

    #[test]
    fn the_card_scene_builds_for_every_status_shape() {
        let palette = UiPalette::new(&Theme::dark());
        let projection = |report| DnsProjection {
            mode: DnsEnhancedMode::default(),
            cache_entries: 0,
            fake_ip_range: String::new(),
            servers: Vec::new(),
            switches: DnsCoreSwitches::default(),
            filter_mode: DnsFakeIpFilterMode::default(),
            form: DnsWorkbenchForm::default(),
            cache_flush: DnsCacheFlushReport::default(),
            fake_ip_pool: FakeIpMappingPool::default(),
            latency: DnsLatencyReport::default(),
            leak: DnsLeakReport::default(),
            stun: report,
            self_heal: DnsSelfHealSnapshot::default(),
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
