//! DUAL-14-08: the Bevy DNS leak cross-source card.
//!
//! The card renders the shared `DnsLeakReport`: one row per configured probe
//! source with the resolver identity the echo authority observed, plus the
//! typed conclusion. A divergent report lists the observed facts and never
//! picks one; a host without a fact source renders the typed unsupported copy
//! (the old hardcoded country/ISP values are gone). Texts are restamped in
//! place by `apply_dns_projection` ([`DnsLineKind::Leak`]).

use crate::localized_widgets::localized_button_scene;
use crate::pages::dns::TestDnsLeakButton;
use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};
use crate::pages::dns_leak_actions::RetryLeakButton;
use crate::pages::dns_leak_rows::rows_scene;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::dns_leak_projection::{LeakTone, project_leak};
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns_leak::DnsLeakReport;

#[derive(Component, Clone, Default)]
pub struct LeakCard;
#[derive(Component, Clone, Default)]
pub struct LeakFeedback;

pub fn leak_conclusion_label(report: &DnsLeakReport, code: &str) -> String {
    project_leak(report, code).conclusion
}
pub fn leak_observation_listing(report: &DnsLeakReport, code: &str) -> String {
    project_leak(report, code).listing()
}
pub fn leak_source_count_label(report: &DnsLeakReport, code: &str) -> String {
    project_leak(report, code).sources
}
pub(crate) fn tone_color(tone: LeakTone, palette: &UiPalette) -> Color {
    match tone {
        LeakTone::Success => palette.success,
        LeakTone::Danger => palette.danger,
        LeakTone::Warning => palette.warning,
        LeakTone::Neutral => palette.ink_dim,
    }
}
pub fn leak_conclusion_color(report: &DnsLeakReport, palette: &UiPalette) -> Color {
    tone_color(project_leak(report, "en-US").tone, palette)
}

/// The DNS leak cross-source card (conclusion + per-source listing).
pub fn dns_leak_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let display = project_leak(&projection.leak, UiLocale::default().code());
    let rows = rows_scene(&display, palette);
    let conclusion = display.conclusion;
    let sources = display.sources;
    let conclusion_color = tone_color(display.tone, palette);

    (
        surface_scene(
            vec![
                Box::new(bsn! {
                    Node { flex_direction: FlexDirection::Column, row_gap: px(space::S8) }
                    Children [
                        LocalizedText::plain("dns_leak_probe_title") TextRole(Role::BodyStrong)
                        --
                        LocalizedText::plain("dns_leak_probe_desc") TextRole(Role::Caption)
                        --
                        Node { column_gap: px(space::S8) }
                        Children [
                            @{ (localized_button_scene(LocalizedText::plain("dns_leak_btn_run"), ButtonVariant::Primary, palette), bsn! { TestDnsLeakButton }) }
                            --
                            @{ (localized_button_scene(LocalizedText::plain("dns_leak_btn_retry"), ButtonVariant::Default, palette), bsn! { RetryLeakButton }) }
                        ]
                        --
                        Text::default() LeakFeedback TextRole(Role::Caption)
                    ]
                }),
                Box::new(bsn! {
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::bottom(Val::Px(space::S8)),
                                }
                                Children [
                                    Text(conclusion)
                                    DnsLine(DnsLineKind::LeakConclusion)
                                    TextRole(Role::BodyStrong)
                                    TextColor(conclusion_color)
                                    --
                                    Text(sources) DnsLine(DnsLineKind::LeakSources) TextRole(Role::Caption)
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
                                    @{ rows }
                                ]
                }),
            ],
            palette,
        ),
        bsn! { LeakCard },
    )
}

/// Per-frame safety net: restamp the leak row after a theme or projection
/// replay that did not run the observer.
pub fn sync_dns_leak_line(
    last: Option<Res<LastDnsProjection>>,
    locale: Option<Res<UiLocale>>,
    mut lines: Query<(&mut Text, &DnsLine)>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let display = project_leak(
        &projection.leak,
        locale.as_ref().map_or("zh-CN", |locale| locale.code()),
    );
    let listing = display.listing();
    let conclusion = display.conclusion;
    let sources = display.sources;
    for (mut text, line) in &mut lines {
        match line.0 {
            DnsLineKind::Leak if text.0 != listing => text.0 = listing.clone(),
            DnsLineKind::LeakConclusion if text.0 != conclusion => text.0 = conclusion.clone(),
            DnsLineKind::LeakSources if text.0 != sources => text.0 = sources.clone(),
            _ => {}
        }
    }
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
    use infiltrator_contract::dns_leak::{
        DnsLeakEchoRecord, DnsLeakObservation, DnsLeakObservationOutcome, DnsLeakProbeSource,
        DnsLeakProbeTransport, DnsLeakReport,
    };
    #[cfg(test)]
    use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
    #[cfg(test)]
    use infiltrator_contract::stun_probe::StunProbeReport;

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
        let listing = leak_observation_listing(&report, "zh-CN");
        assert!(
            listing.contains("system → whoami.ds.akahelp.net 观测身份：203.0.113.9"),
            "{listing}"
        );
        assert!(
            listing.contains("system → o-o.myaddr.l.google.com 观测身份：203.0.113.9"),
            "{listing}"
        );
        assert_eq!(
            leak_source_count_label(&report, "zh-CN"),
            "已配置探测源 2 个"
        );
        assert!(leak_conclusion_label(&report, "zh-CN").contains("2 个来源观测到同一解析器身份"));
    }

    #[test]
    fn the_card_lists_every_observed_fact_without_choosing_one() {
        let divergent = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "198.51.100.7"),
        ]);
        let label = leak_conclusion_label(&divergent, "zh-CN");
        assert!(label.contains("2 个不同解析器身份"), "{label}");
        assert!(label.contains("仅列事实"), "{label}");
        let listing = leak_observation_listing(&divergent, "zh-CN");
        assert_eq!(listing.lines().count(), 2);
        assert!(listing.contains("1.1.1.1 → a.echo.example.org 观测身份：203.0.113.9"));
        assert!(listing.contains("1.1.1.1 → b.echo.example.org 观测身份：198.51.100.7"));

        let consistent = report(&[
            ("a.echo.example.org", "203.0.113.9"),
            ("b.echo.example.org", "203.0.113.9"),
        ]);
        let label = leak_conclusion_label(&consistent, "zh-CN");
        assert!(
            label.contains("2 个来源观测到同一解析器身份 203.0.113.9"),
            "{label}"
        );
        assert!(leak_source_count_label(&consistent, "zh-CN").contains("1 个"));
    }

    #[test]
    fn a_host_without_a_fact_source_renders_the_typed_unsupported_copy() {
        let unsupported = DnsLeakReport::unsupported("no echo authority is configured");
        let label = leak_conclusion_label(&unsupported, "zh-CN");
        assert!(label.contains("宿主未提供泄漏探测事实源"), "{label}");
        assert_eq!(
            leak_observation_listing(&unsupported, "zh-CN"),
            "尚无泄漏探测观测结果"
        );
        assert_eq!(
            leak_source_count_label(&unsupported, "zh-CN"),
            "已配置探测源 0 个"
        );
        assert!(!label.contains("一致"));
        assert!(!label.contains("US"));
        assert!(!label.contains("Cloudflare"));

        // Nothing reported yet is not a verdict either.
        let pending = DnsLeakReport::default();
        let pending_label = leak_conclusion_label(&pending, "zh-CN");
        assert!(pending_label.contains("尚无交叉结论"), "{pending_label}");
        assert!(!pending_label.contains("宿主未提供"), "{pending_label}");
    }

    #[test]
    fn the_card_scene_builds_for_every_conclusion_shape() {
        let palette = UiPalette::new(&Theme::dark());
        let mut projection = DnsProjection {
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
            stun: StunProbeReport::default(),
            self_heal: DnsSelfHealSnapshot::default(),
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
