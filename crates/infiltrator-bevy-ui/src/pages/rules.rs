//! The Rules page (分流规则): ruleset table, MRS/geosite providers,
//! rule tracer, and hit statistics.
//!
//! **Update seam**: mutable nodes carry typed markers ([`RulesLine`],
//! [`RuleHitText`], [`RuleProxyText`], [`RulePayloadText`], [`RuleTypeText`],
//! [`ProviderNameText`], [`ProviderCountText`], [`ProviderUpdatedText`]).
//! [`RulesPagePlugin`] registers [`apply_rules_projection`] and action observers
//! once at product assembly. When [`RulesProjectionUpdated`]
//! fires, texts and hit counts restamp in place without tree rebuilds.

use crate::a11y::button_semantic_node;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_field_scene;
use crate::pages::rules_builder::{
    RulesBuilderState, on_rules_builder_activated, rules_builder_scene,
};
use crate::pages::rules_draft;
use crate::pages::rules_draft::{RuleDraftMutationButton, RulesDraftState};
use crate::pages::rules_edit::{
    RuleMoveDownButton, RuleMoveUpButton, RuleToggleButton, on_rules_row_edit_activated,
};
use crate::pages::rules_fact_copy;
use crate::pages::rules_json::{
    RulesJsonState, on_rules_json_action_activated, rules_json_scene, sync_rules_json,
};
use crate::pages::rules_mrs::{
    RulesMrsState, apply_mrs_projection, apply_provider_cache_projection,
    on_rules_mrs_action_activated, rules_mrs_scene,
};
use crate::pages::rules_projection::{
    ProviderCountText, ProviderNameText, ProviderUpdatedText, RuleHitText, RuleIndexText,
    RulePayloadText, RuleProxyText, RuleTypeBadge, RuleTypeText, RulesLine, RulesLineKind,
    apply_rules_projection, refresh_hit_copy, rule_type_chip_fill,
};
use crate::pages::rules_statistics::{RulesStatisticsPlugin, statistics_scene};
use crate::pages::rules_subrule_copy;
use crate::pages::rules_subrules::{
    RulesSubRuleState, on_rules_subrules_activated, rules_subrules_scene,
};
use crate::pages::rules_tabs::{
    RulesTabState, on_rules_tab_activated, rules_tabs_scene, tab_body_scene,
};
use crate::pages::rules_tracer::{
    RulesTraceState, apply_tracer_projection, on_tracer_action_activated,
    on_tracer_override_activated, rules_tracer_scene,
};
use crate::pages::rules_view::{
    RuleRow, RuleSearchField, RulesListScrollArea, RulesPageIndicator, RulesPageNextButton,
    RulesPagePrevButton, RulesViewState, RulesWindowRows, on_rules_paging_activated,
};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::rule_provider_projection::{
    default_action, etag_support_line, provider_count, provider_fingerprint_line,
    provider_lifecycle_line, published_truncation, rules_summary,
};
use infiltrator_application::rule_row_projection::row_hits_key;
use infiltrator_application::rule_statistics_projection::project_statistics;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::gesture::{PullToRefreshState, pull_to_refresh_scene};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::{SurfacePanel, surface_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::error::Failure;
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, ProviderCacheFingerprint, RuleProviderCacheSnapshot,
};
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesTab};
use infiltrator_domain::rules::matrix::matrix_label;
use infiltrator_domain::rules::view::{RULE_DEFAULT_VIEWPORT_PX, rule_window};
use infiltrator_shared::locales::Lang;

/// Root marker on the Rules page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct RulesPageRoot;

/// Marker for the "Refresh Rule Providers" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshRuleProvidersButton;

/// Marker for the "Clear Rule Hit Counters" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearRuleHitCountersButton;

/// A single rule entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleItem {
    pub edit_id: Option<RuleRowId>,
    pub raw: String,
    pub source_ip: bool,
    pub no_resolve: bool,
    pub failure: Option<Failure>,
    pub id: usize,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub hit_count: Option<u64>,
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
    pub cache_fingerprint: Option<ProviderCacheFingerprint>,
}

/// Snapshot of the Rules domain.
#[derive(Clone, Debug, PartialEq)]
pub struct RulesProjection {
    pub total_rules: usize,
    pub default_action: String,
    pub providers: Vec<RuleProviderItem>,
    pub rules: Vec<RuleItem>,
    /// Shared live rule tracer read model published by the surface reader.
    pub tracer: RuleTracerSnapshot,
    /// Shared rule hit-audit read model (hits, dead/shadowed rules, CIDR overlaps).
    pub hit_audit: Option<RuleHitAuditSnapshot>,
    /// DUAL-11-03: shared MRS binary acceleration read model.
    pub mrs_acceleration: MrsAccelerationSnapshot,
    /// DUAL-11-08: rules the publisher dropped from `rules`, when the published
    /// list is a truncated view of the profile list. `None` means complete.
    pub truncated_rule_count: Option<usize>,
    /// DUAL-11-08: publish cap the reader applied to `rules` (0 = uncapped).
    pub rule_publish_limit: usize,
    /// DUAL-11-07: the observed kernel rule-provider cache location.
    pub provider_cache: RuleProviderCacheSnapshot,
    /// DUAL-11-05: the kernel's real `etag-support` capability declared by the
    /// active profile (a top-level key; mihomo defaults it to `true`). The
    /// provider card renders the declaration, never a `304` outcome.
    pub etag_support: KernelEtagSupportSnapshot,
    /// DUAL-11-14: the rules-workspace JSON documents published by the shared
    /// reader (the same text the Iced JSON editors load).
    pub json_documents: Vec<RulesJsonDocumentSnapshot>,
}

/// The typed event dispatched when rules data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct RulesProjectionUpdated(pub RulesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastRulesProjection(pub Option<RulesProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn rules_page(projection: &RulesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = rules_summary(projection.total_rules, projection.providers.len(), "en-US");
    let default_action = default_action(&projection.default_action, "en-US");
    let audit_copy = project_statistics(projection.hit_audit.as_ref(), "en-US");
    let hit_audit_line = LocalizedText::new(audit_copy.key, audit_copy.params);
    let truncation_line = published_truncation(
        projection.truncated_rule_count,
        projection.rule_publish_limit,
        "en-US",
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
    let initial_window = rule_window(0.0, RULE_DEFAULT_VIEWPORT_PX, projection.rules.len());
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
                @{ pull_to_refresh_scene(&PullToRefreshState::default(), palette) }
                --
                @{ header_card_scene(summary, default_action, hit_audit_line, truncation_line, palette) }
                --
                @{ rules_tabs_scene(palette) }
                --
                @{ tab_body_scene(
                        RulesTab::List,
                        Box::new(rules_list_partition(rule_scenes, palette)),
                ) }
                --
                @{ tab_body_scene(
                        RulesTab::Providers,
                        Box::new(providers_partition(provider_scenes, palette, &projection.mrs_acceleration, &projection.provider_cache, &projection.etag_support)),
                ) }
                --
                @{ tab_body_scene(
                        RulesTab::JsonEditors,
                        Box::new(rules_json_scene(
                                palette,
                                &RulesJsonState::default(),
                        )),
                ) }
                --
                @{ tab_body_scene(
                        RulesTab::Tracer,
                        Box::new(rules_tracer_scene(palette, &projection.tracer)),
                ) }
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
                @{ rules_draft::scene(palette) }
                --
                @{ rules_table_scene(rule_scenes, palette) }
                --
                @{ statistics_scene(palette) }
                --
                @{ rules_builder_scene(palette) }
                --
                @{ rules_subrules_scene(
                        palette,
                        &RulesSubRuleState::default(),
                ) }
            ]
    }
}

/// DUAL-11-14: the providers partition — the MRS acceleration card (which also
/// hosts the Geo database update entry) and the provider lifecycle table.
fn providers_partition(
    provider_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
    mrs: &MrsAccelerationSnapshot,
    provider_cache: &RuleProviderCacheSnapshot,
    etag_support: &KernelEtagSupportSnapshot,
) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
            }
            Children [
                @{ rules_mrs_scene(palette, mrs, provider_cache) }
                --
                @{ providers_card_scene(provider_scenes, etag_support, palette) }
            ]
    }
}

fn header_card_scene(
    summary: String,
    default_action: String,
    hit_audit_line: LocalizedText,
    truncation_line: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("Rules");

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("rules_title")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Zap, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                Text(summary) RulesLine(RulesLineKind::Summary) TextRole(Role::Heading)
                                --
                                Text(default_action) RulesLine(RulesLineKind::DefaultAction) TextRole(Role::Caption)
                                --
                                LocalizedText { .. { hit_audit_line } } RulesLine(RulesLineKind::HitAudit) TextRole(Role::Caption)
                                --
                                Text(truncation_line) RulesLine(RulesLineKind::Truncation) TextRole(Role::Caption)
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
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            RefreshRuleProvidersButton
                            Children [
                                LocalizedText::plain("rules_refresh_providers_action") TextRole(Role::Body)
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
                            Button
                            ClearRuleHitCountersButton ButtonDisabled(true)
                            Children [
                                LocalizedText::plain("rule_hit_btn_clear") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

fn providers_card_scene(
    provider_scenes: Vec<Box<dyn Scene>>,
    etag_support: &KernelEtagSupportSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let etag_line = etag_support_line(etag_support, &Lang("en-US"));
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
                                LocalizedText::plain("rules_providers_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("rules_providers_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                Text(etag_line) RulesLine(RulesLineKind::EtagSupport) TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                { provider_scenes }
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
    let count_info = provider_count(provider.rule_count, &provider.behavior, "en-US");
    let updated = provider_lifecycle_line(
        &provider.updated_at,
        provider.source_url.as_deref(),
        provider.refresh_interval_secs,
        &Lang("en-US"),
    );
    let updated = provider
        .cache_fingerprint
        .as_ref()
        .map(|observation| {
            format!(
                "{updated} · {}",
                provider_fingerprint_line(observation, &Lang("en-US"))
            )
        })
        .unwrap_or(updated);

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
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    Text(name) ProviderNameText(idx) TextRole(Role::Body)
                    --
                    Text(count_info) ProviderCountText(idx) TextRole(Role::Caption)
                ]
                --
                Text(updated) ProviderUpdatedText(idx) TextRole(Role::Caption)
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
                                LocalizedText::plain("rules_flow_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("rules_flow_hint") TextRole(Role::Caption)
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
                                Node {
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                RuleSearchField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_rules_search"),
                                            palette,
                                    ) }
                                ]
                                --
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
                                    LocalizedText::plain("common_previous_page") TextRole(Role::Caption)
                                ]
                                --
                                LocalizedText::plain("rules_empty_pagination") RulesPageIndicator TextRole(Role::Caption)
                                --
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
                                    LocalizedText::plain("common_next_page") TextRole(Role::Caption)
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                // DUAL-11-08: the fixed-height list viewport. Only the
                                // shared render window is mounted inside it; the spacers
                                // keep the scrollable range the full list height.
                                height: px(RULE_DEFAULT_VIEWPORT_PX),
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::scroll_y(),
                            }
                            ScrollArea
                            RulesListScrollArea
                            Children [
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Column,
                                }
                                RulesWindowRows
                                Children [
                                    { rule_scenes }
                                ]
                            ]
            }),
        ],
        palette,
    )
}

pub(crate) fn rule_row_scene(
    idx: usize,
    rule: &RuleItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let idx_str = format!("#{}", rule.id);
    let type_chip = rule_type_chip_scene(idx, &rule.rule_type, palette);
    let payload = rule.payload.clone();
    let proxy = rule.proxy.clone();
    let hits = LocalizedText::new(
        row_hits_key(rule.hit_count, rule.is_enabled, rule.is_shadowed),
        vec![(
            "count",
            rule.hit_count
                .map(|value| value.to_string())
                .unwrap_or_default(),
        )],
    );

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
                Node {
                    min_width: px(0.0),
                    flex_basis: px(0.0),
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                }
                Children [
                    Node { flex_shrink: 0.0 }
                    Children [
                        Text(idx_str) RuleIndexText(idx) TextRole(Role::Caption)
                    ]
                    --
                    @type_chip
                    --
                    Node {
                        min_width: px(0.0),
                        flex_basis: px(0.0),
                        flex_grow: 1.0,
                    }
                    Children [
                        Text(payload) RulePayloadText(idx) TextRole(Role::Body)
                    ]
                ]
                --
                Node {
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S12),
                }
                Children [
                    Text(proxy) RuleProxyText(idx) TextRole(Role::Body)
                    --
                    LocalizedText { .. { hits } } RuleHitText(idx) TextRole(Role::Caption)
                    --
                    @{ rule_row_controls_scene(rule.edit_id, palette) }
                ]
            ]
    }
}

/// DUAL-11-01: one rule row's type chip, filled from the shared type family.
/// The label and the fill both restamp in place from the shared catalogue.
fn rule_type_chip_scene(idx: usize, rule_type: &str, palette: &UiPalette) -> impl Scene + use<> {
    let label = matrix_label(rule_type);
    let fill = rule_type_chip_fill(rule_type, palette);
    bsn! {
            Node {
                flex_shrink: 0.0,
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
                Text(label) RuleTypeText(idx) TextRole(Role::Caption)
            ]
    }
}

/// DUAL-11-09/10: one row's enable/disable switch plus reorder handles. Every
/// control forwards the same typed intent for the row index to the shared
/// application, which applies `infiltrator_domain::rules::edit`.
fn rule_row_controls_scene(id: Option<RuleRowId>, palette: &UiPalette) -> impl Scene + use<> {
    let toggle_node = button_semantic_node("");
    let up_node = button_semantic_node("");
    let down_node = button_semantic_node("");
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S4),
            }
            Children [
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Button
                RuleToggleButton(id) RuleDraftMutationButton ButtonDisabled(true)
                toggle_node LocalizedLabel::plain("rules_toggle_row")
                Children [
                    LocalizedText::plain("rules_toggle_row") TextRole(Role::Caption)
                ]
                --
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S6)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                RuleMoveUpButton(id) RuleDraftMutationButton ButtonDisabled(true)
                up_node LocalizedLabel::plain("rules_move_up")
                Children [
                    @{ icon_tile_scene(IconId::ArrowUp, 14.0, palette) }
                ]
                --
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S6)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                RuleMoveDownButton(id) RuleDraftMutationButton ButtonDisabled(true)
                down_node LocalizedLabel::plain("rules_move_down")
                Children [
                    @{ icon_tile_scene(IconId::ArrowDown, 14.0, palette) }
                ]
            ]
    }
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct RulesPagePlugin;

impl Plugin for RulesPagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RulesStatisticsPlugin);
        app.init_resource::<LastRulesProjection>();
        app.add_systems(
            Update,
            (
                refresh_hit_copy,
                rules_fact_copy::sync,
                rules_subrule_copy::sync,
            ),
        );
        app.init_resource::<RulesTraceState>();
        app.init_resource::<RulesBuilderState>();
        app.init_resource::<RulesDraftState>();
        app.add_observer(apply_rules_projection);
        // Resources outlive route entities; remounts preserve unsaved inputs.
        app.init_resource::<RulesSubRuleState>();
        // DUAL-11-06: the unpack target follows the shared MRS projection.
        app.init_resource::<RulesMrsState>();
        // The first real projection seeds JSON; subsequent mounts keep the draft.
        app.init_resource::<RulesTabState>();
        app.init_resource::<RulesJsonState>();
        // DUAL-11-13: the shared page cursor for the keyword search + pagination.
        app.init_resource::<RulesViewState>();
        // DUAL-11-11: the shared wizard type selection for the add-rule form.
        app.add_observer(on_rules_row_edit_activated);
        app.add_observer(on_rules_builder_activated);
        app.add_observer(on_rules_paging_activated);
        app.add_observer(on_rules_subrules_activated);
        app.add_observer(apply_mrs_projection);
        app.add_observer(apply_provider_cache_projection);
        app.add_observer(on_rules_mrs_action_activated);
        app.add_observer(apply_tracer_projection);
        app.add_observer(on_tracer_action_activated);
        app.add_observer(on_tracer_override_activated);
        // DUAL-11-14: the partition bar, the JSON editor partition and its
        // projection adoption.
        app.add_observer(on_rules_tab_activated);
        app.add_observer(on_rules_json_action_activated);
        app.add_observer(sync_rules_json);
        app.add_observer(on_rules_action_activated);
    }
}

pub(crate) fn on_rules_action_activated(
    activate: On<Activate>,
    buttons: Query<(), With<RefreshRuleProvidersButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.contains(activate.entity) {
        handle.submit(UiCommand::RefreshRuleProviders);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_rules_fixture() {
        let proj = RulesProjection::demo();
        assert_eq!(proj.total_rules, 2842);
        assert_eq!(proj.default_action, "DIRECT");
        assert_eq!(proj.providers.len(), 3);
        assert_eq!(proj.providers[0].name, "geosite-geolocation-!cn");
        assert_eq!(proj.providers[0].rule_count, 1420);
        assert_eq!(proj.rules.len(), 5);
        assert_eq!(proj.rules[0].rule_type, "DOMAIN-SUFFIX");
        assert_eq!(proj.rules[0].payload, "google.com");
        assert_eq!(proj.rules[0].proxy, "STREAMING");
    }
}
