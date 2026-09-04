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
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
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

use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{PageRoot, Route};

pub use crate::pages::rules_mrs::{RulesMrsRoot, UnpackRuleProviderButton, rules_mrs_scene};

/// Root marker on the Rules page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_rules_page)]
pub struct RulesPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct RulesPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesLine(pub RulesLineKind);

/// Different text lines on the rules page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RulesLineKind {
    /// Overview summary: total rules and active providers count.
    #[default]
    Summary,
    /// Default fallback rule target.
    DefaultAction,
}

/// Marker for a rule item hit count text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleHitText(pub usize);

/// Marker for a rule item proxy outbound text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleProxyText(pub usize);

/// Marker for a rule item payload text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulePayloadText(pub usize);

/// Marker for a rule item type text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTypeText(pub usize);

/// Marker for a rule provider name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderNameText(pub usize);

/// Marker for a rule provider count text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCountText(pub usize);

/// Marker for a rule provider update time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderUpdatedText(pub usize);

/// Marker for the "Refresh Rule Providers" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshRuleProvidersButton;

/// A single rule entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleItem {
    pub id: usize,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub hit_count: u64,
}

/// A rule provider (MRS / geosite) entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleProviderItem {
    pub name: String,
    pub rule_count: usize,
    pub behavior: String,
    pub updated_at: String,
}

/// Snapshot of the Rules domain.
#[derive(Clone, Debug, PartialEq)]
pub struct RulesProjection {
    pub total_rules: usize,
    pub default_action: String,
    pub providers: Vec<RuleProviderItem>,
    pub rules: Vec<RuleItem>,
}

impl RulesProjection {
    /// Believable demo fixture for the Rules page.
    pub fn demo() -> Self {
        Self {
            total_rules: 2842,
            default_action: "DIRECT (漏网之鱼直连)".to_owned(),
            providers: vec![
                RuleProviderItem {
                    name: "geosite-geolocation-!cn".to_owned(),
                    rule_count: 1420,
                    behavior: "domain".to_owned(),
                    updated_at: "2026-09-02 06:00".to_owned(),
                },
                RuleProviderItem {
                    name: "geoip-cn".to_owned(),
                    rule_count: 850,
                    behavior: "ipcidr".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                },
                RuleProviderItem {
                    name: "custom-reject-ads".to_owned(),
                    rule_count: 572,
                    behavior: "classical".to_owned(),
                    updated_at: "2026-08-30 18:30".to_owned(),
                },
            ],
            rules: vec![
                RuleItem {
                    id: 1,
                    rule_type: "DOMAIN-SUFFIX".to_owned(),
                    payload: "google.com".to_owned(),
                    proxy: "国外媒体 (GLOBAL-MEDIA)".to_owned(),
                    hit_count: 1420,
                },
                RuleItem {
                    id: 2,
                    rule_type: "DOMAIN-KEYWORD".to_owned(),
                    payload: "github".to_owned(),
                    proxy: "节点选择 (PROXIES)".to_owned(),
                    hit_count: 852,
                },
                RuleItem {
                    id: 3,
                    rule_type: "GEOIP".to_owned(),
                    payload: "CN".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: 4210,
                },
                RuleItem {
                    id: 4,
                    rule_type: "RULE-SET".to_owned(),
                    payload: "custom-reject-ads".to_owned(),
                    proxy: "REJECT".to_owned(),
                    hit_count: 128,
                },
                RuleItem {
                    id: 5,
                    rule_type: "MATCH".to_owned(),
                    payload: "".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: 56,
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

    let provider_scenes: Vec<Box<dyn Scene>> = projection
        .providers
        .iter()
        .enumerate()
        .map(|(idx, p)| Box::new(provider_item_scene(idx, p, palette)) as Box<dyn Scene>)
        .collect();

    let rule_scenes: Vec<Box<dyn Scene>> = projection
        .rules
        .iter()
        .enumerate()
        .map(|(idx, r)| Box::new(rule_row_scene(idx, r, palette)) as Box<dyn Scene>)
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
        Children [
            ( { header_card_scene(summary, default_action, palette) } ),
            ( { crate::pages::rules_tracer::rules_tracer_scene(palette) } ),
            ( { crate::pages::rules_mrs::rules_mrs_scene(palette) } ),
            ( { crate::pages::rules_builder::rules_builder_scene(palette) } ),
            ( { providers_card_scene(provider_scenes, palette) } ),
            ( { rules_table_scene(rule_scenes, palette) } ),
        ]
    }
}

fn header_card_scene(
    summary: String,
    default_action: String,
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
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn providers_card_scene(
    provider_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
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
    let updated = format!("更新: {}", provider.updated_at);

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
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { rule_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn rule_row_scene(idx: usize, rule: &RuleItem, palette: &UiPalette) -> impl Scene + use<> {
    let idx_str = format!("#{}", rule.id);
    let type_str = format!("[{}]", rule.rule_type);
    let payload = rule.payload.clone();
    let proxy = rule.proxy.clone();
    let hits = format!("{} 次命中", rule.hit_count);

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( Text(idx_str) TextRole(Role::Caption) ),
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
                    ]
                ),
            ]
        })],
        palette,
    )
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_rules_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<RulesPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(RulesPageBound);
    commands.add_observer(apply_rules_projection);
    commands.add_observer(on_rules_action_activated);
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

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_rules_projection(
    update: On<RulesProjectionUpdated>,
    mut last: Option<ResMut<LastRulesProjection>>,
    mut lines: Query<
        (&mut Text, &RulesLine),
        (
            With<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut hits: Query<
        (&mut Text, &RuleHitText),
        (
            With<RuleHitText>,
            Without<RulesLine>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut proxies: Query<
        (&mut Text, &RuleProxyText),
        (
            With<RuleProxyText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut payloads: Query<
        (&mut Text, &RulePayloadText),
        (
            With<RulePayloadText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut types: Query<
        (&mut Text, &RuleTypeText),
        (
            With<RuleTypeText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut provider_names: Query<
        (&mut Text, &ProviderNameText),
        (
            With<ProviderNameText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderCountText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut provider_counts: Query<
        (&mut Text, &ProviderCountText),
        (
            With<ProviderCountText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderUpdatedText>,
        ),
    >,
    mut provider_updates: Query<
        (&mut Text, &ProviderUpdatedText),
        (
            With<ProviderUpdatedText>,
            Without<RulesLine>,
            Without<RuleHitText>,
            Without<RuleProxyText>,
            Without<RulePayloadText>,
            Without<RuleTypeText>,
            Without<ProviderNameText>,
            Without<ProviderCountText>,
        ),
    >,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            RulesLineKind::Summary => {
                let want = format!(
                    "分流规则 · 共 {} 条规则 ({} 个规则集 / 命中统计开启)",
                    projection.total_rules,
                    projection.providers.len()
                );
                if text.0 != want {
                    text.0 = want;
                }
            }
            RulesLineKind::DefaultAction => {
                let want = format!("最终匹配目标: {}", projection.default_action);
                if text.0 != want {
                    text.0 = want;
                }
            }
        }
    }

    for (mut text, marker) in &mut hits {
        if let Some(rule) = projection.rules.get(marker.0) {
            let want = format!("{} 次命中", rule.hit_count);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    for (mut text, marker) in &mut proxies {
        if let Some(rule) = projection.rules.get(marker.0)
            && text.0 != rule.proxy
        {
            text.0 = rule.proxy.clone();
        }
    }

    for (mut text, marker) in &mut payloads {
        if let Some(rule) = projection.rules.get(marker.0)
            && text.0 != rule.payload
        {
            text.0 = rule.payload.clone();
        }
    }

    for (mut text, marker) in &mut types {
        if let Some(rule) = projection.rules.get(marker.0) {
            let want = format!("[{}]", rule.rule_type);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    for (mut text, marker) in &mut provider_names {
        if let Some(provider) = projection.providers.get(marker.0)
            && text.0 != provider.name
        {
            text.0 = provider.name.clone();
        }
    }

    for (mut text, marker) in &mut provider_counts {
        if let Some(provider) = projection.providers.get(marker.0) {
            let want = format!("{} 条 ({})", provider.rule_count, provider.behavior);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    for (mut text, marker) in &mut provider_updates {
        if let Some(provider) = projection.providers.get(marker.0) {
            let want = format!("更新: {}", provider.updated_at);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
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
