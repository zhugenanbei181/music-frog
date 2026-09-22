//! DUAL-14-13: the Bevy DNS self-heal card.
//!
//! The card renders the shared `DnsSelfHealSnapshot`: one row per observed
//! check, each carrying the host's real detail and the typed suggested fix.
//! A check the host could not observe stays「未观测」and is never dressed up as
//! healthy. The texts are restamped in place by `apply_dns_projection`
//! ([`DnsLineKind::SelfHeal`]), so the card never rebuilds the tree.

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

use crate::pages::dns::{DnsLine, DnsLineKind, DnsProjection, LastDnsProjection};
use crate::pages::dns_fakeip::{self_heal_listing, self_heal_state_label};

/// The DNS self-heal card (overall state + one row per check).
pub fn dns_self_heal_card_scene(
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let overall = self_heal_state_label(projection.self_heal.overall_state());
    let overall_label = overall.to_owned();
    let listing = self_heal_listing(&projection.self_heal);
    let overall_color = state_color(projection.self_heal.overall_state(), palette);

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
                    ( Text({ "DNS 故障自愈检测 (DUAL-14-13)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text(overall_label) TextRole(Role::BodyStrong) TextColor(overall_color) ),
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
                        DnsLine(DnsLineKind::SelfHeal)
                        TextRole(Role::Mono)
                    ),
                ]
            }),
        ],
        palette,
    )
}

fn state_color(
    state: infiltrator_contract::dns_self_heal::DnsSelfHealState,
    palette: &UiPalette,
) -> bevy::prelude::Color {
    use infiltrator_contract::dns_self_heal::DnsSelfHealState;
    match state {
        DnsSelfHealState::Healthy => palette.success,
        DnsSelfHealState::Warning => palette.warning,
        DnsSelfHealState::Critical => palette.danger,
        DnsSelfHealState::Unknown => palette.ink_dim,
    }
}

/// Per-frame safety net: restamp the self-heal row after a theme or
/// projection replay that did not run the observer.
pub fn sync_dns_self_heal_line(
    last: Option<Res<LastDnsProjection>>,
    mut lines: Query<(&mut Text, &DnsLine)>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let listing = self_heal_listing(&projection.self_heal);
    for (mut text, line) in &mut lines {
        if line.0 == DnsLineKind::SelfHeal && text.0 != listing {
            text.0 = listing.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_self_heal::{
        DnsSelfHealCheck, DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
    };

    fn projection(snapshot: DnsSelfHealSnapshot) -> DnsProjection {
        DnsProjection {
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
            self_heal_state_label(projection.self_heal.overall_state()),
            "故障"
        );
        let palette = UiPalette::new(&infiltrator_bevy_widgets::theme::Theme::dark());
        let _ = dns_self_heal_card_scene(&projection, &palette);
    }

    #[test]
    fn an_unobserved_snapshot_renders_unknown() {
        let projection = projection(DnsSelfHealSnapshot::default());
        assert_eq!(
            self_heal_state_label(projection.self_heal.overall_state()),
            "未观测"
        );
        let palette = UiPalette::new(&infiltrator_bevy_widgets::theme::Theme::dark());
        let _ = dns_self_heal_card_scene(&projection, &palette);
    }
}
