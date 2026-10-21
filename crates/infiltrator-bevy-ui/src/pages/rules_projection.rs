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
use crate::pages::command_outcome::CommandOutcomes;
use crate::pages::rules_outcome::fold_provider_failures;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeTextColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::switch::SwitchPosition;
use infiltrator_domain::rules::matrix::{RuleTypeFamily, matrix_family, matrix_label};
use infiltrator_presentation::rule_provider_projection::{
    default_action, etag_support_line, provider_count, provider_fingerprint_line,
    provider_lifecycle_line, published_truncation, rules_summary,
};
use infiltrator_presentation::rule_row_projection::{row_hits_copy, row_hits_key};
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

/// DUAL-11-01: chip node behind a rule item's type label; its ink restamps
/// from the shared type family.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTypeBadge(pub usize);

/// Marker on a rule row's enable/disable switch knob; the payload is the row
/// index. The knob's rest position is the shared switch's on/off fact.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleEnabledKnob(pub usize);

/// Marker for a rule provider name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderNameText(pub usize);

/// Marker for a rule provider count text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCountText(pub usize);

/// Marker for a rule provider update time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderUpdatedText(pub usize);

/// DUAL-11-01: the semantic hue of a rule-type family.
///
/// The family is the shared catalogue's fact; the hue is the surface's
/// presentation of it, so the badge reads as a tag instead of one flat pill.
pub(crate) fn rule_type_chip_token(rule_type: &str) -> Token {
    match matrix_family(rule_type) {
        RuleTypeFamily::Host => Token::Accent,
        RuleTypeFamily::Geo => Token::Success,
        RuleTypeFamily::Address => Token::Warning,
        RuleTypeFamily::Process => Token::Danger,
        RuleTypeFamily::Port => Token::Accent,
        _ => Token::InkDim,
    }
}

/// The semantic hue of a rule's target action.
///
/// A blocked target reads as a failure, a direct target reads as the allowed
/// path, and a proxy group stays the neutral ink of an ordinary name.
pub(crate) fn rule_target_token(target: &str) -> Token {
    let normalized = target.trim().to_ascii_uppercase();
    if normalized == "DIRECT" {
        Token::Success
    } else if normalized.starts_with("REJECT") {
        Token::Danger
    } else {
        Token::Ink
    }
}

/// The ink of a rule row's hit statistics, by the row's own state.
pub(crate) fn rule_hits_token(enabled: bool, shadowed: bool) -> Token {
    if shadowed {
        Token::Warning
    } else if enabled {
        Token::InkDim
    } else {
        Token::DisabledInk
    }
}

pub(crate) fn apply_rules_projection(
    update: On<RulesProjectionUpdated>,
    mut last: Option<ResMut<LastRulesProjection>>,
    locale: Res<UiLocale>,
    mut commands: Commands,
    targets: RuleProjectionTargets,
    outcomes: Option<Res<CommandOutcomes>>,
) {
    let RuleProjectionTargets {
        type_inks,
        mut lines,
        mut hits,
        hit_inks,
        mut proxies,
        proxy_inks,
        mut payloads,
        mut types,
        mut knobs,
        mut provider_names,
        mut provider_counts,
        mut provider_updates,
    } = targets;

    let mut projection = update.0.clone();
    if let Some(outcomes) = outcomes.as_deref() {
        fold_provider_failures(&mut projection, outcomes);
    }
    let projection = &projection;
    // The badge's ink follows the shared type family: the chip reads as a tag
    // of that family instead of one flat blue pill.
    for (entity, current, marker) in &type_inks {
        let Some(rule) = projection.rules.get(marker.0) else {
            continue;
        };
        let want = rule_type_chip_token(&rule.rule_type);
        if current.is_none_or(|current| current.0 != want.theme_token()) {
            commands
                .entity(entity)
                .insert(ThemeTextColor(want.theme_token()));
        }
    }

    for (mut text, line) in &mut lines {
        let want = match line.0 {
            RulesLineKind::Summary => rules_summary(
                projection.total_rules,
                projection.providers.len(),
                &Lang(locale.code()),
            ),
            RulesLineKind::DefaultAction => {
                default_action(&projection.default_action, &Lang(locale.code()))
            }
            RulesLineKind::Truncation => published_truncation(
                projection.truncated_rule_count,
                projection.rule_publish_limit,
                &Lang(locale.code()),
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
                &Lang(locale.code()),
            );
            if text.0 != want {
                text.0 = want;
            }
        }
    }

    // The hit statistics are a chip of their own: their ink follows the row's
    // state, so a disabled or shadowed row says so without another word.
    for (entity, current, marker) in &hit_inks {
        let Some(rule) = projection.rules.get(marker.0) else {
            continue;
        };
        let want = rule_hits_token(rule.is_enabled, rule.is_shadowed);
        if current.is_none_or(|current| current.0 != want.theme_token()) {
            commands
                .entity(entity)
                .insert(ThemeTextColor(want.theme_token()));
        }
    }

    for (mut text, marker) in &mut proxies {
        if let Some(rule) = projection.rules.get(marker.0)
            && text.0 != rule.proxy
        {
            text.0 = rule.proxy.clone();
        }
    }

    // The target action carries its own semantic hue: a blocked target reads
    // as a failure, a direct target as the allowed path.
    for (entity, current, marker) in &proxy_inks {
        let Some(rule) = projection.rules.get(marker.0) else {
            continue;
        };
        let want = rule_target_token(&rule.proxy);
        if current.is_none_or(|current| current.0 != want.theme_token()) {
            commands
                .entity(entity)
                .insert(ThemeTextColor(want.theme_token()));
        }
    }

    // The row's enable/disable switch: the knob's rest position is the shared
    // control's on/off fact, so it follows the row's published state.
    for (mut knob, marker) in &mut knobs {
        let Some(rule) = projection.rules.get(marker.0) else {
            continue;
        };
        let want = SwitchPosition::from_enabled(rule.is_enabled);
        if knob.0 != want {
            knob.0 = want;
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
            let want = provider_count(
                provider.rule_count,
                &provider.behavior,
                &Lang(locale.code()),
            );
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
                &Lang(locale.code()),
            );
            if text.0 != value {
                text.0 = value;
            }
        }
    }
}
