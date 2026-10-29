//! Scoped native component access for rules projection systems.

use super::{
    ProviderCountText, ProviderNameText, ProviderUpdatedText, RuleEnabledKnob, RuleHitText,
    RulePayloadText, RuleProxyText, RuleRowStatusChip, RuleRowStatusText, RuleTypeText, RulesLine,
};
use crate::pages::rules_outcome::RuleProviderRefreshButton;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::feathers::theme::ThemeTextColor;
use bevy::ui::prelude::Node;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::switch::SwitchKnob;

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionLinesFilter {
    with_rules_line: With<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_row_status_text: Without<RuleRowStatusText>,
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
    without_rule_row_status_text: Without<RuleRowStatusText>,
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

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionStatusChipsFilter {
    with_rule_row_status_chip: With<RuleRowStatusChip>,
}

#[derive(QueryFilter)]
pub struct ApplyRulesProjectionStatusTextsFilter {
    with_rule_row_status_text: With<RuleRowStatusText>,
    without_rules_line: Without<RulesLine>,
    without_rule_hit_text: Without<RuleHitText>,
    without_rule_proxy_text: Without<RuleProxyText>,
    without_rule_payload_text: Without<RulePayloadText>,
    without_rule_type_text: Without<RuleTypeText>,
    without_provider_name_text: Without<ProviderNameText>,
    without_provider_count_text: Without<ProviderCountText>,
    without_provider_updated_text: Without<ProviderUpdatedText>,
}

#[derive(SystemParam)]
pub struct RuleProjectionTargets<'w, 's> {
    pub(super) type_inks: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static ThemeTextColor>,
            &'static RuleTypeText,
        ),
        ApplyRulesProjectionTypesFilter,
    >,
    pub(super) lines:
        Query<'w, 's, (&'static mut Text, &'static RulesLine), ApplyRulesProjectionLinesFilter>,
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
    pub(super) hit_inks: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static ThemeTextColor>,
            &'static RuleHitText,
        ),
        ApplyRulesProjectionHitsFilter,
    >,
    /// The status chip's own box: the chip is shown only when the row has a
    /// status fact of its own.
    pub(super) status_chips: Query<
        'w,
        's,
        (&'static mut Node, &'static RuleRowStatusChip),
        ApplyRulesProjectionStatusChipsFilter,
    >,
    /// The status chip's copy.
    pub(super) status_texts: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static RuleRowStatusText,
            &'static mut LocalizedText,
        ),
        ApplyRulesProjectionStatusTextsFilter,
    >,
    pub(super) proxies: Query<
        'w,
        's,
        (&'static mut Text, &'static RuleProxyText),
        ApplyRulesProjectionProxiesFilter,
    >,
    pub(super) proxy_inks: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static ThemeTextColor>,
            &'static RuleProxyText,
        ),
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
    pub(super) knobs: Query<'w, 's, (&'static mut SwitchKnob, &'static RuleEnabledKnob)>,
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
    pub(super) provider_buttons: Query<
        'w,
        's,
        (
            &'static mut ButtonDisabled,
            &'static RuleProviderRefreshButton,
        ),
    >,
}
