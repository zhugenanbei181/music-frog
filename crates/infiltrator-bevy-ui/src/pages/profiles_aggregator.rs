//! DUAL-08 multi-subscription aggregator card (多订阅节点聚合器).
//!
//! The card is a pure projection of the shared `ProfilesPageSnapshot`:
//! the source checklist, the cleaning switches, and the preview counters /
//! region clusters / group cascade / YAML structure all render the shared
//! `AggregationReport`. Clicking preview, save, template or re-aggregate
//! submits the shared command; the surface never deduplicates, clusters, or
//! synthesizes groups locally.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::Query;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;

use crate::pages::profiles::ProfilesProjection;
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationDraft, AggregationRenameRule, AggregationReport,
};

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

/// Which aggregation switch a checkbox wrapper carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AggregatorSwitchKind {
    /// DUAL-08-02: drop fingerprint-identical nodes across sources.
    #[default]
    Deduplicate,
    /// DUAL-08-03: normalise names so geo clustering can bucket them.
    GeoCluster,
    /// DUAL-08-04/08-05: synthesize region groups + master cascade.
    GenerateGroups,
    /// Strip emoji characters from node names before grouping.
    RemoveEmojis,
    /// DUAL-08-09: drop nodes failing the required-field precheck.
    AvailabilityPrecheck,
    /// DUAL-08-12: make the generated profile the active profile.
    ActivateAfterCreate,
}

/// Marker for one cleaning/topology switch wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorSwitch(pub AggregatorSwitchKind);

/// Marker for the DUAL-08-08 rename-rules text-field wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorRenamesField;

/// Marker for the DUAL-08-10 custom group name text-field wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorCustomGroupNameField;

/// Marker for the DUAL-08-10 custom group keywords text-field wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorCustomGroupKeywordsField;

/// Marker for the "append custom group" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddAggregatorCustomGroupButton;

/// Marker for the "clear custom groups" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearAggregatorCustomGroupsButton;

/// Marker for the appended custom-group preview line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorCustomGroupsText;

/// Which shared projection line a preview text carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AggregatorPreviewKind {
    #[default]
    Counters,
    Regions,
    Groups,
    Yaml,
    Templates,
}

/// Marker for one restamped preview line (DUAL-08-11 viewport included).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorPreviewText(pub AggregatorPreviewKind);

/// Marker for the DUAL-08-13 template name text-field wrapper.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorTemplateNameField;

/// Marker for the "save as template" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveAggregationTemplateButton;

/// Marker for the "use template" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UseAggregationTemplateButton;

/// Marker for the DUAL-08-07 "re-aggregate" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReAggregateTemplateButton;

/// Marker for the "delete template" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeleteAggregationTemplateButton;

/// Marker for the wizard status line (validation feedback).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AggregatorStatusText;

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
        "聚合预览：输入 {} · 去重 {} · 归一化 {} · 规则重命名 {} · 预检剔除 {} · 输出 {} 节点 · 区域 {}{}{}",
        report.input_nodes,
        report.duplicates_removed,
        report.renamed_nodes,
        report.rule_renamed_nodes,
        report.invalid_nodes_removed,
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

/// DUAL-08-04/08-05/08-10: the synthesized group cascade as one preview block.
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
            let custom = if group.is_custom { " [自定义]" } else { "" };
            format!(
                "{}{}{} · {} · {} 成员",
                group.name,
                master,
                custom,
                kind,
                group.members.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// DUAL-08-11: the generated YAML structure, rendered from the shared report.
pub fn aggregation_yaml_preview(report: Option<&AggregationReport>) -> String {
    let Some(report) = report else {
        return "聚合 YAML 结构：等待预览".to_owned();
    };
    let total = report.yaml.lines().count();
    format!(
        "聚合 YAML 结构（共 {total} 行）:\n{}",
        report.yaml_preview(40)
    )
}

/// DUAL-08-10: the appended custom groups awaiting the next submit.
pub fn aggregation_custom_groups(groups: &[AggregationCustomGroup]) -> String {
    if groups.is_empty() {
        return "自定义策略组：尚未追加".to_owned();
    }
    groups
        .iter()
        .map(|group| {
            let keywords = if group.member_keywords.is_empty() {
                "全部节点".to_owned()
            } else {
                group.member_keywords.join(", ")
            };
            format!(
                "自定义策略组：{} · {} · {}",
                group.name, group.group_type, keywords
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// DUAL-08-13: the persisted template library with its target profiles.
pub fn aggregation_templates(projection: &ProfilesProjection) -> String {
    if !projection.aggregation_templates_available {
        return "历史聚合模板：宿主未提供模板存储（不支持）".to_owned();
    }
    if projection.aggregation_templates.is_empty() {
        return "历史聚合模板：暂无（填写模板名称后点击「保存为模板」）".to_owned();
    }
    projection
        .aggregation_templates
        .iter()
        .map(|template| {
            format!(
                "{} → {}（更新于 {}，{} 个源）",
                template.name,
                template.draft.target_name,
                template.updated_at,
                template.draft.source_profiles.len()
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
    let yaml = aggregation_yaml_preview(report);
    let templates = aggregation_templates(projection);
    // Mount-time view of the appended custom groups: the last shared preview
    // when there is one, otherwise an empty editor state (the wizard's own
    // accumulation restamps this line on every add/clear/template action).
    let custom_groups = aggregation_custom_groups(
        report
            .map(|report| report.draft.custom_groups.as_slice())
            .unwrap_or_default(),
    );
    let name = report
        .map(|report| report.draft.target_name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Aggregated-Profiles".to_owned());
    let renames = report
        .map(|report| AggregationRenameRule::to_text(&report.draft.rename_rules))
        .unwrap_or_default();

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
            AggregatorSwitch(AggregatorSwitchKind::Deduplicate)
            Children [ ( { checkbox_scene(
                "跨订阅节点自动去重".to_owned(),
                switch_of(report, |draft| draft.deduplicate),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorSwitch(AggregatorSwitchKind::GeoCluster)
            Children [ ( { checkbox_scene(
                "区域节点自动归类".to_owned(),
                switch_of(report, |draft| draft.geo_cluster),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorSwitch(AggregatorSwitchKind::GenerateGroups)
            Children [ ( { checkbox_scene(
                "生成区域测速策略组".to_owned(),
                switch_of(report, |draft| draft.generate_groups),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorSwitch(AggregatorSwitchKind::RemoveEmojis)
            Children [ ( { checkbox_scene(
                "清洗节点名 emoji".to_owned(),
                switch_of(report, |draft| draft.remove_emojis),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorSwitch(AggregatorSwitchKind::AvailabilityPrecheck)
            Children [ ( { checkbox_scene(
                "节点可用性预检过滤".to_owned(),
                switch_of(report, |draft| draft.availability_precheck),
                palette,
            ) } ) ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
            Node { align_items: AlignItems::Center }
            AggregatorSwitch(AggregatorSwitchKind::ActivateAfterCreate)
            Children [ ( { checkbox_scene(
                "创建后设为当前配置".to_owned(),
                switch_of(report, |draft| draft.activate_after_create),
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
                Node { width: percent(100) }
                AggregatorRenamesField
                Children [ ( { text_field_with_placeholder_scene(
                    renames,
                    "节点重命名规则: 模式 => 替换（多条用 ; 分隔）".to_owned(),
                    palette,
                ) } ) ]
            }),
            Box::new(bsn! {
                Node { width: percent(100) }
                AggregatorCustomGroupNameField
                Children [ ( { text_field_with_placeholder_scene(
                    String::new(),
                    "自定义策略组名称 (例如: 流媒体专用)".to_owned(),
                    palette,
                ) } ) ]
            }),
            Box::new(bsn! {
                Node { width: percent(100) }
                AggregatorCustomGroupKeywordsField
                Children [ ( { text_field_with_placeholder_scene(
                    String::new(),
                    "成员关键词，逗号分隔（留空 = 全部节点）".to_owned(),
                    palette,
                ) } ) ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
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
                        AddAggregatorCustomGroupButton
                        Children [
                            ( Text({ "追加自定义策略组".to_owned() }) TextRole(Role::Body) ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        ClearAggregatorCustomGroupsButton
                        Children [
                            ( Text({ "清空自定义策略组".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                ]
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
                Node { width: percent(100) }
                AggregatorTemplateNameField
                Children [ ( { text_field_with_placeholder_scene(
                    String::new(),
                    "模板名称（复用/重新聚合均按此名称查找）".to_owned(),
                    palette,
                ) } ) ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
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
                        SaveAggregationTemplateButton
                        Children [
                            ( Text({ "保存为模板".to_owned() }) TextRole(Role::Body) ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        UseAggregationTemplateButton
                        Children [
                            ( Text({ "复用模板".to_owned() }) TextRole(Role::Body) ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        ReAggregateTemplateButton
                        Children [
                            ( Text({ "重新聚合".to_owned() }) TextRole(Role::Body) ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        DeleteAggregationTemplateButton
                        Children [
                            ( Text({ "删除模板".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
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
                        AggregatorPreviewText(AggregatorPreviewKind::Counters)
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ regions.clone() })
                        AggregatorPreviewText(AggregatorPreviewKind::Regions)
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ groups.clone() })
                        AggregatorPreviewText(AggregatorPreviewKind::Groups)
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ yaml.clone() })
                        AggregatorPreviewText(AggregatorPreviewKind::Yaml)
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ templates.clone() })
                        AggregatorPreviewText(AggregatorPreviewKind::Templates)
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ custom_groups.clone() })
                        AggregatorCustomGroupsText
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ "聚合向导：编辑后点击预览，可保存为新配置或设为当前配置".to_owned() })
                        AggregatorStatusText
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
pub(super) fn sync_aggregation_preview(
    update: On<crate::pages::profiles::ProfilesProjectionUpdated>,
    mut lines: Query<(&mut Text, &AggregatorPreviewText)>,
) {
    let projection = &update.0;
    let report = projection.aggregation.as_ref();
    for (mut line, marker) in &mut lines {
        line.0 = match marker.0 {
            AggregatorPreviewKind::Counters => aggregation_counters(report),
            AggregatorPreviewKind::Regions => aggregation_regions(report),
            AggregatorPreviewKind::Groups => aggregation_groups(report),
            AggregatorPreviewKind::Yaml => aggregation_yaml_preview(report),
            AggregatorPreviewKind::Templates => aggregation_templates(projection),
        };
    }
}
