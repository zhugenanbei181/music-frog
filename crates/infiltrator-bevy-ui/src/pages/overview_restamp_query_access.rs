//! Scoped native component access for overview restamp systems.

use super::{
    ActiveExitText, OverviewMasterSwitchButton, OverviewMasterSwitchText,
    SubscriptionQuotaProgress, SubscriptionQuotaText,
};
use crate::pages::overview::{
    OverviewCardState, OverviewChip, OverviewLine, OverviewReloadMask, OverviewReloadMaskText,
    OverviewStatusCard,
};
use crate::pages::overview_public_ip::PublicIpText;
use crate::pages::overview_topology::{TopologyStageButton, TopologyText};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::text::TextColor;
use bevy::ui::prelude::Node;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::chart::topology::TopologyPlate;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;

#[derive(QueryFilter)]
pub struct ApplyOverviewProjectionReloadMasksFilter {
    with_overview_reload_mask: With<OverviewReloadMask>,
    without_subscription_quota_progress: Without<SubscriptionQuotaProgress>,
}

#[derive(QueryFilter)]
pub struct ApplyOverviewProjectionReloadMaskTextsFilter {
    with_overview_reload_mask_text: With<OverviewReloadMaskText>,
    without_overview_line: Without<OverviewLine>,
    without_topology_text: Without<TopologyText>,
    without_active_exit_text: Without<ActiveExitText>,
    without_subscription_quota_text: Without<SubscriptionQuotaText>,
    without_overview_master_switch_text: Without<OverviewMasterSwitchText>,
    without_public_ip_text: Without<PublicIpText>,
    without_stat_chip_value: Without<StatChipValue>,
}

#[derive(SystemParam)]
pub struct OverviewProjectionTargets<'w, 's> {
    pub(super) lines: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static mut TextColor,
            &'static OverviewLine,
            Option<&'static mut AccessibilityNode>,
        ),
        Without<OverviewChip>,
    >,
    pub(super) values: Query<'w, 's, &'static mut Text, StatValueFilter>,
    pub(super) dynamic_texts: Query<'w, 's, OverviewDynamicText, OverviewDynamicTextFilter>,
    pub(super) quota_progress: Query<'w, 's, &'static mut Node, QuotaProgressFilter>,
    pub(super) master_buttons: Query<'w, 's, &'static mut OverviewMasterSwitchButton>,
    pub(super) topology_buttons: Query<'w, 's, &'static mut TopologyStageButton>,

    pub(super) cards: Query<'w, 's, &'static mut OverviewCardState, With<OverviewStatusCard>>,
    pub(super) chips: Query<
        'w,
        's,
        (
            Entity,
            &'static OverviewChip,
            Option<&'static mut AccessibilityNode>,
        ),
    >,
    pub(super) reload_masks:
        Query<'w, 's, &'static mut Node, ApplyOverviewProjectionReloadMasksFilter>,
    pub(super) reload_mask_texts: Query<
        'w,
        's,
        (&'static mut Text, &'static mut LocalizedText),
        ApplyOverviewProjectionReloadMaskTextsFilter,
    >,
    pub(super) groups: Query<'w, 's, &'static Children>,
    pub(super) charts: Query<'w, 's, &'static mut ChartPlate>,
    pub(super) topology_charts: Query<'w, 's, &'static mut TopologyPlate>,
}

#[derive(QueryFilter)]
pub struct StatValueFilter {
    with_stat_chip_value: With<StatChipValue>,
    without_overview_line: Without<OverviewLine>,
}

#[derive(QueryFilter)]
pub struct TopologyOutputFilter {
    with_topology_text: With<TopologyText>,
    without_overview_line: Without<OverviewLine>,
    without_public_ip_text: Without<PublicIpText>,
    without_overview_reload_mask_text: Without<OverviewReloadMaskText>,
    without_stat_chip_value: Without<StatChipValue>,
}

#[derive(QueryFilter)]
pub struct ExitOutputFilter {
    with_active_exit_text: With<ActiveExitText>,
    without_overview_line: Without<OverviewLine>,
    without_topology_text: Without<TopologyText>,
    without_public_ip_text: Without<PublicIpText>,
    without_overview_reload_mask_text: Without<OverviewReloadMaskText>,
    without_stat_chip_value: Without<StatChipValue>,
}

#[derive(QueryFilter)]
pub struct QuotaOutputFilter {
    with_subscription_quota_text: With<SubscriptionQuotaText>,
    without_overview_line: Without<OverviewLine>,
    without_topology_text: Without<TopologyText>,
    without_active_exit_text: Without<ActiveExitText>,
    without_public_ip_text: Without<PublicIpText>,
    without_overview_reload_mask_text: Without<OverviewReloadMaskText>,
    without_stat_chip_value: Without<StatChipValue>,
}

#[derive(QueryFilter)]
pub struct QuotaProgressFilter {
    with_subscription_quota_progress: With<SubscriptionQuotaProgress>,
    without_overview_reload_mask: Without<OverviewReloadMask>,
}

#[derive(QueryFilter)]
pub struct MasterSwitchOutputFilter {
    with_overview_master_switch_text: With<OverviewMasterSwitchText>,
    without_overview_line: Without<OverviewLine>,
    without_topology_text: Without<TopologyText>,
    without_active_exit_text: Without<ActiveExitText>,
    without_subscription_quota_text: Without<SubscriptionQuotaText>,
    without_public_ip_text: Without<PublicIpText>,
    without_overview_reload_mask_text: Without<OverviewReloadMaskText>,
    without_stat_chip_value: Without<StatChipValue>,
}

#[derive(QueryFilter)]
pub struct PublicIpOutputFilter {
    with_public_ip_text: With<PublicIpText>,
    without_overview_line: Without<OverviewLine>,
    without_topology_text: Without<TopologyText>,
    without_active_exit_text: Without<ActiveExitText>,
    without_subscription_quota_text: Without<SubscriptionQuotaText>,
    without_overview_master_switch_text: Without<OverviewMasterSwitchText>,
    without_overview_reload_mask_text: Without<OverviewReloadMaskText>,
    without_stat_chip_value: Without<StatChipValue>,
}
/// Union of the original role queries, preserving their exclusion semantics.
#[derive(QueryFilter)]
pub struct OverviewDynamicTextFilter {
    roles: Or<(
        TopologyOutputFilter,
        ExitOutputFilter,
        QuotaOutputFilter,
        MasterSwitchOutputFilter,
        PublicIpOutputFilter,
    )>,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct OverviewDynamicText {
    pub(super) text: &'static mut Text,
    pub(super) ink: Option<&'static mut TextColor>,
    pub(super) topology: Option<&'static TopologyText>,
    pub(super) exit: Option<&'static ActiveExitText>,
    pub(super) quota: Option<&'static SubscriptionQuotaText>,
    pub(super) master: Option<&'static OverviewMasterSwitchText>,
    pub(super) public_ip: Option<&'static PublicIpText>,
}
