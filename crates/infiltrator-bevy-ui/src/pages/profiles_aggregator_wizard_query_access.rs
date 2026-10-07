//! Scoped native component access for profiles aggregator wizard systems.

use super::AggregatorStatusParts;
use crate::pages::profiles_aggregator::{
    AddAggregatorCustomGroupButton, AggregatorCustomGroupKeywordsField,
    AggregatorCustomGroupNameField, AggregatorCustomGroupsText, AggregatorNameField,
    AggregatorRenamesField, AggregatorSourceToggle, AggregatorStatusText, AggregatorSwitch,
    AggregatorTemplateNameField, PreviewAggregationButton, SaveAggregatedProfileButton,
    SaveAggregationTemplateButton, UseAggregationTemplateButton,
};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::text_input::TextField;

#[derive(QueryFilter)]
pub struct OnPreviewAggregationStatusFilter {
    with_aggregator_status_text: With<AggregatorStatusText>,
    without_aggregator_custom_groups_text: Without<AggregatorCustomGroupsText>,
}

#[derive(SystemParam)]
pub struct AggregationPreviewControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<PreviewAggregationButton>>,
    pub(super) name_field: Query<'w, 's, &'static Children, With<AggregatorNameField>>,
    pub(super) renames_field: Query<'w, 's, &'static Children, With<AggregatorRenamesField>>,
    pub(super) source_toggles: Query<'w, 's, (&'static AggregatorSourceToggle, &'static Children)>,
    pub(super) switches: Query<'w, 's, (&'static AggregatorSwitch, &'static Children)>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
    pub(super) status: Query<'w, 's, AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
}

#[derive(SystemParam)]
pub struct AggregationSaveControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<SaveAggregatedProfileButton>>,
    pub(super) name_field: Query<'w, 's, &'static Children, With<AggregatorNameField>>,
    pub(super) renames_field: Query<'w, 's, &'static Children, With<AggregatorRenamesField>>,
    pub(super) source_toggles: Query<'w, 's, (&'static AggregatorSourceToggle, &'static Children)>,
    pub(super) switches: Query<'w, 's, (&'static AggregatorSwitch, &'static Children)>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
    pub(super) status: Query<'w, 's, AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
}

#[derive(QueryFilter)]
pub struct OnAddAggregatorCustomGroupCustomLinesFilter {
    with_aggregator_custom_groups_text: With<AggregatorCustomGroupsText>,
    without_aggregator_status_text: Without<AggregatorStatusText>,
}

#[derive(SystemParam)]
pub struct AggregationGroupControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<AddAggregatorCustomGroupButton>>,
    pub(super) name_field: Query<'w, 's, &'static Children, With<AggregatorCustomGroupNameField>>,
    pub(super) keywords_field:
        Query<'w, 's, &'static Children, With<AggregatorCustomGroupKeywordsField>>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) status: Query<'w, 's, AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
    pub(super) custom_lines:
        Query<'w, 's, &'static mut Text, OnAddAggregatorCustomGroupCustomLinesFilter>,
}

#[derive(SystemParam)]
pub struct AggregationTemplateSaveControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<SaveAggregationTemplateButton>>,
    pub(super) template_field: Query<'w, 's, &'static Children, With<AggregatorTemplateNameField>>,
    pub(super) name_field: Query<'w, 's, &'static Children, With<AggregatorNameField>>,
    pub(super) renames_field: Query<'w, 's, &'static Children, With<AggregatorRenamesField>>,
    pub(super) source_toggles: Query<'w, 's, (&'static AggregatorSourceToggle, &'static Children)>,
    pub(super) switches: Query<'w, 's, (&'static AggregatorSwitch, &'static Children)>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
    pub(super) status: Query<'w, 's, AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
}

#[derive(SystemParam)]
pub struct AggregationTemplateLoadControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<UseAggregationTemplateButton>>,
    pub(super) template_field: Query<'w, 's, &'static Children, With<AggregatorTemplateNameField>>,
    pub(super) name_field: Query<'w, 's, &'static Children, With<AggregatorNameField>>,
    pub(super) renames_field: Query<'w, 's, &'static Children, With<AggregatorRenamesField>>,
    pub(super) source_toggles: Query<'w, 's, (&'static AggregatorSourceToggle, &'static Children)>,
    pub(super) switches: Query<'w, 's, (&'static AggregatorSwitch, &'static Children)>,
    pub(super) text_fields: Query<'w, 's, &'static mut TextField>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
    pub(super) status: Query<'w, 's, AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
    pub(super) custom_lines:
        Query<'w, 's, &'static mut Text, OnAddAggregatorCustomGroupCustomLinesFilter>,
}
