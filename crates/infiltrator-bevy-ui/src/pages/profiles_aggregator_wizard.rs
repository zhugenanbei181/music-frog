//! DUAL-08 aggregator wizard input collection and shared command submission.
//!
//! Split from `profiles_aggregator.rs` (the card scene) under the source line
//! budget: this module owns the surface-local input accumulation
//! ([`AggregatorComposerState`]) and the observers that collect the draft from
//! the widget tree and submit it through the shared command bus.

#[path = "profiles_aggregator_wizard_query_access.rs"]
pub mod query_access;
use self::query_access::{
    AggregationGroupControls, AggregationPreviewControls, AggregationSaveControls,
    AggregationTemplateLoadControls, AggregationTemplateSaveControls,
    OnAddAggregatorCustomGroupCustomLinesFilter, OnPreviewAggregationStatusFilter,
};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::LastProfilesProjection;
use crate::pages::profiles_aggregator::{
    AggregatorNameField, AggregatorRenamesField, AggregatorSourceToggle, AggregatorSwitch,
    AggregatorSwitchKind, AggregatorTemplateNameField, ClearAggregatorCustomGroupsButton,
    DeleteAggregationTemplateButton, ReAggregateTemplateButton,
};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Checkbox, ValueChange};
use infiltrator_application::aggregation_preview_projection::aggregation_custom_groups;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldState;
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationDraft, AggregationRenameRule, AggregationTemplate,
};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct AggregatorStatusParts {
    text: &'static mut Text,
    copy: &'static mut LocalizedText,
}

/// SDK checkbox changes edit the local wizard draft; persistence waits for submit.
pub(super) fn on_aggregation_checkbox_changed(
    event: On<ValueChange<bool>>,
    checkboxes: Query<&ChildOf, With<Checkbox>>,
    sources: Query<(), With<AggregatorSourceToggle>>,
    switches: Query<(), With<AggregatorSwitch>>,
    mut commands: Commands,
) {
    let Ok(parent) = checkboxes.get(event.source) else {
        return;
    };
    if !sources.contains(parent.parent()) && !switches.contains(parent.parent()) {
        return;
    }
    if event.value {
        commands.entity(event.source).insert(Checked);
    } else {
        commands.entity(event.source).remove::<Checked>();
    }
}

/// DUAL-08-10: surface-local accumulation of the custom groups typed into the
/// wizard. Everything else on the card is a projection of the shared report.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AggregatorComposerState {
    pub custom_groups: Vec<AggregationCustomGroup>,
}

/// DUAL-08-10: restamp the appended custom groups line. The composer resource
/// is the surface-local accumulation for input the projection cannot carry
/// before it is submitted.
fn refresh_custom_groups_text(
    groups: &[AggregationCustomGroup],
    lines: &mut Query<&mut Text, OnAddAggregatorCustomGroupCustomLinesFilter>,
) {
    let text = aggregation_custom_groups(groups, UiLocale::default().code());
    for mut line in lines.iter_mut() {
        line.0 = text.clone();
    }
}

fn read_text_field<M: Component>(
    parents: &Query<&Children, With<M>>,
    text_fields: &Query<&TextField>,
) -> Option<String> {
    parents
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

/// The checked state of the named switch wrapper.
fn switch_state(
    switches: &Query<(&AggregatorSwitch, &Children)>,
    checkboxes: &Query<&Checked>,
    kind: AggregatorSwitchKind,
) -> bool {
    switches
        .iter()
        .filter(|(switch, _)| switch.0 == kind)
        .flat_map(|(_, children)| children.iter())
        .any(|child| checkboxes.get(*child).is_ok())
}

/// Apply a checked state to the named switch wrapper.
fn write_switch(
    commands: &mut Commands,
    switches: &Query<(&AggregatorSwitch, &Children)>,
    checkboxes: &Query<&Checked>,
    kind: AggregatorSwitchKind,
    checked: bool,
) {
    for (switch, children) in switches.iter() {
        if switch.0 != kind {
            continue;
        }
        for child in children.iter() {
            if checkboxes.get(*child).is_err() {
                continue;
            }
            let mut entity = commands.entity(*child);
            if checked {
                entity.insert(Checked);
            } else {
                entity.remove::<Checked>();
            }
        }
    }
}

/// Overwrite the text of the first field under `M` (used when a template is
/// applied to the wizard).
fn write_text_field<M: Component>(
    parents: &Query<&Children, With<M>>,
    text_fields: &mut Query<&mut TextField>,
    value: &str,
) {
    for children in parents.iter() {
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child) {
                field.0 = TextFieldState::new(value.to_owned());
                return;
            }
        }
    }
}

/// Collect the edited draft from the card widgets. The shared application
/// re-validates every field, so the surface sends raw values; a malformed
/// rename line is reported back to the caller instead of being dropped.
fn draft_from_widgets(
    name_field: &Query<&Children, With<AggregatorNameField>>,
    renames_field: &Query<&Children, With<AggregatorRenamesField>>,
    source_toggles: &Query<(&AggregatorSourceToggle, &Children)>,
    switches: &Query<(&AggregatorSwitch, &Children)>,
    text_fields: &Query<&TextField>,
    checkboxes: &Query<&Checked>,
    custom_groups: &[AggregationCustomGroup],
) -> Result<AggregationDraft, String> {
    let mut checked_sources: Vec<(&usize, &String)> = source_toggles
        .iter()
        .filter(|(_, children)| children.iter().any(|child| checkboxes.get(*child).is_ok()))
        .map(|(toggle, _)| (&toggle.0, &toggle.1))
        .collect();
    checked_sources.sort_by_key(|(index, _)| **index);
    let source_profiles = checked_sources
        .into_iter()
        .map(|(_, name)| name.clone())
        .collect();

    let switch = |kind: AggregatorSwitchKind| switch_state(switches, checkboxes, kind);
    Ok(AggregationDraft {
        source_profiles,
        target_name: read_text_field(name_field, text_fields).unwrap_or_default(),
        deduplicate: switch(AggregatorSwitchKind::Deduplicate),
        deduplicate_names: true,
        geo_cluster: switch(AggregatorSwitchKind::GeoCluster),
        generate_groups: switch(AggregatorSwitchKind::GenerateGroups),
        remove_emojis: switch(AggregatorSwitchKind::RemoveEmojis),
        rename_rules: AggregationRenameRule::parse_list(
            &read_text_field(renames_field, text_fields).unwrap_or_default(),
        )?,
        custom_groups: custom_groups.to_vec(),
        availability_precheck: switch(AggregatorSwitchKind::AvailabilityPrecheck),
        activate_after_create: switch(AggregatorSwitchKind::ActivateAfterCreate),
    })
}

/// DUAL-08-01/08-11: submit the edited draft for a real shared preview.
pub(super) fn on_preview_aggregation(
    activate: On<Activate>,
    composer: Option<Res<AggregatorComposerState>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: AggregationPreviewControls,
) {
    let AggregationPreviewControls {
        buttons,
        name_field,
        renames_field,
        source_toggles,
        switches,
        text_fields,
        checkboxes,
        mut status,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let custom_groups = composer
        .map(|composer| composer.custom_groups.clone())
        .unwrap_or_default();
    match draft_from_widgets(
        &name_field,
        &renames_field,
        &source_toggles,
        &switches,
        &text_fields,
        &checkboxes,
        &custom_groups,
    ) {
        Ok(draft) => {
            report_status(
                &mut status,
                LocalizedText::plain("aggregation_preview_submitted"),
            );
            handle.submit(UiCommand::PreviewProfileAggregation { draft });
        }
        Err(line) => report_status(
            &mut status,
            LocalizedText::new("aggregator_rename_invalid", vec![("line", line)]),
        ),
    }
}

/// DUAL-08-06: submit the edited draft for materialisation into a new profile.
pub(super) fn on_save_aggregated_profile(
    activate: On<Activate>,
    composer: Option<Res<AggregatorComposerState>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: AggregationSaveControls,
) {
    let AggregationSaveControls {
        buttons,
        name_field,
        renames_field,
        source_toggles,
        switches,
        text_fields,
        checkboxes,
        mut status,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let custom_groups = composer
        .map(|composer| composer.custom_groups.clone())
        .unwrap_or_default();
    match draft_from_widgets(
        &name_field,
        &renames_field,
        &source_toggles,
        &switches,
        &text_fields,
        &checkboxes,
        &custom_groups,
    ) {
        Ok(draft) => {
            report_status(
                &mut status,
                LocalizedText::plain("aggregation_save_submitted"),
            );
            handle.submit(UiCommand::CreateAggregatedProfile { draft });
        }
        Err(line) => report_status(
            &mut status,
            LocalizedText::new("aggregator_rename_invalid", vec![("line", line)]),
        ),
    }
}

/// DUAL-08-10: append the typed custom group to the wizard's accumulation.
pub(super) fn on_add_aggregator_custom_group(
    activate: On<Activate>,
    mut composer: Option<ResMut<AggregatorComposerState>>,
    targets: AggregationGroupControls,
) {
    let AggregationGroupControls {
        buttons,
        name_field,
        keywords_field,
        text_fields,
        mut status,
        mut custom_lines,
    } = targets;

    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(composer) = composer.as_mut() else {
        return;
    };
    let name = read_text_field(&name_field, &text_fields)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if name.is_empty() {
        report_status(
            &mut status,
            LocalizedText::plain("aggregation_custom_name_required"),
        );
        return;
    }
    let keywords = read_text_field(&keywords_field, &text_fields)
        .unwrap_or_default()
        .split([',', '，', ';'])
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
        .map(str::to_owned)
        .collect();
    composer.custom_groups.push(AggregationCustomGroup {
        name,
        group_type: "select".to_owned(),
        member_keywords: keywords,
    });
    refresh_custom_groups_text(&composer.custom_groups, &mut custom_lines);
    report_status(
        &mut status,
        LocalizedText::plain("aggregation_custom_added"),
    );
}

/// DUAL-08-10: drop the wizard's appended custom groups.
pub(super) fn on_clear_aggregator_custom_groups(
    activate: On<Activate>,
    buttons: Query<(), With<ClearAggregatorCustomGroupsButton>>,
    mut composer: Option<ResMut<AggregatorComposerState>>,
    mut custom_lines: Query<&mut Text, OnAddAggregatorCustomGroupCustomLinesFilter>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let cleared = Vec::new();
    if let Some(composer) = composer.as_mut() {
        composer.custom_groups.clear();
        refresh_custom_groups_text(&composer.custom_groups, &mut custom_lines);
        return;
    }
    refresh_custom_groups_text(&cleared, &mut custom_lines);
}

/// DUAL-08-13: save the edited draft as a template under the typed name.
pub(super) fn on_save_aggregation_template(
    activate: On<Activate>,
    composer: Option<Res<AggregatorComposerState>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: AggregationTemplateSaveControls,
) {
    let AggregationTemplateSaveControls {
        buttons,
        template_field,
        name_field,
        renames_field,
        source_toggles,
        switches,
        text_fields,
        checkboxes,
        mut status,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(name) = read_text_field(&template_field, &text_fields)
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
    else {
        report_status(
            &mut status,
            LocalizedText::plain("aggregation_template_name_required"),
        );
        return;
    };
    let custom_groups = composer
        .map(|composer| composer.custom_groups.clone())
        .unwrap_or_default();
    match draft_from_widgets(
        &name_field,
        &renames_field,
        &source_toggles,
        &switches,
        &text_fields,
        &checkboxes,
        &custom_groups,
    ) {
        Ok(draft) => {
            report_status(
                &mut status,
                LocalizedText::plain("aggregation_template_save_submitted"),
            );
            handle.submit(UiCommand::SaveAggregationTemplate { name, draft });
        }
        Err(line) => report_status(
            &mut status,
            LocalizedText::new("aggregator_rename_invalid", vec![("line", line)]),
        ),
    }
}

/// DUAL-08-13: prefill the wizard from the named saved template.
pub(super) fn on_use_aggregation_template(
    activate: On<Activate>,
    last: Option<Res<LastProfilesProjection>>,
    composer: Option<Res<AggregatorComposerState>>,
    mut commands: Commands,
    targets: AggregationTemplateLoadControls,
) {
    let AggregationTemplateLoadControls {
        buttons,
        template_field,
        name_field,
        renames_field,
        source_toggles,
        switches,
        mut text_fields,
        checkboxes,
        mut status,
        mut custom_lines,
    } = targets;

    if buttons.get(activate.entity).is_err() {
        return;
    }
    let name = template_field
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().trim().to_owned());
    let template = name
        .as_deref()
        .and_then(|name| {
            last.as_ref()?
                .0
                .as_ref()?
                .aggregation_templates
                .iter()
                .find(|template| template.name == name)
        })
        .cloned();
    let Some(template) = template else {
        report_status(
            &mut status,
            LocalizedText::plain("aggregation_template_not_found"),
        );
        return;
    };
    let draft = template.draft.clone();

    write_text_field(&name_field, &mut text_fields, &draft.target_name);
    write_text_field(
        &renames_field,
        &mut text_fields,
        &AggregationRenameRule::to_text(&draft.rename_rules),
    );
    for (toggle, children) in source_toggles.iter() {
        let checked = draft.source_profiles.iter().any(|name| name == &toggle.1);
        for child in children.iter() {
            if checkboxes.get(*child).is_err() {
                continue;
            }
            let mut entity = commands.entity(*child);
            if checked {
                entity.insert(Checked);
            } else {
                entity.remove::<Checked>();
            }
        }
    }
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::Deduplicate,
        draft.deduplicate,
    );
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::GeoCluster,
        draft.geo_cluster,
    );
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::GenerateGroups,
        draft.generate_groups,
    );
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::RemoveEmojis,
        draft.remove_emojis,
    );
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::AvailabilityPrecheck,
        draft.availability_precheck,
    );
    write_switch(
        &mut commands,
        &switches,
        &checkboxes,
        AggregatorSwitchKind::ActivateAfterCreate,
        draft.activate_after_create,
    );

    let mut composer = composer
        .map(|composer| composer.clone())
        .unwrap_or_default();
    composer.custom_groups = draft.custom_groups.clone();
    refresh_custom_groups_text(&composer.custom_groups, &mut custom_lines);
    commands.insert_resource(composer);
    report_status(
        &mut status,
        LocalizedText::plain("aggregation_template_loaded"),
    );
}

/// DUAL-08-07: re-aggregate the profile the named template produced.
pub(super) fn on_reaggregate_template(
    activate: On<Activate>,
    buttons: Query<(), With<ReAggregateTemplateButton>>,
    template_field: Query<&Children, With<AggregatorTemplateNameField>>,
    text_fields: Query<&TextField>,
    last: Option<Res<LastProfilesProjection>>,
    mut status: Query<AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(template) = find_template(&template_field, &text_fields, last.as_deref()) else {
        report_status(
            &mut status,
            LocalizedText::plain("aggregation_template_not_found"),
        );
        return;
    };
    report_status(
        &mut status,
        LocalizedText::plain("aggregation_template_regenerate_submitted"),
    );
    handle.submit(UiCommand::ReAggregateProfile {
        template_name: template.name.clone(),
    });
}

/// DUAL-08-13: delete the named saved template.
pub(super) fn on_delete_aggregation_template(
    activate: On<Activate>,
    buttons: Query<(), With<DeleteAggregationTemplateButton>>,
    template_field: Query<&Children, With<AggregatorTemplateNameField>>,
    text_fields: Query<&TextField>,
    last: Option<Res<LastProfilesProjection>>,
    mut status: Query<AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(template) = find_template(&template_field, &text_fields, last.as_deref()) else {
        report_status(
            &mut status,
            LocalizedText::plain("aggregation_template_not_found"),
        );
        return;
    };
    report_status(
        &mut status,
        LocalizedText::plain("aggregation_template_delete_submitted"),
    );
    handle.submit(UiCommand::DeleteAggregationTemplate {
        name: template.name.clone(),
    });
}

fn find_template(
    template_field: &Query<&Children, With<AggregatorTemplateNameField>>,
    text_fields: &Query<&TextField>,
    last: Option<&LastProfilesProjection>,
) -> Option<AggregationTemplate> {
    let name = read_text_field(template_field, text_fields)?;
    let projection = last?.0.as_ref()?;
    projection
        .aggregation_templates
        .iter()
        .find(|template| template.name == name.trim())
        .cloned()
}

fn report_status(
    status: &mut Query<AggregatorStatusParts, OnPreviewAggregationStatusFilter>,
    message: LocalizedText,
) {
    for mut parts in status.iter_mut() {
        parts.text.0 = message.render(&UiLocale::default());
        *parts.copy = message.clone();
    }
}
