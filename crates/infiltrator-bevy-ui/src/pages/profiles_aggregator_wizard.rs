//! DUAL-08 aggregator wizard input collection and shared command submission.
//!
//! Split from `profiles_aggregator.rs` (the card scene) under the source line
//! budget: this module owns the surface-local input accumulation
//! ([`AggregatorComposerState`]) and the observers that collect the draft from
//! the widget tree and submit it through the shared command bus.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldState;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles_aggregator::{
    AddAggregatorCustomGroupButton, AggregatorCustomGroupKeywordsField,
    AggregatorCustomGroupNameField, AggregatorCustomGroupsText, AggregatorNameField,
    AggregatorRenamesField, AggregatorSourceToggle, AggregatorStatusText, AggregatorSwitch,
    AggregatorSwitchKind, AggregatorTemplateNameField, ClearAggregatorCustomGroupsButton,
    DeleteAggregationTemplateButton, PreviewAggregationButton, ReAggregateTemplateButton,
    SaveAggregatedProfileButton, SaveAggregationTemplateButton, UseAggregationTemplateButton,
    aggregation_custom_groups,
};
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationDraft, AggregationRenameRule,
};

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
    lines: &mut Query<
        &mut Text,
        (
            With<AggregatorCustomGroupsText>,
            Without<AggregatorStatusText>,
        ),
    >,
) {
    let text = aggregation_custom_groups(groups);
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_preview_aggregation(
    activate: On<Activate>,
    buttons: Query<(), With<PreviewAggregationButton>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    renames_field: Query<&Children, With<AggregatorRenamesField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    switches: Query<(&AggregatorSwitch, &Children)>,
    text_fields: Query<&TextField>,
    checkboxes: Query<&Checked>,
    composer: Option<Res<AggregatorComposerState>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    handle: Option<Res<CommandSinkHandle>>,
) {
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
            report_status(&mut status, "已提交共享聚合预览");
            handle.submit(UiCommand::PreviewProfileAggregation { draft });
        }
        Err(line) => report_status(
            &mut status,
            &format!("重命名规则格式错误（应为 模式 => 替换）: {line}"),
        ),
    }
}

/// DUAL-08-06: submit the edited draft for materialisation into a new profile.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_save_aggregated_profile(
    activate: On<Activate>,
    buttons: Query<(), With<SaveAggregatedProfileButton>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    renames_field: Query<&Children, With<AggregatorRenamesField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    switches: Query<(&AggregatorSwitch, &Children)>,
    text_fields: Query<&TextField>,
    checkboxes: Query<&Checked>,
    composer: Option<Res<AggregatorComposerState>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    handle: Option<Res<CommandSinkHandle>>,
) {
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
            report_status(&mut status, "已提交共享聚合落盘命令");
            handle.submit(UiCommand::CreateAggregatedProfile { draft });
        }
        Err(line) => report_status(
            &mut status,
            &format!("重命名规则格式错误（应为 模式 => 替换）: {line}"),
        ),
    }
}

/// DUAL-08-10: append the typed custom group to the wizard's accumulation.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_add_aggregator_custom_group(
    activate: On<Activate>,
    buttons: Query<(), With<AddAggregatorCustomGroupButton>>,
    name_field: Query<&Children, With<AggregatorCustomGroupNameField>>,
    keywords_field: Query<&Children, With<AggregatorCustomGroupKeywordsField>>,
    text_fields: Query<&TextField>,
    mut composer: Option<ResMut<AggregatorComposerState>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    mut custom_lines: Query<
        &mut Text,
        (
            With<AggregatorCustomGroupsText>,
            Without<AggregatorStatusText>,
        ),
    >,
) {
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
        report_status(&mut status, "请先填写自定义策略组名称");
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
    report_status(&mut status, "已追加自定义策略组，预览/保存后生效");
}

/// DUAL-08-10: drop the wizard's appended custom groups.
pub(super) fn on_clear_aggregator_custom_groups(
    activate: On<Activate>,
    buttons: Query<(), With<ClearAggregatorCustomGroupsButton>>,
    mut composer: Option<ResMut<AggregatorComposerState>>,
    mut custom_lines: Query<
        &mut Text,
        (
            With<AggregatorCustomGroupsText>,
            Without<AggregatorStatusText>,
        ),
    >,
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_save_aggregation_template(
    activate: On<Activate>,
    buttons: Query<(), With<SaveAggregationTemplateButton>>,
    template_field: Query<&Children, With<AggregatorTemplateNameField>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    renames_field: Query<&Children, With<AggregatorRenamesField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    switches: Query<(&AggregatorSwitch, &Children)>,
    text_fields: Query<&TextField>,
    checkboxes: Query<&Checked>,
    composer: Option<Res<AggregatorComposerState>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    handle: Option<Res<CommandSinkHandle>>,
) {
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
        report_status(&mut status, "请先填写模板名称");
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
            report_status(&mut status, "已提交模板保存命令");
            handle.submit(UiCommand::SaveAggregationTemplate { name, draft });
        }
        Err(line) => report_status(
            &mut status,
            &format!("重命名规则格式错误（应为 模式 => 替换）: {line}"),
        ),
    }
}

/// DUAL-08-13: prefill the wizard from the named saved template.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_use_aggregation_template(
    activate: On<Activate>,
    buttons: Query<(), With<UseAggregationTemplateButton>>,
    template_field: Query<&Children, With<AggregatorTemplateNameField>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    renames_field: Query<&Children, With<AggregatorRenamesField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    switches: Query<(&AggregatorSwitch, &Children)>,
    mut text_fields: Query<&mut TextField>,
    checkboxes: Query<&Checked>,
    last: Option<Res<crate::pages::profiles::LastProfilesProjection>>,
    composer: Option<Res<AggregatorComposerState>>,
    mut commands: Commands,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    mut custom_lines: Query<
        &mut Text,
        (
            With<AggregatorCustomGroupsText>,
            Without<AggregatorStatusText>,
        ),
    >,
) {
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
        report_status(&mut status, "未找到该名称的已保存模板");
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
    report_status(&mut status, "已复用模板，可预览或保存");
}

/// DUAL-08-07: re-aggregate the profile the named template produced.
pub(super) fn on_reaggregate_template(
    activate: On<Activate>,
    buttons: Query<(), With<ReAggregateTemplateButton>>,
    template_field: Query<&Children, With<AggregatorTemplateNameField>>,
    text_fields: Query<&TextField>,
    last: Option<Res<crate::pages::profiles::LastProfilesProjection>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(template) = find_template(&template_field, &text_fields, last.as_deref()) else {
        report_status(&mut status, "未找到该名称的已保存模板");
        return;
    };
    report_status(&mut status, "已提交重新聚合命令");
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
    last: Option<Res<crate::pages::profiles::LastProfilesProjection>>,
    mut status: Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(template) = find_template(&template_field, &text_fields, last.as_deref()) else {
        report_status(&mut status, "未找到该名称的已保存模板");
        return;
    };
    report_status(&mut status, "已提交模板删除命令");
    handle.submit(UiCommand::DeleteAggregationTemplate {
        name: template.name.clone(),
    });
}

fn find_template(
    template_field: &Query<&Children, With<AggregatorTemplateNameField>>,
    text_fields: &Query<&TextField>,
    last: Option<&crate::pages::profiles::LastProfilesProjection>,
) -> Option<infiltrator_contract::aggregator::AggregationTemplate> {
    let name = read_text_field(template_field, text_fields)?;
    let projection = last?.0.as_ref()?;
    projection
        .aggregation_templates
        .iter()
        .find(|template| template.name == name.trim())
        .cloned()
}

fn report_status(
    status: &mut Query<
        &mut Text,
        (
            With<AggregatorStatusText>,
            Without<AggregatorCustomGroupsText>,
        ),
    >,
    message: &str,
) {
    for mut line in status.iter_mut() {
        line.0 = message.to_owned();
    }
}
