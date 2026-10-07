//! DUAL-14-13: the Bevy DNS self-heal card.
//!
//! The card renders the shared `DnsSelfHealSnapshot`: one row per observed
//! check, each carrying the host's real detail and the typed suggested fix.
//! A check the host could not observe stays「未观测」and is never dressed up as
//! healthy. The texts are restamped in place by `apply_dns_projection`
//! ([`DnsLineKind::SelfHeal`]), so the card never rebuilds the tree.

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Query, Res};
use bevy::prelude::Color;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::dns_health_projection::project_dns_health;
use infiltrator_application::dns_latency_projection::{project_dns_latency, server_tags_text};
use infiltrator_application::dns_observation_projection::DnsObservationTone;
use infiltrator_application::stun_projection::project_stun;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// The DNS self-heal card (overall state + one row per check).
pub fn dns_self_heal_card_scene(
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let display = project_dns_health(&projection.self_heal, UiLocale::default().code());
    let overall_label = display.overall.clone();
    let listing = display.listing();
    let overall_color = observation_color(display.tone, palette);

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
                                LocalizedText::plain("dns_self_heal_title") TextRole(Role::BodyStrong)
                                --
                                Text(overall_label) DnsLine(DnsLineKind::SelfHealOverall) TextRole(Role::BodyStrong) TextColor(overall_color)
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
                                DnsLine(DnsLineKind::SelfHeal)
                                TextRole(Role::Mono)
                            ]
            }),
        ],
        palette,
    )
}

pub(crate) fn observation_color(tone: DnsObservationTone, palette: &UiPalette) -> Color {
    match tone {
        DnsObservationTone::Success => palette.success,
        DnsObservationTone::Warning => palette.warning,
        DnsObservationTone::Danger => palette.danger,
        DnsObservationTone::Neutral => palette.ink_dim,
    }
}

/// Replay shared observation labels after locale/theme changes without rebuilding entities.
pub fn sync_dns_observation_lines(
    last: Option<Res<LastDnsProjection>>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut lines: Query<(&mut Text, &mut TextColor, &DnsLine)>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let display = project_dns_health(&projection.self_heal, locale.code());
    let listing = display.listing();
    let latency = project_dns_latency(&projection.latency, locale.code());
    let latency_listing = latency.listing();
    let stun = project_stun(&projection.stun, locale.code());
    for (mut text, mut color, line) in &mut lines {
        let (wanted, tone) = match line.0 {
            DnsLineKind::StunConclusion => (stun.status.clone(), stun.tone),
            DnsLineKind::Stun => (stun.listing(), DnsObservationTone::Neutral),
            DnsLineKind::SelfHeal => (listing.clone(), display.tone),
            DnsLineKind::SelfHealOverall => (display.overall.clone(), display.tone),
            DnsLineKind::LatencyPolicy => (latency.summary.clone(), latency.tone),
            DnsLineKind::LatencyResults => (latency_listing.clone(), DnsObservationTone::Neutral),
            DnsLineKind::ServerTags(idx) => {
                let Some(server) = projection.servers.get(idx) else {
                    continue;
                };
                (
                    server_tags_text(&server.tags, locale.code()),
                    DnsObservationTone::Neutral,
                )
            }
            _ => continue,
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
        let wanted_color = observation_color(tone, &palette);
        if color.0 != wanted_color {
            color.0 = wanted_color;
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
    #[cfg(test)]
    use infiltrator_contract::dns_leak::DnsLeakReport;
    use infiltrator_contract::dns_self_heal::{
        DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
    };
    #[cfg(test)]
    use infiltrator_contract::stun_probe::StunProbeReport;

    fn projection(snapshot: DnsSelfHealSnapshot) -> DnsProjection {
        DnsProjection {
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
            self_heal: snapshot,
            hosts: Vec::new(),
        }
    }

    #[test]
    fn the_card_carries_the_shared_overall_state_and_every_check() {
        let snapshot = DnsSelfHealSnapshot::new(vec![
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::ListenPort,
                state: DnsSelfHealState::Healthy,
                detail: "dns.listen port 1053 is available".to_owned(),
                fix: None,
            },
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::UpstreamResolution,
                state: DnsSelfHealState::Critical,
                detail: "none of the 2 upstreams answered; resolution is unreachable".to_owned(),
                fix: Some(DnsSelfHealFix::RecheckUpstreams),
            },
        ]);
        let projection = projection(snapshot);
        assert_eq!(
            project_dns_health(&projection.self_heal, "zh-CN").overall,
            "故障"
        );
        let palette = UiPalette::new(&Theme::dark());
        let _ = dns_self_heal_card_scene(&projection, &palette);
    }

    #[test]
    fn an_unobserved_snapshot_renders_unknown() {
        let projection = projection(DnsSelfHealSnapshot::default());
        assert_eq!(
            project_dns_health(&projection.self_heal, "zh-CN").overall,
            "未观测"
        );
        let palette = UiPalette::new(&Theme::dark());
        let _ = dns_self_heal_card_scene(&projection, &palette);
    }
}
