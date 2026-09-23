//! The Rules page (分流规则): ruleset table, MRS/geosite providers,
//! rule tracer, and hit statistics.
//!
//! **Update seam**: mutable nodes carry typed markers ([`RulesLine`],
//! [`RuleHitText`], [`RuleProxyText`], [`RulePayloadText`], [`RuleTypeText`],
//! [`ProviderNameText`], [`ProviderCountText`], [`ProviderUpdatedText`]).
//! The page self-registers [`apply_rules_projection`] and action observers
//! once per world via [`RulesPageRoot`]. When [`RulesProjectionUpdated`]
//! fires, texts and hit counts restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::{SurfacePanel, surface_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::rules_edit::{RuleMoveDownButton, RuleMoveUpButton, RuleToggleButton};
use crate::pages::rules_projection::{
    ProviderCountText, ProviderNameText, ProviderUpdatedText, RuleHitText, RulePayloadText,
    RuleProxyText, RuleTypeBadge, RuleTypeText, RulesLine, RulesLineKind, truncation_label,
};
use crate::pages::rules_view::{
    RuleRow, RuleSearchField, RulesListScrollArea, RulesPageIndicator, RulesPageNextButton,
    RulesPagePrevButton, RulesViewState, RulesWindowRows,
};
use crate::route::{PageRoot, Route};
use infiltrator_contract::provider_cache::ProviderFingerprintChange;

/// Root marker on the Rules page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_rules_page)]
pub struct RulesPageRoot;

/// Marker for the "Refresh Rule Providers" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshRuleProvidersButton;

/// Marker for the "Clear Rule Hit Counters" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearRuleHitCountersButton;

/// A single rule entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleItem {
    pub id: usize,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub hit_count: u64,
    /// DUAL-11-09: shared persisted enabled flag (`#`-prefixed when disabled).
    pub is_enabled: bool,
    /// Last observed hit time (epoch seconds), if any.
    pub last_hit_secs: Option<u64>,
    /// Whether static analysis found this rule shadowed by an earlier rule.
    pub is_shadowed: bool,
    /// Human-readable shadow reason when `is_shadowed` is set.
    pub shadow_reason: Option<String>,
}

/// A rule provider (MRS / geosite) entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleProviderItem {
    pub name: String,
    pub rule_count: usize,
    pub behavior: String,
    pub updated_at: String,
    /// DUAL-11-04: source URL declared in the active profile, when known.
    pub source_url: Option<String>,
    /// DUAL-11-05: automatic-refresh interval declared in the active profile
    /// (seconds). The kernel owns the schedule and the conditional cache, so an
    /// undeclared provider honestly reports `None`.
    pub refresh_interval_secs: Option<u64>,
    /// DUAL-11-05: the client's local cache-file fingerprint observation
    /// (size + SHA-256 + last-modified, compared with the previous observation).
    /// `None` means no local file was observed; this is never an HTTP validator.
    pub cache_fingerprint: Option<infiltrator_contract::provider_cache::ProviderCacheFingerprint>,
}

/// Snapshot of the Rules domain.
#[derive(Clone, Debug, PartialEq)]
pub struct RulesProjection {
    pub total_rules: usize,
    pub default_action: String,
    pub providers: Vec<RuleProviderItem>,
    pub rules: Vec<RuleItem>,
    /// Shared live rule tracer read model published by the surface reader.
    pub tracer: infiltrator_contract::rule_tracer::RuleTracerSnapshot,
    /// Shared rule hit-audit read model (hits, dead/shadowed rules, CIDR overlaps).
    pub hit_audit: infiltrator_contract::rule_tracer::RuleHitAuditSnapshot,
    /// DUAL-11-03: shared MRS binary acceleration read model.
    pub mrs_acceleration: infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot,
    /// DUAL-11-08: rules the publisher dropped from `rules`, when the published
    /// list is a truncated view of the profile list. `None` means complete.
    pub truncated_rule_count: Option<usize>,
    /// DUAL-11-08: publish cap the reader applied to `rules` (0 = uncapped).
    pub rule_publish_limit: usize,
    /// DUAL-11-07: the observed kernel rule-provider cache location.
    pub provider_cache: infiltrator_contract::provider_cache::RuleProviderCacheSnapshot,
    /// DUAL-11-05: the kernel's real `etag-support` capability declared by the
    /// active profile (a top-level key; mihomo defaults it to `true`). The
    /// provider card renders the declaration, never a `304` outcome.
    pub etag_support: infiltrator_contract::provider_cache::KernelEtagSupportSnapshot,
    /// DUAL-11-14: the rules-workspace JSON documents published by the shared
    /// reader (the same text the Iced JSON editors load).
    pub json_documents: Vec<infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot>,
}

impl RulesProjection {
    /// Believable demo fixture for the Rules page.
    pub fn demo() -> Self {
        Self {
            total_rules: 2842,
            default_action: "DIRECT (漏网之鱼直连)".to_owned(),
            hit_audit: infiltrator_contract::rule_tracer::RuleTracerSnapshot::demo_fixture()
                .hit_audit,
            tracer: infiltrator_contract::rule_tracer::RuleTracerSnapshot::demo_fixture(),
            mrs_acceleration:
                infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot::demo_fixture(),
            truncated_rule_count: None,
            rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
            provider_cache: infiltrator_contract::provider_cache::RuleProviderCacheSnapshot::ready(
                "~/.config/mihomo-rs/configs/rules",
                3,
                1_048_576,
            ),
            etag_support:
                infiltrator_contract::provider_cache::KernelEtagSupportSnapshot::from_declared(Some(
                    true,
                )),
            json_documents: vec![
                infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot {
                    section: infiltrator_contract::rules_workspace::RulesJsonSection::RuleProviders,
                    json: "{\n  \"geosite-geolocation-!cn\": {\n    \"type\": \"http\",\n    \"behavior\": \"domain\",\n    \"url\": \"https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs\",\n    \"interval\": 86400\n  }\n}".to_owned(),
                },
                infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot {
                    section: infiltrator_contract::rules_workspace::RulesJsonSection::ProxyProviders,
                    json: "{}".to_owned(),
                },
                infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot {
                    section: infiltrator_contract::rules_workspace::RulesJsonSection::Sniffer,
                    json: "{\n  \"enable\": true,\n  \"sniff\": {\n    \"HTTP\": {\n      \"ports\": [80, \"8080-8880\"]\n    }\n  }\n}".to_owned(),
                },
            ],
            providers: vec![
                RuleProviderItem {
                    name: "geosite-geolocation-!cn".to_owned(),
                    rule_count: 1420,
                    behavior: "domain".to_owned(),
                    updated_at: "2026-09-02 06:00".to_owned(),
                    source_url: Some(
                        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs"
                            .to_owned(),
                    ),
                    refresh_interval_secs: Some(86_400),
                    cache_fingerprint: None,
                },
                RuleProviderItem {
                    name: "geoip-cn".to_owned(),
                    rule_count: 850,
                    behavior: "ipcidr".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                    source_url: Some(
                        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/cn.mrs"
                            .to_owned(),
                    ),
                    refresh_interval_secs: Some(86_400),
                    cache_fingerprint: None,
                },
                RuleProviderItem {
                    name: "custom-reject-ads".to_owned(),
                    rule_count: 572,
                    behavior: "classical".to_owned(),
                    updated_at: "2026-08-30 18:30".to_owned(),
                    source_url: None,
                    refresh_interval_secs: None,
                    cache_fingerprint: None,
                },
            ],
            rules: vec![
                RuleItem {
                    id: 1,
                    rule_type: "DOMAIN-SUFFIX".to_owned(),
                    payload: "google.com".to_owned(),
                    proxy: "国外媒体 (GLOBAL-MEDIA)".to_owned(),
                    hit_count: 1420,
                    is_enabled: true,
                    last_hit_secs: Some(1_700_000_010),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    id: 2,
                    rule_type: "DOMAIN-KEYWORD".to_owned(),
                    payload: "github".to_owned(),
                    proxy: "节点选择 (PROXIES)".to_owned(),
                    hit_count: 852,
                    is_enabled: false,
                    last_hit_secs: Some(1_700_000_008),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    id: 3,
                    rule_type: "GEOIP".to_owned(),
                    payload: "CN".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: 4210,
                    is_enabled: true,
                    last_hit_secs: Some(1_700_000_004),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    id: 4,
                    rule_type: "RULE-SET".to_owned(),
                    payload: "custom-reject-ads".to_owned(),
                    proxy: "REJECT".to_owned(),
                    hit_count: 128,
                    is_enabled: true,
                    last_hit_secs: Some(1_699_999_900),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    id: 5,
                    rule_type: "MATCH".to_owned(),
                    payload: "".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: 56,
                    is_enabled: true,
                    last_hit_secs: None,
                    is_shadowed: true,
                    shadow_reason: Some(
                        "Rule is unreachable because an earlier MATCH rule matches all traffic"
                            .to_owned(),
                    ),
                },
            ],
        }
    }
}

/// The typed event dispatched when rules data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct RulesProjectionUpdated(pub RulesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastRulesProjection(pub Option<RulesProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn rules_page(projection: &RulesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "分流规则 · 共 {} 条规则 ({} 个规则集 / 命中统计开启)",
        projection.total_rules,
        projection.providers.len()
    );
    let default_action = format!("最终匹配目标: {}", projection.default_action);
    let hit_audit_line = hit_audit_label(&projection.hit_audit);
    let truncation_line = truncation_label(
        projection.truncated_rule_count,
        projection.rule_publish_limit,
    );

    let provider_scenes: Vec<Box<dyn Scene>> = projection
        .providers
        .iter()
        .enumerate()
        .map(|(idx, p)| Box::new(provider_item_scene(idx, p, palette)) as Box<dyn Scene>)
        .collect();

    // DUAL-11-08: the first paint mounts the shared render window at offset 0
    // instead of one row per published rule. Every later scroll/search mounts
    // the next window through `rules_view::sync_rules_window`.
    let initial_window = infiltrator_domain::rules::view::rule_window(
        0.0,
        infiltrator_domain::rules::view::RULE_DEFAULT_VIEWPORT_PX,
        projection.rules.len(),
    );
    let rule_scenes: Vec<Box<dyn Scene>> = (initial_window.start..initial_window.end)
        .filter_map(|source_index| {
            projection
                .rules
                .get(source_index)
                .map(|rule| Box::new(rule_row_scene(source_index, rule, palette)) as Box<dyn Scene>)
        })
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
        PageRoot(Route::Rules)
        RulesPageRoot
        // DUAL-11-14: the official wheel/trackpad scroll behavior for the page
        // itself; the list partition owns a nested scroll area of its own.
        ScrollArea
        Children [
            ( { header_card_scene(summary, default_action, hit_audit_line, truncation_line, palette) } ),
            ( { crate::pages::rules_tabs::rules_tabs_scene(palette) } ),
            (
                { crate::pages::rules_tabs::tab_body_scene(
                    infiltrator_contract::rules_workspace::RulesTab::List,
                    Box::new(rules_list_partition(rule_scenes, palette)),
                ) }
            ),
            (
                { crate::pages::rules_tabs::tab_body_scene(
                    infiltrator_contract::rules_workspace::RulesTab::Providers,
                    Box::new(providers_partition(provider_scenes, palette, &projection.mrs_acceleration, &projection.provider_cache, &projection.etag_support)),
                ) }
            ),
            (
                { crate::pages::rules_tabs::tab_body_scene(
                    infiltrator_contract::rules_workspace::RulesTab::JsonEditors,
                    Box::new(crate::pages::rules_json::rules_json_scene(
                        palette,
                        &crate::pages::rules_json::RulesJsonState::default(),
                    )),
                ) }
            ),
            (
                { crate::pages::rules_tabs::tab_body_scene(
                    infiltrator_contract::rules_workspace::RulesTab::Tracer,
                    Box::new(crate::pages::rules_tracer::rules_tracer_scene(palette, &projection.tracer)),
                ) }
            ),
        ]
    }
}

/// DUAL-11-14: the list partition — the windowed rule table, the add-rule
/// wizard and the visual logical sub-rule builder, exactly the panels Iced
/// shows on its list tab.
fn rules_list_partition(
    rule_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
        }
        Children [
            ( { crate::pages::rules_builder::rules_builder_scene(palette) } ),
            ( { crate::pages::rules_subrules::rules_subrules_scene(
                palette,
                &crate::pages::rules_subrules::RulesSubRuleState::default(),
            ) } ),
            ( { rules_table_scene(rule_scenes, palette) } ),
        ]
    }
}

/// DUAL-11-14: the providers partition — the MRS acceleration card (which also
/// hosts the Geo database update entry) and the provider lifecycle table.
fn providers_partition(
    provider_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
    mrs: &infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot,
    provider_cache: &infiltrator_contract::provider_cache::RuleProviderCacheSnapshot,
    etag_support: &infiltrator_contract::provider_cache::KernelEtagSupportSnapshot,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
        }
        Children [
            ( { crate::pages::rules_mrs::rules_mrs_scene(palette, mrs, provider_cache) } ),
            ( { providers_card_scene(provider_scenes, etag_support, palette) } ),
        ]
    }
}

fn header_card_scene(
    summary: String,
    default_action: String,
    hit_audit_line: String,
    truncation_line: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("分流规则概览");

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
                        ( { icon_tile_scene(IconId::Zap, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) RulesLine(RulesLineKind::Summary) TextRole(Role::Heading) ),
                                ( Text(default_action) RulesLine(RulesLineKind::DefaultAction) TextRole(Role::Caption) ),
                                ( Text(hit_audit_line) RulesLine(RulesLineKind::HitAudit) TextRole(Role::Caption) ),
                                ( Text(truncation_line) RulesLine(RulesLineKind::Truncation) TextRole(Role::Caption) ),
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
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            RefreshRuleProvidersButton
                            Children [
                                ( Text({ "刷新规则集".to_owned() }) TextRole(Role::Body) ),
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
                            Button
                            ClearRuleHitCountersButton
                            Children [
                                ( Text({ "清空命中计数".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn providers_card_scene(
    provider_scenes: Vec<Box<dyn Scene>>,
    etag_support: &infiltrator_contract::provider_cache::KernelEtagSupportSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let etag_line = etag_support_label(etag_support);
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
                    ( Text({ "外部规则集 (Rule Providers)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "MRS / GeoSite 二进制加速".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    ( Text(etag_line) RulesLine(RulesLineKind::EtagSupport) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { provider_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn provider_item_scene(
    idx: usize,
    provider: &RuleProviderItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = provider.name.clone();
    let count_info = format!("{} 条 ({})", provider.rule_count, provider.behavior);
    let updated = provider_updated_label(provider);

    bsn! {
        Node {
            width: percent(100),
            padding: UiRect::all(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(name) ProviderNameText(idx) TextRole(Role::Body) ),
                    ( Text(count_info) ProviderCountText(idx) TextRole(Role::Caption) ),
                ]
            ),
            ( Text(updated) ProviderUpdatedText(idx) TextRole(Role::Caption) ),
        ]
    }
}

fn rules_table_scene(rule_scenes: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
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
                    ( Text({ "规则匹配序列表 (Rules Flow)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "自上而下第一命中即生效".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                        }
                        RuleSearchField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                String::new(),
                                "按匹配表达式/类型/目标即时搜索规则".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        RulesPagePrevButton
                        Children [
                            ( Text({ "上一页".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text({ "第 1/1 页 · 共 0 条".to_owned() }) RulesPageIndicator TextRole(Role::Caption) ),
                    (
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        RulesPageNextButton
                        Children [
                            ( Text({ "下一页".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    // DUAL-11-08: the fixed-height list viewport. Only the
                    // shared render window is mounted inside it; the spacers
                    // keep the scrollable range the full list height.
                    height: px(infiltrator_domain::rules::view::RULE_DEFAULT_VIEWPORT_PX),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                RulesListScrollArea
                Children [
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                        }
                        RulesWindowRows
                        Children [
                            { rule_scenes },
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// Header summary of the shared hit-audit read model.
pub(crate) fn hit_audit_label(
    audit: &infiltrator_contract::rule_tracer::RuleHitAuditSnapshot,
) -> String {
    let latency = audit
        .avg_match_latency_us
        .map(|avg| format!("{avg:.1}µs"))
        .unwrap_or_else(|| "—".to_owned());
    format!(
        "命中 {} · 冷门/被遮蔽 {} · CIDR 重叠 {} · 匹配 {}",
        audit.total_hits,
        audit.dead_rules.len(),
        audit.cidr_overlaps.len(),
        latency
    )
}

/// Live hit label for one rule, flagging disabled, zero-hit and shadowed rules
/// from the shared audit rather than showing a bare count.
pub(crate) fn rule_hit_label(rule: &RuleItem) -> String {
    if !rule.is_enabled {
        format!("{} 次命中 · 已停用", rule.hit_count)
    } else if rule.is_shadowed {
        format!("{} 次命中 · 被遮蔽", rule.hit_count)
    } else if rule.hit_count == 0 {
        format!("{} 次命中 · 冷门", rule.hit_count)
    } else {
        format!("{} 次命中", rule.hit_count)
    }
}

/// DUAL-11-05: the kernel's real `etag-support` capability as declared by the
/// active profile. mihomo reads a top-level `etag-support` boolean (default
/// `true`) to gate its `ETag`/`If-None-Match` cache; the card renders the
/// declaration and never the per-request `304` outcome, which it cannot see.
pub(crate) fn etag_support_label(
    snapshot: &infiltrator_contract::provider_cache::KernelEtagSupportSnapshot,
) -> String {
    use infiltrator_contract::provider_cache::KernelEtagSupportState;
    let state = match snapshot.state {
        KernelEtagSupportState::Enabled => "内核已启用",
        KernelEtagSupportState::Disabled => "内核未启用",
        KernelEtagSupportState::NotDeclared => "未声明",
    };
    format!("ETag 缓存: {state}")
}

/// DUAL-11-04/11-05: one provider's lifecycle line: update time, the declared
/// source URL, the declared automatic-refresh schedule and — when this client
/// read a local cache file — the local content fingerprint. The schedule is
/// executed by the kernel, which also owns the `ETag`/`304` conditional cache;
/// the fingerprint is explicitly labelled as a local file fact so nothing here
/// claims a kernel download was skipped.
pub(crate) fn provider_updated_label(provider: &RuleProviderItem) -> String {
    let source = match provider.source_url.as_deref() {
        Some(url) if !url.is_empty() => format!("来源: {url}"),
        _ => "来源: 未声明".to_owned(),
    };
    let schedule = match provider.refresh_interval_secs {
        Some(secs) => format!(
            "自动刷新: {} (内核调度)",
            infiltrator_domain::rules::view::format_refresh_interval(secs)
        ),
        None => "自动刷新: 未声明".to_owned(),
    };
    let fingerprint = match provider.cache_fingerprint.as_ref() {
        Some(observation) => {
            let change = match observation.change {
                ProviderFingerprintChange::FirstSeen => "首次观测",
                ProviderFingerprintChange::Unchanged => "较上次观测未变化",
                ProviderFingerprintChange::Changed => "较上次观测已变化",
            };
            let body = infiltrator_domain::rules::view::format_content_fingerprint(
                &observation.current.sha256,
                observation.current.size_bytes,
                observation.current.modified_unix_secs,
            );
            format!(" · 本地缓存内容指纹（非 HTTP ETag）: {body} · {change}")
        }
        None => String::new(),
    };
    format!(
        "更新: {} · {source} · {schedule}{fingerprint}",
        provider.updated_at
    )
}

pub(crate) fn rule_row_scene(
    idx: usize,
    rule: &RuleItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let idx_str = format!("#{}", rule.id);
    let type_str = format!(
        "[{}]",
        infiltrator_domain::rules::matrix::matrix_label(&rule.rule_type)
    );
    let type_chip = rule_type_chip_scene(idx, &rule.rule_type, palette);
    let payload = rule.payload.clone();
    let proxy = rule.proxy.clone();
    let hits = rule_hit_label(rule);

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S16)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        SurfacePanel
        RuleRow(idx)
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                }
                Children [
                    ( Text(idx_str) TextRole(Role::Caption) ),
                    ( type_chip ),
                    ( Text(type_str) RuleTypeText(idx) TextRole(Role::BodyStrong) ),
                    ( Text(payload) RulePayloadText(idx) TextRole(Role::Body) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                }
                Children [
                    ( Text(proxy) RuleProxyText(idx) TextRole(Role::Body) ),
                    ( Text(hits) RuleHitText(idx) TextRole(Role::Caption) ),
                    ( { rule_row_controls_scene(idx, palette) } ),
                ]
            ),
        ]
    }
}

/// DUAL-11-01: one rule row's type chip, filled from the shared type family.
/// The label and the fill both restamp in place from the shared catalogue.
fn rule_type_chip_scene(idx: usize, rule_type: &str, palette: &UiPalette) -> impl Scene + use<> {
    let label = infiltrator_domain::rules::matrix::matrix_label(rule_type);
    let fill = crate::pages::rules_projection::rule_type_chip_fill(rule_type, palette);
    bsn! {
        Node {
            min_width: px(8.0),
            min_height: px(18.0),
            padding: UiRect::horizontal(Val::Px(space::S6)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ fill })
        RuleTypeBadge(idx)
        Children [
            ( Text(label) TextRole(Role::Caption) ),
        ]
    }
}

/// DUAL-11-09/10: one row's enable/disable switch plus reorder handles. Every
/// control forwards the same typed intent for the row index to the shared
/// application, which applies `infiltrator_domain::rules::edit`.
fn rule_row_controls_scene(idx: usize, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
        }
        Children [
            (
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Button
                RuleToggleButton(idx)
                Children [
                    ( Text({ "启停".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S6)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                RuleMoveUpButton(idx)
                Children [
                    ( Text({ "↑".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S6)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                RuleMoveDownButton(idx)
                Children [
                    ( Text({ "↓".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    }
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_rules_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    // The projection observer owns the once-per-world bind guard; every other
    // rules observer registers behind the same gate.
    let first_bind = crate::pages::rules_projection::bind_rules_projection(&mut world);
    let mut commands = world.commands();
    // DUAL-11-02: the sub-rule builder scene is rebuilt from the shared default
    // draft on every mount, so its state is re-seeded with the same default —
    // the mounted card and the draft never disagree after a route change.
    commands.insert_resource(crate::pages::rules_subrules::RulesSubRuleState::default());
    // DUAL-11-06: the unpack target follows the shared MRS projection.
    commands.insert_resource(crate::pages::rules_mrs::RulesMrsState::default());
    // DUAL-11-14: every mount starts from the shared default partition and a
    // fresh (unfocused) JSON partition whose buffers are seeded from the read
    // model by the projection observer.
    commands.insert_resource(crate::pages::rules_tabs::RulesTabState::default());
    commands.insert_resource(rules_json_seed());
    if !first_bind {
        return;
    }
    // DUAL-11-13: the shared page cursor for the keyword search + pagination.
    commands.insert_resource(RulesViewState::default());
    // DUAL-11-11: the shared wizard type selection for the add-rule form.
    commands.insert_resource(crate::pages::rules_builder::RulesBuilderState::default());
    commands.add_observer(crate::pages::rules_edit::on_rules_row_edit_activated);
    commands.add_observer(crate::pages::rules_builder::on_rules_builder_activated);
    commands.add_observer(crate::pages::rules_view::on_rules_paging_activated);
    commands.add_observer(crate::pages::rules_subrules::on_rules_subrules_activated);
    commands.add_observer(crate::pages::rules_mrs::apply_mrs_projection);
    commands.add_observer(crate::pages::rules_mrs::apply_provider_cache_projection);
    commands.add_observer(crate::pages::rules_mrs::on_rules_mrs_action_activated);
    commands.add_observer(crate::pages::rules_tracer::apply_tracer_projection);
    commands.add_observer(crate::pages::rules_tracer::on_tracer_action_activated);
    commands.add_observer(crate::pages::rules_tracer::on_tracer_override_activated);
    // DUAL-11-14: the partition bar, the JSON editor partition and its
    // projection adoption.
    commands.add_observer(crate::pages::rules_tabs::on_rules_tab_activated);
    commands.add_observer(crate::pages::rules_json::on_rules_json_action_activated);
    commands.add_observer(crate::pages::rules_json::sync_rules_json);
    commands.add_observer(on_rules_action_activated);
}

/// DUAL-11-14: an empty JSON partition; the projection observer fills the
/// buffers from the shared read model on the first update.
fn rules_json_seed() -> crate::pages::rules_json::RulesJsonState {
    let mut state = crate::pages::rules_json::RulesJsonState::default();
    state.adopt(&[]);
    state
}

pub(crate) fn on_rules_action_activated(
    activate: On<Activate>,
    buttons: Query<(), With<RefreshRuleProvidersButton>>,
    clear_buttons: Query<(), With<ClearRuleHitCountersButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.contains(activate.entity) {
        handle.submit(UiCommand::RefreshRuleProviders);
    } else if clear_buttons.contains(activate.entity) {
        handle.submit(UiCommand::ClearRuleHitCounters);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_rules_fixture() {
        let proj = RulesProjection::demo();
        assert_eq!(proj.total_rules, 2842);
        assert_eq!(proj.default_action, "DIRECT (漏网之鱼直连)");
        assert_eq!(proj.providers.len(), 3);
        assert_eq!(proj.providers[0].name, "geosite-geolocation-!cn");
        assert_eq!(proj.providers[0].rule_count, 1420);
        assert_eq!(proj.rules.len(), 5);
        assert_eq!(proj.rules[0].rule_type, "DOMAIN-SUFFIX");
        assert_eq!(proj.rules[0].payload, "google.com");
        assert_eq!(proj.rules[0].proxy, "国外媒体 (GLOBAL-MEDIA)");
    }
}
