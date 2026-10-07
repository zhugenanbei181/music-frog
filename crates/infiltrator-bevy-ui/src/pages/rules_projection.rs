//! Typed restamp markers and the in-place projection observer for the Rules
//! page (分流规则页的原位重盖).
//!
//! The page scene holds one node per rendered line carrying a marker below;
//! [`apply_rules_projection`] rewrites those texts when
//! [`RulesProjectionUpdated`](super::rules::RulesProjectionUpdated) fires, so a
//! data refresh never rebuilds the tree. The page plugin registers this observer once at assembly.

#[path = "rules_projection_query_access.rs"]
pub mod query_access;
use self::query_access::RuleProjectionTargets;

use super::rules::{LastRulesProjection, RulesProjectionUpdated};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use infiltrator_application::rule_provider_projection::{
    default_action, etag_support_line, provider_count, provider_fingerprint_line,
    provider_lifecycle_line, published_truncation, rules_summary,
};
use infiltrator_application::rule_row_projection::{row_hits_copy, row_hits_key};
use infiltrator_application::rule_statistics_projection::project_statistics;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_domain::rules::matrix::{RuleTypeFamily, matrix_family, matrix_label};
use infiltrator_shared::locales::Lang;

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

/// Marker for a rule item's displayed order.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleIndexText(pub usize);

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

/// DUAL-11-01: chip fill for a rule-type family, from palette tokens only.
pub(crate) fn rule_type_chip_fill(rule_type: &str, palette: &UiPalette) -> Color {
    match matrix_family(rule_type) {
        RuleTypeFamily::Host => palette.accent_container,
        RuleTypeFamily::Geo => palette.icon_tile,
        RuleTypeFamily::Address => palette.pressed_bg,
        RuleTypeFamily::Process => palette.border,
        _ => palette.surface_elevated,
    }
}

pub(crate) fn apply_rules_projection(
    update: On<RulesProjectionUpdated>,
    mut last: Option<ResMut<LastRulesProjection>>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    targets: RuleProjectionTargets,
) {
    let RuleProjectionTargets {
        mut type_fills,
        mut lines,
        mut hits,
        mut proxies,
        mut payloads,
        mut types,
        mut provider_names,
        mut provider_counts,
        mut provider_updates,
    } = targets;

    let projection = &update.0;

    for (mut fill, marker) in &mut type_fills {
        if let Some(rule) = projection.rules.get(marker.0) {
            let want = rule_type_chip_fill(&rule.rule_type, &palette);
            if fill.0 != want {
                fill.0 = want;
            }
        }
    }

    for (mut text, line, copy) in &mut lines {
        let want = match line.0 {
            RulesLineKind::Summary => rules_summary(
                projection.total_rules,
                projection.providers.len(),
                locale.code(),
            ),
            RulesLineKind::DefaultAction => {
                default_action(&projection.default_action, locale.code())
            }
            RulesLineKind::HitAudit => {
                let projected = project_statistics(projection.hit_audit.as_ref(), locale.code());
                let value = projected.summary(locale.code());
                if let Some(mut copy) = copy {
                    *copy = LocalizedText::new(projected.key, projected.params);
                }
                value
            }
            RulesLineKind::Truncation => published_truncation(
                projection.truncated_rule_count,
                projection.rule_publish_limit,
                locale.code(),
            ),
            RulesLineKind::EtagSupport => {
                etag_support_line(&projection.etag_support, &Lang(locale.code()))
            }
        };
        if text.0 != want {
            text.0 = want;
        }
    }

    for (mut text, marker, mut copy) in &mut hits {
        if let Some(rule) = projection.rules.get(marker.0) {
            let updated_copy = LocalizedText::new(
                row_hits_key(rule.hit_count, rule.is_enabled, rule.is_shadowed),
                vec![(
                    "count",
                    rule.hit_count
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                )],
            );
            if *copy != updated_copy {
                *copy = updated_copy;
            }
            let want = row_hits_copy(
                rule.hit_count,
                rule.is_enabled,
                rule.is_shadowed,
                locale.code(),
            );
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
            let want = matrix_label(&rule.rule_type).to_owned();
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
            let want = provider_count(provider.rule_count, &provider.behavior, locale.code());
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    for (mut text, marker) in &mut provider_updates {
        if let Some(provider) = projection.providers.get(marker.0) {
            let want = provider_lifecycle_line(
                &provider.updated_at,
                provider.source_url.as_deref(),
                provider.refresh_interval_secs,
                &Lang(locale.code()),
            );
            let want = provider
                .cache_fingerprint
                .as_ref()
                .map(|observation| {
                    format!(
                        "{want} · {}",
                        provider_fingerprint_line(observation, &Lang(locale.code()))
                    )
                })
                .unwrap_or(want);
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

pub(crate) fn refresh_hit_copy(
    last: Res<LastRulesProjection>,
    locale: Res<UiLocale>,
    mut hits: Query<(&RuleHitText, &mut Text, &mut LocalizedText)>,
) {
    let Some(projection) = &last.0 else {
        return;
    };
    for (marker, mut text, mut copy) in &mut hits {
        if let Some(rule) = projection.rules.get(marker.0) {
            let updated_copy = LocalizedText::new(
                row_hits_key(rule.hit_count, rule.is_enabled, rule.is_shadowed),
                vec![(
                    "count",
                    rule.hit_count
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                )],
            );
            if *copy != updated_copy {
                *copy = updated_copy;
            }
            let value = row_hits_copy(
                rule.hit_count,
                rule.is_enabled,
                rule.is_shadowed,
                locale.code(),
            );
            if text.0 != value {
                text.0 = value;
            }
        }
    }
}
