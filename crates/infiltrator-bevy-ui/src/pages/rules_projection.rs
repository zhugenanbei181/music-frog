//! Typed restamp markers and the in-place projection observer for the Rules
//! page (分流规则页的原位重盖).
//!
//! The page scene holds one node per rendered line carrying a marker below;
//! [`apply_rules_projection`] rewrites those texts when
//! [`RulesProjectionUpdated`](super::rules::RulesProjectionUpdated) fires, so a
//! data refresh never rebuilds the tree. This module also owns the
//! once-per-world bind guard so the observer is registered exactly once.

use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::ui::prelude::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_domain::rules::matrix::{RuleTypeFamily, matrix_family};

use super::rules::{
    LastRulesProjection, RulesProjectionUpdated, etag_support_label, hit_audit_label,
    provider_updated_label, rule_hit_label,
};

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
pub struct RulesPageBound;

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
    /// Shared live hit audit: total hits, dead/shadowed rules, CIDR overlaps.
    HitAudit,
    /// DUAL-11-08: honest publish-cap note for the rendered rule list.
    Truncation,
    /// DUAL-11-05: the kernel's declared top-level `etag-support` capability.
    EtagSupport,
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

/// DUAL-11-01: chip node behind a rule item's type label; its fill is restamped
/// from the shared type family.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTypeBadge(pub usize);

/// Marker for a rule provider name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderNameText(pub usize);

/// Marker for a rule provider count text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCountText(pub usize);

/// Marker for a rule provider update time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderUpdatedText(pub usize);

/// Mount hook for the rules page root: register the projection observer once.
/// Returns `true` when this call performed the binding.
pub fn bind_rules_projection(world: &mut DeferredWorld<'_>) -> bool {
    if world.get_resource::<RulesPageBound>().is_some() {
        return false;
    }
    let mut commands = world.commands();
    commands.insert_resource(RulesPageBound);
    // DUAL-12-08: the tracer reverse-apply observer reads the last projection,
    // so the store must exist from the moment the page is bound.
    commands.insert_resource(LastRulesProjection::default());
    commands.add_observer(apply_rules_projection);
    true
}

/// DUAL-11-08: the honest publish-cap note. Empty when the rendered list is the
/// complete profile list. The rendered list itself is a real virtual window
/// (`sync_rules_window`), so this note now only reports the publisher's cap.
pub(crate) fn truncation_label(omitted: Option<usize>, limit: usize) -> String {
    match omitted {
        Some(omitted) if omitted > 0 => {
            format!("发布视口已截断 · 已省略 {omitted} 条 (发布上限 {limit} 条)")
        }
        _ => String::new(),
    }
}

/// DUAL-11-01: chip fill for a rule-type family, from palette tokens only.
pub(crate) fn rule_type_chip_fill(rule_type: &str, palette: &UiPalette) -> bevy::color::Color {
    match matrix_family(rule_type) {
        RuleTypeFamily::Host => palette.accent_container,
        RuleTypeFamily::Geo => palette.icon_tile,
        RuleTypeFamily::Address => palette.pressed_bg,
        RuleTypeFamily::Process => palette.border,
        _ => palette.surface_elevated,
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_rules_projection(
    update: On<RulesProjectionUpdated>,
    mut last: Option<ResMut<LastRulesProjection>>,
    palette: Res<UiPalette>,
    mut type_fills: Query<(&mut BackgroundColor, &RuleTypeBadge)>,
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

    for (mut fill, marker) in &mut type_fills {
        if let Some(rule) = projection.rules.get(marker.0) {
            let want = rule_type_chip_fill(&rule.rule_type, &palette);
            if fill.0 != want {
                fill.0 = want;
            }
        }
    }

    for (mut text, line) in &mut lines {
        let want = match line.0 {
            RulesLineKind::Summary => format!(
                "分流规则 · 共 {} 条规则 ({} 个规则集 / 命中统计开启)",
                projection.total_rules,
                projection.providers.len()
            ),
            RulesLineKind::DefaultAction => {
                format!("最终匹配目标: {}", projection.default_action)
            }
            RulesLineKind::HitAudit => hit_audit_label(&projection.hit_audit),
            RulesLineKind::Truncation => truncation_label(
                projection.truncated_rule_count,
                projection.rule_publish_limit,
            ),
            RulesLineKind::EtagSupport => etag_support_label(&projection.etag_support),
        };
        if text.0 != want {
            text.0 = want;
        }
    }

    for (mut text, marker) in &mut hits {
        if let Some(rule) = projection.rules.get(marker.0) {
            let want = rule_hit_label(rule);
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
            let want = format!(
                "[{}]",
                infiltrator_domain::rules::matrix::matrix_label(&rule.rule_type)
            );
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
            let want = provider_updated_label(provider);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}
