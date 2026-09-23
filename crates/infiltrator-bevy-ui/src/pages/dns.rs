//! The DNS page (域名解析): upstream nameservers, DoH / DoT / DoQ endpoints,
//! Fake-IP filter rules, and DNS cache status.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DnsLine`],
//! [`DnsServerAddress`], [`DnsServerProto`], [`DnsServerLatency`],
//! [`DnsSwitchTrack`], [`DnsBorder`], [`DnsSwitchKnob`]). The page
//! self-registers [`apply_dns_projection`] and action observers once per world
//! via [`DnsPageRoot`]. When [`DnsProjectionUpdated`] fires, texts, latency
//! inks, switches and segmented pills restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{
    DnsCacheFlushReport, DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsHostEntry,
    DnsServerTag, DnsSettingsPatch, DnsSwitchField, FakeIpMappingPool,
};
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::proxies::{format_latency, latency_color};
use crate::route::{PageRoot, Route};

/// Root marker on the DNS page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_dns_page)]
pub struct DnsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct DnsPageBound;

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
    /// DUAL-14-08: the shared cross-source DNS leak observation listing.
    Leak,
    /// DUAL-14-13: the shared DNS self-heal observation.
    SelfHeal,
    /// DUAL-14-11: the applied `dns.hosts` row count.
    HostsSummary,
}

/// Marker for a DNS server's address display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerAddress(pub usize);

/// Marker for a DNS server's protocol display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerProto(pub usize);

/// Marker for a DNS server's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsServerLatency(pub usize);

/// Marker for the "Clear DNS Cache" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearDnsCacheButton;

/// Marker for the "Test DNS Latency" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDnsLatencyButton;

/// DUAL-14-08: marker for the cross-source DNS leak probe button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDnsLeakButton;

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
    /// DUAL-14-13: the shared DNS self-heal observation.
    pub self_heal: DnsSelfHealSnapshot,
    /// DUAL-14-11: the configured `dns.hosts` rows.
    pub hosts: Vec<DnsHostEntry>,
}

impl DnsProjection {
    /// Project the shared DNS page read model into the Bevy render values.
    pub fn from_snapshot(
        snapshot: &infiltrator_contract::surface_snapshot::DnsPageSnapshot,
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
            cache_flush: snapshot.cache_flush.clone(),
            fake_ip_pool: snapshot.fake_ip_pool.clone(),
            latency: snapshot.latency.clone(),
            leak: snapshot.leak.clone(),
            self_heal: snapshot.self_heal.clone(),
            hosts: snapshot.hosts.clone(),
        }
    }
}

/// The typed event dispatched when DNS data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DnsProjectionUpdated(pub DnsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastDnsProjection(pub Option<DnsProjection>);

pub(crate) fn enhanced_mode_label(mode: DnsEnhancedMode) -> &'static str {
    match mode {
        DnsEnhancedMode::FakeIp => "Fake-IP 模式 (增强隐私与速度)",
        DnsEnhancedMode::RedirHost => "Redir-Host 模式 (真实 IP 解析)",
        DnsEnhancedMode::Unmapped => "取消映射 (None)",
    }
}

pub(crate) fn enhanced_mode_pill_label(mode: DnsEnhancedMode) -> &'static str {
    match mode {
        DnsEnhancedMode::FakeIp => "虚拟 IP (Fake-IP)",
        DnsEnhancedMode::RedirHost => "真实 IP (Redir-Host)",
        DnsEnhancedMode::Unmapped => "取消映射 (None)",
    }
}

pub(crate) fn filter_mode_label(mode: DnsFakeIpFilterMode) -> &'static str {
    match mode {
        DnsFakeIpFilterMode::Blacklist => "黑名单 (Blacklist)",
        DnsFakeIpFilterMode::Whitelist => "白名单 (Whitelist)",
        DnsFakeIpFilterMode::Rules => "规则 (Rules)",
    }
}

fn server_tag_label(tag: DnsServerTag) -> &'static str {
    match tag {
        DnsServerTag::Domestic => "国内",
        DnsServerTag::Fallback => "Fallback",
        DnsServerTag::Encrypted => "加密",
        DnsServerTag::Plain => "明文",
    }
}

fn server_tags_text(tags: &[DnsServerTag]) -> String {
    tags.iter()
        .map(|tag| server_tag_label(*tag))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Honest cache flush status line (DUAL-14-07).
pub(crate) fn cache_flush_label(report: &DnsCacheFlushReport) -> String {
    format!(
        "Fake-IP 缓存: {} · 系统 DNS 缓存: {}",
        flush_outcome_label(&report.fake_ip),
        flush_outcome_label(&report.os_cache)
    )
}

fn flush_outcome_label(outcome: &infiltrator_contract::dns::DnsFlushOutcome) -> String {
    use infiltrator_contract::dns::DnsFlushOutcome;
    match outcome {
        DnsFlushOutcome::NotRequested => "尚未执行".to_owned(),
        DnsFlushOutcome::Flushed => "已清空".to_owned(),
        DnsFlushOutcome::Unsupported { reason } => format!("宿主不支持 ({reason})"),
        DnsFlushOutcome::Failed { message } => format!("清理失败 ({message})"),
    }
}

// ---- Scene constructors ---------------------------------------------------

pub fn dns_page(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "域名解析 · {} (缓存条目: {})",
        enhanced_mode_label(projection.mode),
        projection.cache_entries
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
        Children [
            ( { header_card_scene(summary, projection, palette) } ),
            ( { crate::pages::dns_form::dns_form_card_scene(projection, palette) } ),
            ( { crate::pages::dns_edit::dns_edit_card_scene(projection, palette) } ),
            ( { crate::pages::dns_hosts::dns_hosts_card_scene(projection, palette) } ),
            ( { crate::pages::dns_fakeip::dns_fakeip_pool_card_scene(projection, palette) } ),
            ( { crate::pages::dns_leak::dns_leak_card_scene(projection, palette) } ),
            ( { crate::pages::dns_self_heal::dns_self_heal_card_scene(projection, palette) } ),
            ( { servers_card_scene(server_scenes, &projection.latency, palette) } ),
            ( { fake_ip_card_scene(&projection.fake_ip_range, palette) } ),
        ]
    }
}

fn header_card_scene(
    summary: String,
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("DNS 解析概览");
    let flush_label = cache_flush_label(&projection.cache_flush);

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            template_value(AccessibilityNode(header_a11y))
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( { icon_tile_scene(IconId::Network, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) DnsLine(DnsLineKind::Summary) TextRole(Role::Heading) ),
                                (
                                    Text(flush_label)
                                    DnsLine(DnsLineKind::CacheFlush)
                                    TextRole(Role::Caption)
                                ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        (
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
                                ( Text({ "测速".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            TestDnsLeakButton
                            Button
                            Children [
                                ( Text({ "泄漏交叉探测".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                        (
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
                                ( Text({ "清空 DNS 缓存".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn servers_card_scene(
    server_scenes: Vec<Box<dyn Scene>>,
    latency: &DnsLatencyReport,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let latency_label = crate::pages::dns_fakeip::latency_policy_label(latency);
    let latency_results = crate::pages::dns_fakeip::latency_result_listing(latency);

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
                    ( Text({ "上游加密 DNS 服务器 (Nameservers)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "支持 DoH / DoT / DoQ".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Text(latency_label)
                        DnsLine(DnsLineKind::LatencyPolicy)
                        TextRole(Role::Caption)
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    min_height: px(32.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Text(latency_results)
                        DnsLine(DnsLineKind::LatencyResults)
                        TextRole(Role::Mono)
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { server_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn server_row_scene(idx: usize, server: &DnsServerItem, palette: &UiPalette) -> impl Scene + use<> {
    let addr = server.address.clone();
    let proto = server.protocol.clone();
    let fallback_badge = if server.is_fallback {
        " [Fallback]"
    } else {
        ""
    };
    let proto_str = format!("{proto}{fallback_badge}");
    let tag_str = server_tags_text(&server.tags);
    let (lat_str, tier) = format_latency(server.latency_ms);
    let lat_col = latency_color(tier, palette);

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text(addr) DnsServerAddress(idx) TextRole(Role::BodyStrong) ),
                    ( Text(proto_str) DnsServerProto(idx) TextRole(Role::Caption) ),
                    ( Text(tag_str) DnsLine(DnsLineKind::ServerTags(idx)) TextRole(Role::Caption) ),
                ]
            ),
            (
                Text(lat_str)
                DnsServerLatency(idx)
                TextRole(Role::Mono)
                TextColor(lat_col)
            ),
        ]
    }
}

fn fake_ip_card_scene(fake_ip_range: &str, palette: &UiPalette) -> impl Scene + use<> {
    let range_str = format!("分配网段: {fake_ip_range}");

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text({ "Fake-IP 高级设置 (Fake-IP Filter & Pool)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                    ( Text(range_str) DnsLine(DnsLineKind::FakeIpRange) TextRole(Role::Body) ),
                    ( Text({ "过滤域名: *.lan, localhost".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
        ],
        palette,
    )
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_dns_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<DnsPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(DnsPageBound);
    commands.add_observer(apply_dns_projection);
    commands.add_observer(on_dns_action_activated);
    commands.add_observer(crate::pages::dns_edit::apply_dns_edit_projection);
    commands.add_observer(crate::pages::dns_edit::on_dns_edit_activated);
    commands.add_observer(crate::pages::dns_hosts::apply_dns_hosts_projection);
    commands.add_observer(crate::pages::dns_hosts::on_dns_hosts_activated);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_dns_action_activated(
    activate: On<Activate>,
    clear_buttons: Query<(), With<ClearDnsCacheButton>>,
    test_buttons: Query<(), With<TestDnsLatencyButton>>,
    leak_buttons: Query<(), With<TestDnsLeakButton>>,
    switch_buttons: Query<&DnsSwitchButton>,
    enhanced_pills: Query<&DnsEnhancedModePill>,
    filter_pills: Query<&DnsFilterModePill>,
    handle: Option<Res<CommandSinkHandle>>,
    last: Option<Res<LastDnsProjection>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if clear_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ClearDnsCache);
        return;
    }
    if test_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestDnsLatency);
        return;
    }
    if leak_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestDnsLeak);
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn apply_dns_projection(
    update: On<DnsProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastDnsProjection>>,
    mut lines: Query<
        (&mut Text, &mut TextColor, &DnsLine),
        (
            With<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerProto>,
            Without<DnsServerLatency>,
        ),
    >,
    mut addresses: Query<
        (&mut Text, &DnsServerAddress),
        (
            With<DnsServerAddress>,
            Without<DnsLine>,
            Without<DnsServerProto>,
            Without<DnsServerLatency>,
        ),
    >,
    mut protocols: Query<
        (&mut Text, &DnsServerProto),
        (
            With<DnsServerProto>,
            Without<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerLatency>,
        ),
    >,
    mut latencies: Query<
        (&mut Text, &mut TextColor, &DnsServerLatency),
        (
            With<DnsServerLatency>,
            Without<DnsLine>,
            Without<DnsServerAddress>,
            Without<DnsServerProto>,
        ),
    >,
    mut switch_tracks: Query<
        (
            &mut BackgroundColor,
            &mut BorderColor,
            &mut DnsSwitchButton,
            &DnsSwitchTrack,
        ),
        (
            With<DnsSwitchTrack>,
            Without<DnsSwitchKnob>,
            Without<DnsEnhancedModePill>,
            Without<DnsFilterModePill>,
        ),
    >,
    mut switch_knobs: Query<
        (&mut BackgroundColor, &mut Node, &DnsSwitchKnob),
        (
            With<DnsSwitchKnob>,
            Without<DnsSwitchTrack>,
            Without<DnsEnhancedModePill>,
            Without<DnsFilterModePill>,
        ),
    >,
    mut enhanced_pills: Query<
        (&mut BackgroundColor, &DnsEnhancedModePill),
        (
            With<DnsEnhancedModePill>,
            Without<DnsSwitchTrack>,
            Without<DnsSwitchKnob>,
            Without<DnsFilterModePill>,
        ),
    >,
    mut filter_pills: Query<
        (&mut BackgroundColor, &DnsFilterModePill),
        (
            With<DnsFilterModePill>,
            Without<DnsSwitchTrack>,
            Without<DnsSwitchKnob>,
            Without<DnsEnhancedModePill>,
        ),
    >,
) {
    let projection = &update.0;

    for (mut text, mut color, line) in &mut lines {
        match line.0 {
            DnsLineKind::Summary => {
                text.0 = format!(
                    "域名解析 · {} (缓存条目: {})",
                    enhanced_mode_label(projection.mode),
                    projection.cache_entries
                );
            }
            DnsLineKind::FakeIpRange => {
                text.0 = format!("分配网段: {}", projection.fake_ip_range);
            }
            DnsLineKind::CacheFlush => {
                text.0 = cache_flush_label(&projection.cache_flush);
            }
            DnsLineKind::FakeIpMapping => {
                text.0 =
                    crate::pages::dns_fakeip::fake_ip_mapping_listing(&projection.fake_ip_pool, "");
            }
            DnsLineKind::FakeIpMappingCount => {
                text.0 =
                    crate::pages::dns_fakeip::fake_ip_mapping_count(&projection.fake_ip_pool, "");
            }
            DnsLineKind::LatencyPolicy => {
                text.0 = crate::pages::dns_fakeip::latency_policy_label(&projection.latency);
                color.0 = match &projection.latency.status {
                    infiltrator_contract::dns_latency::DnsLatencyStatus::Ready => palette.success,
                    infiltrator_contract::dns_latency::DnsLatencyStatus::Unsupported { .. } => {
                        palette.ink_dim
                    }
                };
            }
            DnsLineKind::LatencyResults => {
                text.0 = crate::pages::dns_fakeip::latency_result_listing(&projection.latency);
            }
            DnsLineKind::LeakConclusion => {
                text.0 = crate::pages::dns_leak::leak_conclusion_label(&projection.leak);
                color.0 = crate::pages::dns_leak::leak_conclusion_color(&projection.leak, &palette);
            }
            DnsLineKind::Leak => {
                text.0 = crate::pages::dns_leak::leak_observation_listing(&projection.leak);
            }
            DnsLineKind::SelfHeal => {
                text.0 = crate::pages::dns_fakeip::self_heal_listing(&projection.self_heal);
                color.0 = match projection.self_heal.overall_state() {
                    infiltrator_contract::dns_self_heal::DnsSelfHealState::Healthy => {
                        palette.success
                    }
                    infiltrator_contract::dns_self_heal::DnsSelfHealState::Warning => {
                        palette.warning
                    }
                    infiltrator_contract::dns_self_heal::DnsSelfHealState::Critical => {
                        palette.danger
                    }
                    infiltrator_contract::dns_self_heal::DnsSelfHealState::Unknown => {
                        palette.ink_dim
                    }
                };
            }
            DnsLineKind::HostsSummary => {
                text.0 = crate::pages::dns_hosts::hosts_summary_label(&projection.hosts);
            }
            DnsLineKind::SwitchStatus(field) => {
                let enabled = projection.switches.value(field);
                text.0 = if enabled { "已开启" } else { "已关闭" }.to_owned();
                color.0 = if enabled {
                    palette.success
                } else {
                    palette.ink_dim
                };
            }
            DnsLineKind::ServerTags(idx) => {
                if let Some(server) = projection.servers.get(idx) {
                    text.0 = server_tags_text(&server.tags);
                }
            }
            DnsLineKind::EnhancedModeLabel(mode) => {
                text.0 = enhanced_mode_pill_label(mode).to_owned();
                color.0 = if mode == projection.mode {
                    palette.on_accent
                } else {
                    palette.ink_dim
                };
            }
            DnsLineKind::FilterModeLabel(mode) => {
                text.0 = filter_mode_label(mode).to_owned();
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

    for (mut text, mut color, marker) in &mut latencies {
        if let Some(server) = projection.servers.get(marker.0) {
            let (str_val, tier) = format_latency(server.latency_ms);
            text.0 = str_val;
            color.0 = latency_color(tier, &palette);
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
