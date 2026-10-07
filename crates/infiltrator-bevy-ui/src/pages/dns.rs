//! The DNS page (域名解析): upstream nameservers, DoH / DoT / DoQ endpoints,
//! Fake-IP filter rules, and DNS cache status.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DnsLine`],
//! [`DnsServerAddress`], [`DnsServerProto`], [`DnsServerLatency`],
//! [`DnsSwitchTrack`], [`DnsBorder`], [`DnsSwitchKnob`]). [`DnsPagePlugin`]
//! registers [`apply_dns_projection`] and action observers once at product
//! assembly. When [`DnsProjectionUpdated`] fires, texts, latency
//! inks, switches and segmented pills restamp in place without tree rebuilds.

#[path = "dns_query_access.rs"]
pub mod query_access;
use self::query_access::{DnsActionControls, DnsProjectionTargets};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::dns_edit::{
    DnsFormState, apply_dns_edit_projection, dns_edit_card_scene, on_dns_edit_activated,
};
use crate::pages::dns_fakeip::dns_fakeip_pool_card_scene;
use crate::pages::dns_form::dns_form_card_scene;
use crate::pages::dns_hosts::dns_hosts_card_scene;
use crate::pages::dns_leak::{dns_leak_card_scene, tone_color};
use crate::pages::dns_query::QueryAction;
use crate::pages::dns_query_scene::query_button;
use crate::pages::dns_self_heal::{dns_self_heal_card_scene, observation_color};
use crate::pages::dns_servers::{server_row_scene, servers_card_scene};
use crate::pages::dns_stun::dns_stun_card_scene;
use crate::pages::proxies::latency_color;
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::dns_health_projection::project_dns_health;
use infiltrator_application::dns_latency_projection::{project_dns_latency, server_tags_text};
use infiltrator_application::dns_leak_projection::project_leak;
use infiltrator_application::dns_status_projection::{
    filter_key, flush_summary, heading, mode_key,
};
use infiltrator_application::latency_projection::project_measured_latency;
use infiltrator_application::stun_projection::project_stun;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsServerTag, DnsSettingsPatch,
    DnsSwitchField, FakeIpMappingPool,
};
use infiltrator_contract::dns_cache::DnsCacheFlushReport;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_hosts::{DnsHostEntry, DnsHostsProfile};
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::surface_snapshot::DnsPageSnapshot;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

/// Root marker on the DNS page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DnsPageRoot;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsLine(pub DnsLineKind);

/// Different text lines on the DNS page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DnsLineKind {
    /// Overview summary: DNS mode and cache count.
    #[default]
    Summary,
    /// Fake-IP range display.
    FakeIpRange,
    /// One of the six switch status words.
    SwitchStatus(DnsSwitchField),
    /// The semantic tag chip row of one server.
    ServerTags(usize),
    /// The label ink of one enhanced-mode pill.
    EnhancedModeLabel(DnsEnhancedMode),
    /// The label ink of one filter-mode pill.
    FilterModeLabel(DnsFakeIpFilterMode),
    /// The honest last DNS cache flush report line.
    CacheFlush,
    /// DUAL-14-06: the filtered Fake-IP binding listing (multi-line text).
    FakeIpMapping,
    /// DUAL-14-06: the shown/total counter of the mapping listing.
    FakeIpMappingCount,
    /// DUAL-14-10: the honest per-nameserver latency policy line.
    LatencyPolicy,
    /// DUAL-14-10: the per-nameserver result rows of the last real probe.
    LatencyResults,
    /// DUAL-14-08: the shared cross-source DNS leak conclusion headline.
    LeakConclusion,
    LeakSources,
    /// DUAL-14-08: the shared cross-source DNS leak observation listing.
    Leak,
    /// DUAL-14-09 (re-scoped): the STUN UDP-egress conclusion headline.
    StunConclusion,
    /// DUAL-14-09 (re-scoped): the STUN UDP-egress mapping listing.
    Stun,
    /// DUAL-14-13: the shared DNS self-heal observation.
    SelfHeal,
    SelfHealOverall,
}

/// Marker for the "Clear DNS Cache" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearDnsCacheButton;

/// Marker for the "Test DNS Latency" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDnsLatencyButton;

/// DUAL-14-08: marker for the cross-source DNS leak probe button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct TestDnsLeakButton;

/// DUAL-14-09 (re-scoped): marker for the STUN UDP-egress probe button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestStunProbeButton;

/// Marker component for the DNS 6-switch form card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsConfigCard;

/// Marker component on a DNS switch button, carrying its rendered state.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsSwitchButton(pub DnsSwitchField, pub bool);

/// Marker component for Domain Mapping Mode segmented control root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEnhancedModeControl;

/// Marker component for Filter Mode segmented control root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFilterModeControl;

/// The background track of one DNS switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsSwitchTrack(pub DnsSwitchField);

/// The background knob of one DNS switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsSwitchKnob(pub DnsSwitchField);

/// A Domain Mapping Mode segmented pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEnhancedModePill(pub DnsEnhancedMode);

/// A Filter Mode segmented pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsFilterModePill(pub DnsFakeIpFilterMode);

/// The border-coloured track of one DNS switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsBorder(pub DnsSwitchField);

/// A DNS upstream nameserver item.
#[derive(Clone, Debug, PartialEq)]
pub struct DnsServerItem {
    pub address: String,
    pub protocol: String,
    pub latency_ms: Option<u32>,
    pub is_fallback: bool,
    pub tags: Vec<DnsServerTag>,
}

/// Snapshot of the DNS domain.
#[derive(Clone, Debug, PartialEq)]
pub struct DnsProjection {
    pub mode: DnsEnhancedMode,
    pub cache_entries: usize,
    pub fake_ip_range: String,
    pub servers: Vec<DnsServerItem>,
    pub switches: DnsCoreSwitches,
    pub filter_mode: DnsFakeIpFilterMode,
    /// Shared workbench draft (upstream lists, fallback policy, Fake-IP).
    pub form: DnsWorkbenchForm,
    /// Honest last Fake-IP / OS cache flush report.
    pub cache_flush: DnsCacheFlushReport,
    /// DUAL-14-06: observed Fake-IP bindings from the shared read model.
    pub fake_ip_pool: FakeIpMappingPool,
    /// DUAL-14-10: the last real per-nameserver probe of this host.
    pub latency: DnsLatencyReport,
    /// DUAL-14-08: the last real cross-source DNS leak probe of this host.
    pub leak: DnsLeakReport,
    /// DUAL-14-09 (re-scoped): the last real STUN UDP-egress probe of this
    /// host, compared against the expected proxied egress.
    pub stun: StunProbeReport,
    /// DUAL-14-13: the shared DNS self-heal observation.
    pub self_heal: DnsSelfHealSnapshot,
    /// DUAL-14-11: the configured `dns.hosts` rows.
    pub hosts: Vec<DnsHostEntry>,
}

impl DnsProjection {
    /// Project the shared DNS page read model into the Bevy render values.
    pub fn from_snapshot(
        snapshot: &DnsPageSnapshot,
        leak: &DnsLeakReport,
        hosts: Option<&DnsHostsProfile>,
        cache: &DnsCacheFlushReport,
    ) -> Self {
        Self {
            mode: snapshot.enhanced_mode,
            cache_entries: snapshot.cache_entries,
            fake_ip_range: snapshot.fake_ip_range.clone(),
            switches: snapshot.switches,
            filter_mode: snapshot.filter_mode,
            servers: snapshot
                .servers
                .iter()
                .map(|server| DnsServerItem {
                    address: server.address.clone(),
                    protocol: server.protocol.clone(),
                    latency_ms: server.latency_ms,
                    is_fallback: server.is_fallback,
                    tags: server.tags.clone(),
                })
                .collect(),
            form: DnsWorkbenchForm::from_snapshot(snapshot),
            cache_flush: cache.clone(),
            fake_ip_pool: snapshot.fake_ip_pool.clone(),
            latency: snapshot.latency.clone(),
            leak: leak.clone(),
            stun: snapshot.stun.clone(),
            self_heal: snapshot.self_heal.clone(),
            hosts: hosts.map(|hosts| hosts.entries.clone()).unwrap_or_default(),
        }
    }
}

/// The typed event dispatched when DNS data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DnsProjectionUpdated(pub DnsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastDnsProjection(pub Option<DnsProjection>);

pub(crate) fn enhanced_mode_pill_label(mode: DnsEnhancedMode, code: &str) -> String {
    Lang(code).tr(mode_key(mode)).into_owned()
}
pub(crate) fn filter_mode_label(mode: DnsFakeIpFilterMode, code: &str) -> String {
    Lang(code).tr(filter_key(mode)).into_owned()
}
pub(crate) fn cache_flush_label(report: &DnsCacheFlushReport, code: &str) -> String {
    flush_summary(report, code)
}

// ---- Scene constructors ---------------------------------------------------

pub fn dns_page(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = heading(
        projection.mode,
        projection.cache_entries,
        UiLocale::default().code(),
    );

    let server_scenes: Vec<Box<dyn Scene>> = projection
        .servers
        .iter()
        .enumerate()
        .map(|(idx, s)| Box::new(server_row_scene(idx, s, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Dns)
            DnsPageRoot
            ScrollArea
            Children [
                @{ query_button(QueryAction::Open, LocalizedText::plain("dns_query_open"), palette) }
                --
                @{ header_card_scene(summary, projection, palette) }
                --
                @{ dns_leak_card_scene(projection, palette) }
                --
                @{ dns_form_card_scene(projection, palette) }
                --
                @{ dns_edit_card_scene(projection, palette) }
                --
                @{ dns_hosts_card_scene(palette) }
                --
                @{ dns_fakeip_pool_card_scene(projection, palette) }
                --
                @{ dns_stun_card_scene(projection, palette) }
                --
                @{ dns_self_heal_card_scene(projection, palette) }
                --
                @{ servers_card_scene(server_scenes, &projection.latency, palette) }
                --
                @{ fake_ip_card_scene(&projection.fake_ip_range, palette) }
            ]
    }
}

fn header_card_scene(
    summary: String,
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label(
        Lang(UiLocale::default().code())
            .tr("dns_overview_label")
            .into_owned(),
    );
    let flush_label = cache_flush_label(&projection.cache_flush, UiLocale::default().code());

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("dns_overview_label")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Network, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                Text(summary) DnsLine(DnsLineKind::Summary) TextRole(Role::Heading)
                                --
                                Text(flush_label)
                                DnsLine(DnsLineKind::CacheFlush)
                                TextRole(Role::Caption)
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            TestDnsLatencyButton
                            Button
                            Children [
                                LocalizedText::plain("runtime_delay_test_one") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            TestStunProbeButton
                            Button
                            Children [
                                LocalizedText::plain("dns_stun_egress_action") TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            ClearDnsCacheButton
                            Button
                            Children [
                                LocalizedText::plain("dns_clear_cache_action") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

fn fake_ip_card_scene(fake_ip_range: &str, palette: &UiPalette) -> impl Scene + use<> {
    let range_str = localize(
        UiLocale::default().code(),
        "dns_allocation_range",
        &[("range", fake_ip_range.into())],
    );

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("dns_fake_ip_advanced_title") TextRole(Role::BodyStrong)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::all(Val::Px(space::S8)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Children [
                                Text(range_str) DnsLine(DnsLineKind::FakeIpRange) TextRole(Role::Body)
                                --
                                LocalizedText::plain("dns_fake_ip_filter_hint") TextRole(Role::Caption)
                            ]
            }),
        ],
        palette,
    )
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct DnsPagePlugin;

impl Plugin for DnsPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DnsFormState>();
        app.add_observer(apply_dns_projection);
        app.add_observer(on_dns_action_activated);
        app.add_observer(apply_dns_edit_projection);
        app.add_observer(on_dns_edit_activated);
    }
}

pub(crate) fn on_dns_action_activated(
    activate: On<Activate>,
    handle: Option<Res<CommandSinkHandle>>,
    last: Option<Res<LastDnsProjection>>,
    targets: DnsActionControls,
) {
    let DnsActionControls {
        test_buttons,
        stun_buttons,
        switch_buttons,
        enhanced_pills,
        filter_pills,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if test_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestDnsLatency);
        return;
    }
    if stun_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RunStunProbe);
        return;
    }
    if let Ok(btn) = switch_buttons.get(activate.entity) {
        let mut switches = last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .map(|projection| projection.switches)
            .unwrap_or_default();
        switches.set(btn.0, !btn.1);
        handle.submit(UiCommand::ApplyDnsSettings {
            patch: DnsSettingsPatch {
                switches: Some(switches),
                ..DnsSettingsPatch::default()
            },
        });
        return;
    }
    if let Ok(pill) = enhanced_pills.get(activate.entity) {
        handle.submit(UiCommand::ApplyDnsSettings {
            patch: DnsSettingsPatch {
                enhanced_mode: Some(pill.0),
                ..DnsSettingsPatch::default()
            },
        });
        return;
    }
    if let Ok(pill) = filter_pills.get(activate.entity) {
        handle.submit(UiCommand::ApplyDnsSettings {
            patch: DnsSettingsPatch {
                filter_mode: Some(pill.0),
                ..DnsSettingsPatch::default()
            },
        });
    }
}

pub(crate) fn apply_dns_projection(
    update: On<DnsProjectionUpdated>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    mut last: Option<ResMut<LastDnsProjection>>,
    targets: DnsProjectionTargets,
) {
    let DnsProjectionTargets {
        mut lines,
        mut addresses,
        mut protocols,
        mut latencies,
        mut switch_tracks,
        mut switch_knobs,
        mut enhanced_pills,
        mut filter_pills,
    } = targets;

    let projection = &update.0;

    let leak = project_leak(&projection.leak, locale.code());
    let latency = project_dns_latency(&projection.latency, locale.code());
    let health = project_dns_health(&projection.self_heal, locale.code());
    let stun = project_stun(&projection.stun, locale.code());
    for (mut text, mut color, line) in &mut lines {
        match line.0 {
            DnsLineKind::Summary => {
                text.0 = heading(projection.mode, projection.cache_entries, locale.code());
            }
            DnsLineKind::FakeIpRange => {
                text.0 = localize(
                    locale.code(),
                    "dns_allocation_range",
                    &[("range", projection.fake_ip_range.clone())],
                );
            }
            DnsLineKind::CacheFlush => {
                text.0 = cache_flush_label(&projection.cache_flush, locale.code());
            }
            // The filter system owns these texts, preserving the current input.
            DnsLineKind::FakeIpMapping | DnsLineKind::FakeIpMappingCount => {}
            DnsLineKind::LatencyPolicy => {
                text.0 = latency.summary.clone();
                color.0 = observation_color(latency.tone, &palette);
            }
            DnsLineKind::LatencyResults => {
                text.0 = latency.listing();
            }
            DnsLineKind::LeakConclusion => {
                text.0 = leak.conclusion.clone();
                color.0 = tone_color(leak.tone, &palette);
            }
            DnsLineKind::Leak => {
                text.0 = leak.listing();
            }
            DnsLineKind::LeakSources => {
                text.0 = leak.sources.clone();
            }
            DnsLineKind::StunConclusion => {
                text.0 = stun.status.clone();
                color.0 = observation_color(stun.tone, &palette);
            }
            DnsLineKind::Stun => {
                text.0 = stun.listing();
            }
            DnsLineKind::SelfHeal => {
                text.0 = health.listing();
                color.0 = observation_color(health.tone, &palette);
            }
            DnsLineKind::SelfHealOverall => {
                text.0 = health.overall.clone();
                color.0 = observation_color(health.tone, &palette);
            }
            DnsLineKind::SwitchStatus(field) => {
                let enabled = projection.switches.value(field);
                text.0 = Lang(locale.code())
                    .tr(if enabled {
                        "dns_switch_enabled"
                    } else {
                        "dns_switch_disabled"
                    })
                    .into_owned();
                color.0 = if enabled {
                    palette.success
                } else {
                    palette.ink_dim
                };
            }
            DnsLineKind::ServerTags(idx) => {
                if let Some(server) = projection.servers.get(idx) {
                    text.0 = server_tags_text(&server.tags, locale.code());
                }
            }
            DnsLineKind::EnhancedModeLabel(mode) => {
                text.0 = enhanced_mode_pill_label(mode, locale.code());
                color.0 = if mode == projection.mode {
                    palette.on_accent
                } else {
                    palette.ink_dim
                };
            }
            DnsLineKind::FilterModeLabel(mode) => {
                text.0 = filter_mode_label(mode, locale.code());
                color.0 = if mode == projection.filter_mode {
                    palette.on_accent
                } else {
                    palette.ink_dim
                };
            }
        }
    }

    for (mut text, marker) in &mut addresses {
        if let Some(server) = projection.servers.get(marker.0) {
            text.0 = server.address.clone();
        }
    }

    for (mut text, marker) in &mut protocols {
        if let Some(server) = projection.servers.get(marker.0) {
            let fallback_badge = if server.is_fallback {
                " [Fallback]"
            } else {
                ""
            };
            text.0 = format!("{}{fallback_badge}", server.protocol);
        }
    }

    for (mut text, mut color, marker, mut copy) in &mut latencies {
        if let Some(server) = projection.servers.get(marker.0) {
            let latency = project_measured_latency(server.latency_ms);
            *copy = LocalizedText::new(latency.key, latency.params);
            text.0 = copy.render(&locale);
            color.0 = latency_color(latency.band, &palette);
        }
    }

    for (mut background, mut border, mut button, marker) in &mut switch_tracks {
        let enabled = projection.switches.value(marker.0);
        button.1 = enabled;
        background.0 = if enabled {
            palette.accent
        } else {
            palette.surface_elevated
        };
        let edge = if enabled {
            palette.accent
        } else {
            palette.border
        };
        border.top = edge;
        border.right = edge;
        border.bottom = edge;
        border.left = edge;
    }

    for (mut background, mut node, marker) in &mut switch_knobs {
        let enabled = projection.switches.value(marker.0);
        background.0 = if enabled {
            palette.on_accent
        } else {
            palette.ink_dim
        };
        node.left = if enabled { Val::Px(18.0) } else { Val::Px(2.0) };
    }

    for (mut background, marker) in &mut enhanced_pills {
        background.0 = if marker.0 == projection.mode {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    for (mut background, marker) in &mut filter_pills {
        background.0 = if marker.0 == projection.filter_mode {
            palette.accent
        } else {
            palette.surface_elevated
        };
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}
