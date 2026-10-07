//! DUAL-08 multi-subscription aggregator card (多订阅节点聚合器).
//!
//! The card is a pure projection of the shared `ProfilesPageSnapshot`:
//! the source checklist, the cleaning switches, and the preview counters /
//! region clusters / group cascade / YAML structure all render the shared
//! `AggregationReport`. Clicking preview, save, template or re-aggregate
//! submits the shared command; the surface never deduplicates, clusters, or
//! synthesizes groups locally.

use infiltrator_application::aggregation_preview_projection::{
    aggregation_counters, aggregation_custom_groups, aggregation_groups, aggregation_regions,
    aggregation_templates, aggregation_yaml_preview,
};

use crate::localized_widgets::{localized_checkbox_scene, localized_field_scene};
use crate::pages::profiles::LastProfilesProjection;
use crate::pages::profiles::{ProfilesProjection, ProfilesProjectionUpdated};
use crate::pages::profiles_aggregator_wizard::AggregatorComposerState;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
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
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::aggregator::{
    AggregationDraft, AggregationRenameRule, AggregationReport,
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
    let locale = UiLocale::default();
    let report = projection.aggregation.as_ref();
    let counters = aggregation_counters(report, locale.code());
    let regions = aggregation_regions(report, locale.code());
    let groups = aggregation_groups(report, locale.code());
    let yaml = aggregation_yaml_preview(report, locale.code());
    let templates = aggregation_templates(
        projection.aggregation_templates_available,
        &projection.aggregation_templates,
        locale.code(),
    );
    // Mount-time view of the appended custom groups: the last shared preview
    // when there is one, otherwise an empty editor state (the wizard's own
    // accumulation restamps this line on every add/clear/template action).
    let custom_groups = aggregation_custom_groups(
        report
            .map(|report| report.draft.custom_groups.as_slice())
            .unwrap_or_default(),
        locale.code(),
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
                                @{ checkbox_scene(label, checked, palette) }
                            ]
            }) as Box<dyn Scene>
        })
        .collect();

    let switch_rows: Vec<Box<dyn Scene>> = vec![
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::Deduplicate)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_dedup"),
                                switch_of(report, |draft| draft.deduplicate),
                                palette,
                        ) }
                    ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::GeoCluster)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_geo_cluster"),
                                switch_of(report, |draft| draft.geo_cluster),
                                palette,
                        ) }
                    ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::GenerateGroups)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_generate_groups"),
                                switch_of(report, |draft| draft.generate_groups),
                                palette,
                        ) }
                    ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::RemoveEmojis)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_remove_emojis"),
                                switch_of(report, |draft| draft.remove_emojis),
                                palette,
                        ) }
                    ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::AvailabilityPrecheck)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_availability_precheck"),
                                switch_of(report, |draft| draft.availability_precheck),
                                palette,
                        ) }
                    ]
        }) as Box<dyn Scene>,
        Box::new(bsn! {
                    Node { align_items: AlignItems::Center }
                    AggregatorSwitch(AggregatorSwitchKind::ActivateAfterCreate)
                    Children [
                        @{ localized_checkbox_scene(LocalizedText::plain("aggregator_activate_after_create"),
                                switch_of(report, |draft| draft.activate_after_create),
                                palette,
                        ) }
                    ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::FileText, 24.0, palette) }
                                    --
                                    LocalizedText::plain("profiles_aggregator_title") TextRole(Role::BodyStrong)
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
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
                                        LocalizedText::plain("aggregator_btn_preview") TextRole(Role::Body)
                                    ]
                                    --
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
                                        LocalizedText::plain("aggregator_btn_save") TextRole(Role::BodyStrong)
                                    ]
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            AggregatorNameField
                            Children [
                                @{ localized_field_scene(name, LocalizedText::plain("aggregator_name_placeholder"),
                                        palette,
                                ) }
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            AggregatorRenamesField
                            Children [
                                @{ localized_field_scene(renames, LocalizedText::plain("aggregator_renames_ph"),
                                        palette,
                                ) }
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            AggregatorCustomGroupNameField
                            Children [
                                @{ localized_field_scene(String::new(), LocalizedText::plain("aggregator_custom_name_ph"),
                                        palette,
                                ) }
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            AggregatorCustomGroupKeywordsField
                            Children [
                                @{ localized_field_scene(String::new(), LocalizedText::plain("aggregator_custom_keywords_ph"),
                                        palette,
                                ) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
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
                                    LocalizedText::plain("profiles_aggregator_add_group") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("profiles_aggregator_clear_groups") TextRole(Role::Body)
                                ]
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
                                { switch_rows }
                            ]
            }),
            Box::new(bsn! {
                            Node { width: percent(100) }
                            AggregatorTemplateNameField
                            Children [
                                @{ localized_field_scene(String::new(), LocalizedText::plain("field_aggregation_template"),
                                        palette,
                                ) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
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
                                    LocalizedText::plain("aggregator_template_save") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("profiles_aggregator_reuse_template") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("aggregator_template_reaggregate") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("profiles_aggregator_delete_template") TextRole(Role::Body)
                                ]
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
                                Text({ counters.clone() })
                                AggregatorPreviewText(AggregatorPreviewKind::Counters)
                                TextRole(Role::Caption)
                                --
                                Text({ regions.clone() })
                                AggregatorPreviewText(AggregatorPreviewKind::Regions)
                                TextRole(Role::Caption)
                                --
                                Text({ groups.clone() })
                                AggregatorPreviewText(AggregatorPreviewKind::Groups)
                                TextRole(Role::Caption)
                                --
                                Text({ yaml.clone() })
                                AggregatorPreviewText(AggregatorPreviewKind::Yaml)
                                TextRole(Role::Caption)
                                --
                                Text({ templates.clone() })
                                AggregatorPreviewText(AggregatorPreviewKind::Templates)
                                TextRole(Role::Caption)
                                --
                                Text({ custom_groups.clone() })
                                AggregatorCustomGroupsText
                                TextRole(Role::Caption)
                                --
                                LocalizedText::plain("profiles_aggregator_wizard_hint")
                                AggregatorStatusText
                                TextRole(Role::Caption)
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
                                LocalizedText::plain("profiles_aggregator_sources_label") TextRole(Role::BodyStrong)
                                --
                                { source_rows }
                            ]
            }),
        ],
        palette,
    )
}

/// DUAL-08: restamp the preview from the shared report carried by the surface
/// snapshot. The card keeps no second aggregation state.
pub(super) fn sync_aggregation_preview(
    update: On<ProfilesProjectionUpdated>,
    locale: Option<Res<UiLocale>>,
    mut lines: Query<(&mut Text, &AggregatorPreviewText)>,
) {
    let fallback = UiLocale::default();
    let locale = locale.as_deref().unwrap_or(&fallback);
    let projection = &update.0;
    for (mut line, marker) in &mut lines {
        line.0 = preview_line(marker.0, projection, locale.code());
    }
}

fn preview_line(
    kind: AggregatorPreviewKind,
    projection: &ProfilesProjection,
    code: &str,
) -> String {
    let report = projection.aggregation.as_ref();
    match kind {
        AggregatorPreviewKind::Counters => aggregation_counters(report, code),
        AggregatorPreviewKind::Regions => aggregation_regions(report, code),
        AggregatorPreviewKind::Groups => aggregation_groups(report, code),
        AggregatorPreviewKind::Yaml => aggregation_yaml_preview(report, code),
        AggregatorPreviewKind::Templates => aggregation_templates(
            projection.aggregation_templates_available,
            &projection.aggregation_templates,
            code,
        ),
    }
}

pub(crate) fn replay_aggregation_copy(
    locale: Res<UiLocale>,
    last: Option<Res<LastProfilesProjection>>,
    composer: Option<Res<AggregatorComposerState>>,
    mut lines: Query<(&mut Text, &AggregatorPreviewText), Without<AggregatorCustomGroupsText>>,
    mut custom: Query<
        &mut Text,
        (
            With<AggregatorCustomGroupsText>,
            Without<AggregatorPreviewText>,
        ),
    >,
) {
    if !locale.is_changed()
        && !last.as_ref().is_some_and(|value| value.is_changed())
        && !composer.as_ref().is_some_and(|value| value.is_changed())
    {
        return;
    }
    if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
        for (mut text, marker) in &mut lines {
            let value = preview_line(marker.0, projection, locale.code());
            if text.0 != value {
                text.0 = value
            }
        }
    }
    if let Some(composer) = composer {
        let value = aggregation_custom_groups(&composer.custom_groups, locale.code());
        for mut text in &mut custom {
            if text.0 != value {
                text.0 = value.clone()
            }
        }
    }
}
