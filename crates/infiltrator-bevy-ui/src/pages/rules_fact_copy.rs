//! Locale replay reads the existing projection; it never reloads facts or edits drafts.
use crate::pages::rules::LastRulesProjection;
use crate::pages::rules_mrs::{MrsItemText, MrsStatusText, ProviderCacheText};
use crate::pages::rules_projection::{
    ProviderCountText, ProviderUpdatedText, RulesLine, RulesLineKind,
};
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::rule_mrs_projection::{
    mrs_acceleration_item_label, mrs_acceleration_status_line,
};
use infiltrator_application::rule_provider_projection::{
    default_action, etag_support_line, provider_cache_line, provider_count,
    provider_fingerprint_line, provider_lifecycle_line, published_truncation, rules_summary,
};
use infiltrator_application::rule_statistics_projection::project_statistics;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_shared::locales::Lang;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct RuleFactText {
    text: &'static mut Text,
    line: Option<&'static RulesLine>,
    count: Option<&'static ProviderCountText>,
    updated: Option<&'static ProviderUpdatedText>,
    status: Option<&'static MrsStatusText>,
    item: Option<&'static MrsItemText>,
    cache: Option<&'static ProviderCacheText>,
}
#[derive(QueryFilter)]
pub struct ProviderCopyFilter {
    provider: Or<(With<ProviderCountText>, With<ProviderUpdatedText>)>,
}
#[derive(QueryFilter)]
pub struct MrsCopyFilter {
    mrs: Or<(
        With<MrsStatusText>,
        With<MrsItemText>,
        With<ProviderCacheText>,
    )>,
}
#[derive(QueryFilter)]
pub struct RuleFactFilter {
    facts: Or<(With<RulesLine>, ProviderCopyFilter, MrsCopyFilter)>,
}
pub fn sync(
    last: Res<LastRulesProjection>,
    locale: Res<UiLocale>,
    mut texts: Query<RuleFactText, RuleFactFilter>,
) {
    let Some(projection) = &last.0 else {
        return;
    };
    let lang = Lang(locale.code());
    for mut target in &mut texts {
        let value = if let Some(line) = target.line {
            Some(match line.0 {
                RulesLineKind::Summary => rules_summary(
                    projection.total_rules,
                    projection.providers.len(),
                    locale.code(),
                ),
                RulesLineKind::DefaultAction => {
                    default_action(&projection.default_action, locale.code())
                }
                RulesLineKind::HitAudit => {
                    project_statistics(projection.hit_audit.as_ref(), locale.code())
                        .summary(locale.code())
                }
                RulesLineKind::Truncation => published_truncation(
                    projection.truncated_rule_count,
                    projection.rule_publish_limit,
                    locale.code(),
                ),
                RulesLineKind::EtagSupport => etag_support_line(&projection.etag_support, &lang),
            })
        } else if let Some(count) = target.count {
            projection.providers.get(count.0).map(|provider| {
                provider_count(provider.rule_count, &provider.behavior, locale.code())
            })
        } else if let Some(updated) = target.updated {
            projection.providers.get(updated.0).map(|provider| {
                let lifecycle = provider_lifecycle_line(
                    &provider.updated_at,
                    provider.source_url.as_deref(),
                    provider.refresh_interval_secs,
                    &lang,
                );
                provider
                    .cache_fingerprint
                    .as_ref()
                    .map(|observation| {
                        format!(
                            "{lifecycle} · {}",
                            provider_fingerprint_line(observation, &lang)
                        )
                    })
                    .unwrap_or(lifecycle)
            })
        } else if target.status.is_some() {
            Some(mrs_acceleration_status_line(
                &lang,
                &projection.mrs_acceleration,
            ))
        } else if let Some(item) = target.item {
            projection
                .mrs_acceleration
                .items
                .get(item.0)
                .map(|item| mrs_acceleration_item_label(&lang, item))
        } else if target.cache.is_some() {
            Some(provider_cache_line(&projection.provider_cache, &lang))
        } else {
            None
        };
        if let Some(value) = value
            && target.text.0 != value
        {
            target.text.0 = value;
        }
    }
}
