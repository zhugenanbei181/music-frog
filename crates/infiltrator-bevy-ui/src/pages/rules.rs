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
    RulesJsonState, animate_rules_json_panel, on_rules_json_action_activated, rules_json_scene,
    sync_rules_json,
};
use crate::pages::rules_mrs::{
    RulesMrsState, apply_mrs_projection, apply_provider_cache_projection,
    on_rules_mrs_action_activated,
};
use crate::pages::rules_outcome::{
    on_rule_provider_refresh_activated, sync_rule_provider_outcomes,
};
use crate::pages::rules_projection::{
    RuleEnabledKnob, RuleHitText, RuleIndexText, RulePayloadText, RuleProxyText, RuleRowStatusChip,
    RuleRowStatusText, RuleTypeBadge, RuleTypeText, RulesLine, RulesLineKind,
    apply_rules_projection, refresh_hit_copy, rule_hits_token, rule_status_token,
    rule_target_token, rule_type_chip_token,
};
use crate::pages::rules_providers::{provider_scenes, providers_partition};
use crate::pages::rules_statistics::{RulesStatisticsPlugin, statistics_scene};
use crate::pages::rules_subrule_copy;
use crate::pages::rules_subrules::{
    RulesSubRuleState, on_rules_subrules_activated, rules_subrules_scene,
};
use crate::pages::rules_tabs::{
    RulesTabState, on_rules_tab_activated, on_rules_tab_chip_activated, rules_tabs_scene,
    tab_body_scene,
};
use crate::pages::rules_tracer::{
    RulesTraceState, apply_tracer_projection, on_tracer_action_activated,
    on_tracer_override_activated, rules_tracer_scene,
};
use crate::pages::rules_view::{
    RuleRow, RuleSearchField, RulesListScrollArea, RulesPageIndicator, RulesPageNextButton,
    RulesPagePrevButton, RulesViewState, RulesWindowRows, on_rules_paging_activated,
    paint_rules_scrollbar,
};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderColor, BorderRadius, Display, FlexDirection, GridPlacement, JustifyContent,
    Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::button::{
    ButtonDisabled, ButtonLabel, ButtonSize, ButtonVariant, button_with_label_scene,
};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{PullToRefreshState, pull_to_refresh_scene};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::list::scrollbar::{
    ListScrollContainer, list_scroll_frame_with, list_scrollbar_scene,
};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::switch::{SwitchPosition, switch_knob_scene, switch_track_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, ProviderCacheFingerprint, RuleProviderCacheSnapshot,
};
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_provider_snapshot::RuleProviderControllerReference;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesTab};
use infiltrator_domain::rules::matrix::matrix_label;
use infiltrator_domain::rules::view::{RULE_DEFAULT_VIEWPORT_PX, rule_window};
use infiltrator_presentation::rule_provider_projection::{
    default_action, published_truncation, rules_summary,
};
use infiltrator_presentation::rule_row_projection::RuleRowChips;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::Failure;

/// One-off layout value (not a ladder rung): the minimum height of the rule
/// type chip that carries the row's rule-type label.
const RULE_TYPE_CHIP_MIN_HEIGHT_PX: f32 = 18.0;

/// One-off layout value (not a ladder rung): the width of the rule row's
/// ordinal column, so the row numbers line up as a column.
const RULE_INDEX_COLUMN_WIDTH_PX: f32 = 40.0;

/// One-off layout value (not a ladder rung): the height of the rule list's
/// viewport. The official scrollbar beside the viewport walks a longer list,
/// so the viewport itself stays a band inside the card that owns it.
const RULE_LIST_VIEWPORT_HEIGHT_PX: f32 = 320.0;

/// One-off layout value (not a ladder rung): the plate size of a row's
/// reorder arrow.
const REORDER_ICON_SIZE_PX: f32 = 14.0;

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
    pub provider_type: String,
    pub rule_count: usize,
    pub behavior: String,
    pub vehicle_type: String,
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
    /// The owner-fenced controller identity and inventory revision of this
    /// provider row. `None` means the owner published no live controller
    /// observation for it, so this surface cannot refresh it.
    pub controller: Option<RuleProviderControllerReference>,
    /// The owner's typed answer to this provider's last refresh, when the
    /// refresh was refused. `None` means the owner published no failure for
    /// this provider — never "the refresh succeeded".
    pub failure: Option<Failure>,
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
    let summary = rules_summary(
        projection.total_rules,
        projection.providers.len(),
        &Lang("en-US"),
    );
    let default_action = default_action(&projection.default_action, &Lang("en-US"));
    let truncation_line = published_truncation(
        projection.truncated_rule_count,
        projection.rule_publish_limit,
        &Lang("en-US"),
    );

    let provider_scenes = provider_scenes(&projection.providers, palette);

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
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::Rules)
            RulesPageRoot
            // DUAL-11-14: the official wheel/trackpad scroll behavior for the page
            // itself; the list partition owns a nested scroll area of its own.
            ScrollArea
            Children [
                // The page's own column: it never shrinks, so the page root
                // scrolls the whole workspace instead of squeezing a panel
                // until its content overflows the card that owns it.
                Node {
                    width: percent(100),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S16),
                }
                Children [
                    @{ pull_to_refresh_scene(&PullToRefreshState::default(), palette) }
                    --
                    @{ header_card_scene(summary, default_action, palette) }
                    --
                    @{ rules_tabs_scene(palette) }
                    --
                    @{ tab_body_scene(
                            RulesTab::List,
                            Box::new(rules_list_partition(rule_scenes, truncation_line, palette)),
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
            ]
    }
}

/// DUAL-11-14: the list partition — the windowed rule table, the add-rule
/// wizard and the visual logical sub-rule builder, exactly the panels Iced
/// shows on its list tab.
fn rules_list_partition(
    rule_scenes: Vec<Box<dyn Scene>>,
    truncation_line: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
            }
            Children [
                @{ rules_draft::scene(palette) }
                --
                @{ rules_table_scene(rule_scenes, truncation_line, palette) }
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

fn header_card_scene(
    summary: String,
    default_action: String,
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
                        column_gap: rem(space::S16),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("rules_title")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Zap, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S4),
                            }
                            Children [
                                Text(summary) RulesLine(RulesLineKind::Summary) TextRole(Role::Heading)
                                --
                                Text(default_action) RulesLine(RulesLineKind::DefaultAction) TextRole(Role::Caption)
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                            flex_shrink: 0.0,
                        }
                        Children [
                            @{
                                (
                                    button_with_label_scene(
                                        bsn! {
                                            LocalizedText::plain("rules_refresh_providers_action")
                                            TextRole(Role::Caption)
                                            ButtonLabel
                                        },
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ),
                                    bsn! {
                                        RefreshRuleProvidersButton
                                        LocalizedLabel::plain("rules_refresh_providers_action")
                                    },
                                )
                            }
                            --
                            @{
                                (
                                    button_with_label_scene(
                                        bsn! {
                                            LocalizedText::plain("rule_hit_btn_clear")
                                            TextRole(Role::Caption)
                                            ButtonLabel
                                        },
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ),
                                    bsn! {
                                        ClearRuleHitCountersButton
                                        LocalizedLabel::plain("rule_hit_btn_clear")
                                    },
                                )
                            }
                        ]
                    ]
        })],
        palette,
    )
}

fn rules_table_scene(
    rule_scenes: Vec<Box<dyn Scene>>,
    truncation_line: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    // The rule list's viewport is the official `ScrollArea`, and the official
    // scrollbar beside it drives that viewport's own scroll position. The bar
    // must sit *beside* the viewport, not inside it, so the two are siblings
    // in the shared list's grid frame.
    let list_container = Box::new(bsn! {
            Node {
                width: percent(100),
                // DUAL-11-08: the list viewport. Only the shared render window
                // is mounted inside it; the spacers keep the scrollable range
                // the full list height. The viewport is bounded so the list
                // never grows past the card that owns it — the official
                // scrollbar beside it is what walks a long list.
                height: rem(RULE_LIST_VIEWPORT_HEIGHT_PX),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                grid_row: GridPlacement::start(1),
                grid_column: GridPlacement::start(1),
            }
            ScrollArea
            RulesListScrollArea
            ListScrollContainer
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
    }) as Box<dyn Scene>;
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("rules_flow_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("rules_flow_hint") TextRole(Role::Caption)
                            ]
            }),
            // The publish-window note belongs to the list it describes, not to
            // the page header: the header carries the title and the actions.
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                Text({ truncation_line }) RulesLine(RulesLineKind::Truncation) TextRole(Role::Caption)
                            ]
            }),
            // The keyword field owns its own row: the list's paging controls
            // never squeeze the search field into a stub.
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    min_width: px(0.0),
                                }
                                RuleSearchField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_rules_search"),
                                            palette,
                                    ) }
                                ]
                            ]
            }),
            // The list's own paging row. The page buttons move the viewport's
            // own scroll position; they never own a second offset.
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexEnd,
                                column_gap: rem(space::S8),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                Node {
                                    min_height: rem(palette.control_height_px),
                                    padding: UiRect::horizontal(rem(space::S8)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                }
                                ThemeBackgroundColor({ Token::SurfaceElevated })
                                Button
                                RulesPagePrevButton
                                Children [
                                    LocalizedText::plain("common_previous_page") TextRole(Role::Caption)
                                ]
                                --
                                LocalizedText::plain("rules_empty_pagination") RulesPageIndicator TextRole(Role::Caption)
                                --
                                Node {
                                    min_height: rem(palette.control_height_px),
                                    padding: UiRect::horizontal(rem(space::S8)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                }
                                ThemeBackgroundColor({ Token::SurfaceElevated })
                                Button
                                RulesPageNextButton
                                Children [
                                    LocalizedText::plain("common_next_page") TextRole(Role::Caption)
                                ]
                            ]
            }),
            // The rules table owns its own viewport, so it names its own
            // scrollbar: the shared frame only owns the geometry beside the
            // viewport, and the official bar drives that viewport's own
            // scroll position.
            list_scroll_frame_with(list_container, list_scrollbar_scene()),
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
    let type_chip = rule_type_chip_scene(idx, &rule.rule_type);
    let payload = rule.payload.clone();
    let proxy = rule.proxy.clone();
    // The row carries two facts of its own: the local trace's count and the
    // row's own status. They are two typed facts, so the row draws them as
    // two chips instead of reading both out of one mixed sentence.
    let chips = RuleRowChips::of(rule.hit_count, rule.is_enabled, rule.is_shadowed);
    let count = rule
        .hit_count
        .map(|value| value.to_string())
        .unwrap_or_default();
    let hits = LocalizedText::new(chips.hits.key(), vec![("count", count.clone())]);
    // A plain row carries no status fact of its own, so it has no status
    // chip: the chip's box starts hidden and the projection shows it when
    // the row really has a status. Its copy is a complete template either
    // way, so the hidden chip never shows a half-rendered sentence.
    let status_key = chips.status.key();
    let status = match status_key {
        Some(key) => LocalizedText::plain(key),
        None => LocalizedText::new(chips.hits.key(), vec![("count", count)]),
    };
    let status_display = if status_key.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    // The target action and the hit statistics carry their own semantic ink
    // from the first frame: the row is mounted from the projection, so the
    // scene is where the row's own facts become their colors.
    let target_ink = rule_target_token(&rule.proxy);
    let hits_ink = rule_hits_token();
    let status_ink = rule_status_token(chips.status);

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::vertical(rem(space::S8)),
                // A rule row is a table row, not a card: no radius ladder and
                // no card frame, only the hairline that separates two rows.
                border: UiRect::bottom(Val::Px(palette.hairline_px)),
            }
            BorderColor {
                top: Color::NONE,
                right: Color::NONE,
                bottom: { palette.border },
                left: Color::NONE,
            }
            RuleRow(idx)
            Children [
                Node {
                    min_width: px(0.0),
                    flex_basis: px(0.0),
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S12),
                }
                Children [
                    Node {
                        flex_shrink: 0.0,
                        min_width: rem(RULE_INDEX_COLUMN_WIDTH_PX),
                        justify_content: JustifyContent::FlexEnd,
                    }
                    Children [
                        Text(idx_str) RuleIndexText(idx) TextRole(Role::Mono)
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
                    column_gap: rem(space::S12),
                }
                Children [
                    Text(proxy) RuleProxyText(idx) TextRole(Role::BodyStrong) ThemeTextColor({ target_ink })
                    --
                    Node {
                        flex_shrink: 0.0,
                        padding: UiRect::horizontal(rem(space::S8)),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(rem(radius::XS)),
                    }
                    ThemeBackgroundColor({ Token::SurfaceElevated })
                    Children [
                        LocalizedText { .. { hits } } RuleHitText(idx) TextRole(Role::Caption) ThemeTextColor({ hits_ink })
                    ]
                    --
                    Node {
                        flex_shrink: 0.0,
                        display: { status_display },
                        padding: UiRect::horizontal(rem(space::S8)),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(rem(radius::XS)),
                    }
                    ThemeBackgroundColor({ Token::SurfaceElevated })
                    RuleRowStatusChip(idx)
                    Children [
                        LocalizedText { .. { status } } RuleRowStatusText(idx) TextRole(Role::Caption) ThemeTextColor({ status_ink })
                    ]
                    --
                    @{ rule_row_controls_scene(rule.edit_id, rule.is_enabled, idx, palette) }
                ]
            ]
    }
}

/// DUAL-11-01: one rule row's type chip. The chip's *label* carries the
/// semantic hue of the shared type family, so the badge reads as a tag instead
/// of a flat blue pill, and the label keeps the readable ink of the family.
fn rule_type_chip_scene(idx: usize, rule_type: &str) -> impl Scene + use<> {
    let label = matrix_label(rule_type);
    let token = rule_type_chip_token(rule_type);
    bsn! {
            Node {
                flex_shrink: 0.0,
                min_width: rem(space::S8),
                min_height: rem(RULE_TYPE_CHIP_MIN_HEIGHT_PX),
                padding: UiRect::horizontal(rem(space::S6)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::XS)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            RuleTypeBadge(idx)
            Children [
                Text(label) RuleTypeText(idx) TextRole(Role::Caption) ThemeTextColor({ token })
            ]
    }
}

/// DUAL-11-09/10: one row's enable/disable switch plus reorder handles. Every
/// control forwards the same typed intent for the row index to the shared
/// application, which applies `infiltrator_domain::rules::edit`.
///
/// The enable/disable control is the shared switch: the knob's rest position
/// is the row's enabled state, so the control and the row never disagree.
fn rule_row_controls_scene(
    id: Option<RuleRowId>,
    enabled: bool,
    idx: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let toggle_node = button_semantic_node("");
    let up_node = button_semantic_node("");
    let down_node = button_semantic_node("");
    let track_fill = palette.surface_elevated;
    let track_edge = palette.border;
    let knob_fill = palette.ink;
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: rem(space::S4),
                flex_shrink: 0.0,
            }
            Children [
                @{ switch_track_scene(palette, track_fill, track_edge) }
                RuleToggleButton(id) RuleDraftMutationButton ButtonDisabled(true)
                toggle_node LocalizedLabel::plain("rules_toggle_row")
                Button
                Children [
                    @{ switch_knob_scene(knob_fill, SwitchPosition::from_enabled(enabled)) }
                    RuleEnabledKnob(idx)
                ]
                --
                Node {
                    min_height: rem(space::S24),
                    min_width: rem(space::S24),
                    padding: UiRect::all(rem(space::S4)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(rem(radius::XS)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Button
                RuleMoveUpButton(id) RuleDraftMutationButton ButtonDisabled(true)
                up_node LocalizedLabel::plain("rules_move_up")
                Children [
                    @{ icon_scene(IconId::ArrowUp, REORDER_ICON_SIZE_PX, palette.ink_dim) }
                ]
                --
                Node {
                    min_height: rem(space::S24),
                    min_width: rem(space::S24),
                    padding: UiRect::all(rem(space::S4)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(rem(radius::XS)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Button
                RuleMoveDownButton(id) RuleDraftMutationButton ButtonDisabled(true)
                down_node LocalizedLabel::plain("rules_move_down")
                Children [
                    @{ icon_scene(IconId::ArrowDown, REORDER_ICON_SIZE_PX, palette.ink_dim) }
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
                animate_rules_json_panel,
                paint_rules_scrollbar,
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
        app.add_observer(on_rules_tab_chip_activated);
        app.add_observer(on_rules_json_action_activated);
        app.add_observer(sync_rules_json);
        app.add_observer(on_rules_action_activated);
        // DUAL-11-05: the per-provider refresh control and its failure line.
        app.add_observer(on_rule_provider_refresh_activated);
        app.add_systems(PostUpdate, sync_rule_provider_outcomes);
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
