//! DUAL-08 multi-subscription aggregator card (多订阅节点聚合器).
//!
//! The card is a pure projection of the shared `ProfilesPageSnapshot`:
//! the source checklist, the cleaning switches, and the preview counters /
//! region clusters / group cascade all render the shared
//! `AggregationReport`. Clicking preview or save submits the shared command;
//! the surface never deduplicates, clusters, or synthesizes groups locally.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::ProfilesProjection;
use infiltrator_contract::aggregator::{AggregationDraft, AggregationReport};

/// Marker for the profile aggregator card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileAggregatorRoot;

/// Marker for the "preview aggregation" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewAggregationButton;

/// Marker for the "save as new profile" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveAggregatedProfileButton;

/// Marker for the aggregated profile name text-field wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorNameField;

/// Marker for one source-profile checkbox wrapper.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct AggregatorSourceToggle(
    /// Tree order of the row (the profile list is a projection of the store).
    pub usize,
    /// Source profile name carried by the row, so the submitted draft never
    /// depends on the projection being current at click time.
    pub String,
);

/// Marker for the DUAL-08-02 cross-source dedup checkbox wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorDeduplicateToggle;

/// Marker for the DUAL-08-03 geo-clustering checkbox wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorGeoClusterToggle;

/// Marker for the DUAL-08-04/08-05 group-generation checkbox wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorGenerateGroupsToggle;

/// Marker for the node-name emoji cleaning checkbox wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorRemoveEmojisToggle;

/// Marker for the shared preview counters line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorCountersText;

/// Marker for the multi-line region-cluster preview.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorRegionsText;

/// Marker for the multi-line group-cascade preview.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorGroupsText;

/// DUAL-08: the counters line for one shared report.
pub fn aggregation_counters(report: Option<&AggregationReport>) -> String {
    let Some(report) = report else {
        return "聚合预览：尚未生成（点击「预览聚合结果」）".to_owned();
    };
    let master = report
        .master_group()
        .map(|group| format!(" · 主选择器级联 {} 项", group.members.len()))
        .unwrap_or_default();
    let missing = if report.missing_sources.is_empty() {
        String::new()
    } else {
        format!(" · 未能读取: {}", report.missing_sources.join(" / "))
    };
    format!(
        "聚合预览：输入 {} · 去重 {} · 归一化 {} · 输出 {} 节点 · 区域 {}{}{}",
        report.input_nodes,
        report.duplicates_removed,
        report.renamed_nodes,
        report.total_nodes,
        report.regions.len(),
        master,
        missing
    )
}

/// DUAL-08-03: every region cluster as one preview line.
pub fn aggregation_regions(report: Option<&AggregationReport>) -> String {
    let Some(report) = report else {
        return "区域归类：等待预览".to_owned();
    };
    if report.regions.is_empty() {
        return "区域归类：未识别到任何 ISO 区域".to_owned();
    }
    report
        .regions
        .iter()
        .map(|region| {
            format!(
                "{} {} {} → {}（{} 节点）",
                region.flag,
                region.iso,
                region.label,
                region.group_name,
                region.node_names.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// DUAL-08-04/08-05: the synthesized group cascade as one preview block.
pub fn aggregation_groups(report: Option<&AggregationReport>) -> String {
    let Some(report) = report else {
        return "策略组拓扑：等待预览".to_owned();
    };
    if report.groups.is_empty() {
        return "策略组拓扑：未生成策略组".to_owned();
    }
    report
        .groups
        .iter()
        .map(|group| {
            let kind = if group.group_type == "url-test" {
                "自动测速"
            } else {
                "手动选择"
            };
            let master = if group.is_master {
                " [主选择器]"
            } else {
                ""
            };
            format!(
                "{}{} · {} · {} 成员",
                group.name,
                master,
                kind,
                group.members.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn selected_source_of(report: Option<&AggregationReport>, name: &str) -> bool {
    match report {
        Some(report) => report.draft.source_profiles.iter().any(|s| s == name),
        // No preview yet: the wizard defaults to every source selected.
        None => true,
    }
}

fn switch_of(report: Option<&AggregationReport>, read: impl Fn(&AggregationDraft) -> bool) -> bool {
    report.map(|report| read(&report.draft)).unwrap_or(true)
}

/// Multi-profile aggregator card scene.
pub fn profiles_aggregator_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let report = projection.aggregation.as_ref();
    let counters = aggregation_counters(report);
    let regions = aggregation_regions(report);
    let groups = aggregation_groups(report);
    let name = report
        .map(|report| report.draft.target_name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Aggregated-Profiles".to_owned());

    let source_rows: Vec<Box<dyn Scene>> = projection
        .profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let checked = selected_source_of(report, &profile.name);
            let label = profile.name.clone();
            let source_name = profile.name.clone();
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                }
                AggregatorSourceToggle(index, source_name)
                Children [
                    ( { checkbox_scene(label, checked, palette) } ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    let switch_rows: Vec<Box<dyn Scene>> = vec![
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorDeduplicateToggle
            Children [ ( { checkbox_scene(
                "跨订阅节点自动去重".to_owned(),
                switch_of(report, |draft| draft.deduplicate),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorGeoClusterToggle
            Children [ ( { checkbox_scene(
                "区域节点自动归类".to_owned(),
                switch_of(report, |draft| draft.geo_cluster),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorGenerateGroupsToggle
            Children [ ( { checkbox_scene(
                "生成区域测速策略组".to_owned(),
                switch_of(report, |draft| draft.generate_groups),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorRemoveEmojisToggle
            Children [ ( { checkbox_scene(
                "清洗节点名 emoji".to_owned(),
                switch_of(report, |draft| draft.remove_emojis),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
    ];

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                ProfileAggregatorRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            ( Text({ "多订阅节点聚合器 (Profile Aggregator)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Button
                                PreviewAggregationButton
                                Children [
                                    ( Text({ "预览聚合结果".to_owned() }) TextRole(Role::Body) ),
                                ]
                            ),
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                Button
                                SaveAggregatedProfileButton
                                Children [
                                    ( Text({ "保存为新配置".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node { width: percent(100) }
                AggregatorNameField
                Children [ ( { text_field_with_placeholder_scene(
                    name,
                    "聚合配置名称 (例如: Aggregated-All)".to_owned(),
                    palette,
                ) } ) ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    { switch_rows },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Text({ counters.clone() })
                        AggregatorCountersText
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ regions.clone() })
                        AggregatorRegionsText
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ groups.clone() })
                        AggregatorGroupsText
                        TextRole(Role::Caption)
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ "聚合源订阅（勾选后点击预览）".to_owned() }) TextRole(Role::BodyStrong) ),
                    { source_rows },
                ]
            }),
        ],
        palette,
    )
}

/// DUAL-08: restamp the preview from the shared report carried by the surface
/// snapshot. The card keeps no second aggregation state.
#[allow(clippy::type_complexity)]
pub(super) fn sync_aggregation_preview(
    update: On<crate::pages::profiles::ProfilesProjectionUpdated>,
    mut counters: Query<
        &mut Text,
        (
            With<AggregatorCountersText>,
            bevy::ecs::query::Without<AggregatorRegionsText>,
            bevy::ecs::query::Without<AggregatorGroupsText>,
        ),
    >,
    mut regions: Query<
        &mut Text,
        (
            With<AggregatorRegionsText>,
            bevy::ecs::query::Without<AggregatorCountersText>,
            bevy::ecs::query::Without<AggregatorGroupsText>,
        ),
    >,
    mut groups: Query<
        &mut Text,
        (
            With<AggregatorGroupsText>,
            bevy::ecs::query::Without<AggregatorCountersText>,
            bevy::ecs::query::Without<AggregatorRegionsText>,
        ),
    >,
) {
    let report = update.0.aggregation.as_ref();
    let counters_text = aggregation_counters(report);
    for mut line in &mut counters {
        line.0 = counters_text.clone();
    }
    let regions_text = aggregation_regions(report);
    for mut line in &mut regions {
        line.0 = regions_text.clone();
    }
    let groups_text = aggregation_groups(report);
    for mut line in &mut groups {
        line.0 = groups_text.clone();
    }
}

fn read_text_field(
    parents: &Query<&Children, With<AggregatorNameField>>,
    text_fields: &Query<&TextField>,
) -> Option<String> {
    parents
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

fn toggle_checked<M: Component>(
    parents: &Query<&Children, With<M>>,
    checkboxes: &Query<&Checked>,
) -> bool {
    parents
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok())
}

/// Collect the edited draft from the card widgets. The shared application
/// re-validates every field, so the surface sends raw values.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn draft_from_widgets(
    name_field: &Query<&Children, With<AggregatorNameField>>,
    source_toggles: &Query<(&AggregatorSourceToggle, &Children)>,
    dedup: &Query<&Children, With<AggregatorDeduplicateToggle>>,
    geo: &Query<&Children, With<AggregatorGeoClusterToggle>>,
    groups: &Query<&Children, With<AggregatorGenerateGroupsToggle>>,
    emojis: &Query<&Children, With<AggregatorRemoveEmojisToggle>>,
    text_fields: &Query<&TextField>,
    checkboxes: &Query<&Checked>,
) -> AggregationDraft {
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

    AggregationDraft {
        source_profiles,
        target_name: read_text_field(name_field, text_fields).unwrap_or_default(),
        deduplicate: toggle_checked(dedup, checkboxes),
        deduplicate_names: true,
        geo_cluster: toggle_checked(geo, checkboxes),
        generate_groups: toggle_checked(groups, checkboxes),
        remove_emojis: toggle_checked(emojis, checkboxes),
    }
}

/// DUAL-08-01/08-11: submit the edited draft for a real shared preview.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_preview_aggregation(
    activate: On<Activate>,
    buttons: Query<(), With<PreviewAggregationButton>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    dedup: Query<&Children, With<AggregatorDeduplicateToggle>>,
    geo: Query<&Children, With<AggregatorGeoClusterToggle>>,
    groups: Query<&Children, With<AggregatorGenerateGroupsToggle>>,
    emojis: Query<&Children, With<AggregatorRemoveEmojisToggle>>,
    text_fields: Query<&TextField>,
    checkboxes: Query<&Checked>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let draft = draft_from_widgets(
        &name_field,
        &source_toggles,
        &dedup,
        &geo,
        &groups,
        &emojis,
        &text_fields,
        &checkboxes,
    );
    handle.submit(UiCommand::PreviewProfileAggregation { draft });
}

/// DUAL-08-06: submit the edited draft for materialisation into a new profile.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn on_save_aggregated_profile(
    activate: On<Activate>,
    buttons: Query<(), With<SaveAggregatedProfileButton>>,
    name_field: Query<&Children, With<AggregatorNameField>>,
    source_toggles: Query<(&AggregatorSourceToggle, &Children)>,
    dedup: Query<&Children, With<AggregatorDeduplicateToggle>>,
    geo: Query<&Children, With<AggregatorGeoClusterToggle>>,
    groups: Query<&Children, With<AggregatorGenerateGroupsToggle>>,
    emojis: Query<&Children, With<AggregatorRemoveEmojisToggle>>,
    text_fields: Query<&TextField>,
    checkboxes: Query<&Checked>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let draft = draft_from_widgets(
        &name_field,
        &source_toggles,
        &dedup,
        &geo,
        &groups,
        &emojis,
        &text_fields,
        &checkboxes,
    );
    handle.submit(UiCommand::CreateAggregatedProfile { draft });
}
