//! Scoped native component access for rules projection systems.

use super::{
    ProviderCountText, ProviderNameText, ProviderUpdatedText, RuleHitText, RulePayloadText,
    RuleProxyText, RuleTypeBadge, RuleTypeText, RulesLine,
};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::LocalizedText;

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionLinesFilter {
    with_rules_line: With<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionHitsFilter {
    with_rule_hit_text: With<RuleHitText>,
    without_rules_line: Without<RulesLine>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionProxiesFilter {
    with_rule_proxy_text: With<RuleProxyText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionPayloadsFilter {
    with_rule_payload_text: With<RulePayloadText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionTypesFilter {
    with_rule_type_text: With<RuleTypeText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionProviderNamesFilter {
    with_provider_name_text: With<ProviderNameText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionProviderCountsFilter {
    with_provider_count_text: With<ProviderCountText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionProviderUpdatesFilter {
    with_provider_updated_text: With<ProviderUpdatedText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
}

#[derive(SystemParam)]
pub struct RuleProjectionTargets<'w, 's> {
    pub(super) type_fills: Query<'w, 's, (&'static mut BackgroundColor, &'static RuleTypeBadge)>,
    pub(super) lines: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static RulesLine,
            Option<&'static mut LocalizedText>,
        ),
        ApplyRulesProjectionLinesFilter,
    >,
    pub(super) hits: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static RuleHitText,
            &'static mut LocalizedText,
        ),
        ApplyRulesProjectionHitsFilter,
    >,
    pub(super) proxies: Query<
        'w,
        's,
        (&'static mut Text, &'static RuleProxyText),
        ApplyRulesProjectionProxiesFilter,
    >,
    pub(super) payloads: Query<
        'w,
        's,
        (&'static mut Text, &'static RulePayloadText),
        ApplyRulesProjectionPayloadsFilter,
    >,
    pub(super) types:
        Query<'w, 's, (&'static mut Text, &'static RuleTypeText), ApplyRulesProjectionTypesFilter>,
    pub(super) provider_names: Query<
        'w,
        's,
        (&'static mut Text, &'static ProviderNameText),
        ApplyRulesProjectionProviderNamesFilter,
    >,
    pub(super) provider_counts: Query<
        'w,
        's,
        (&'static mut Text, &'static ProviderCountText),
        ApplyRulesProjectionProviderCountsFilter,
    >,
    pub(super) provider_updates: Query<
        'w,
        's,
        (&'static mut Text, &'static ProviderUpdatedText),
        ApplyRulesProjectionProviderUpdatesFilter,
    >,
}
