//! Visual logical sub-rule builder card for the Bevy Rules page (DUAL-11-02).
//!
//! The card holds one [`LogicalDraft`] in [`RulesSubRuleState`] and every
//! mutation, the canonical preview and the validation gate come from the shared
//! `infiltrator_domain::rules::logical` reduction — the same one the Iced panel
//! consumes. Inserting submits the ordinary `AddCustomRule` intent, so the
//! application builds and persists the composed rule through
//! `infiltrator_domain::rules::edit` exactly like the wizard does.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_domain::rules::{edit, logical};

use crate::command::{CommandSinkHandle, UiCommand};

/// Marker for the sub-rule builder card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesSubRuleRoot;

/// One selectable logical operator; payload indexes
/// [`logical::LOGICAL_OPERATOR_CHOICES`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleOperatorChip(pub usize);

/// Caption echoing the active operator.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleOperatorSelection;

/// The condition list body; rebuilt when the draft's conditions change.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleConditionList;

/// One rendered condition row; payload is the draft condition index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleConditionRow(pub usize);

/// Remove control of a rendered condition row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleRemoveConditionButton(pub usize);

/// One-click condition preset; payload indexes
/// [`logical::SUB_RULE_CONDITION_PRESETS`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRulePresetButton(pub usize);

/// Wrapper of the composition target text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleTargetField;

/// The insert-into-rules button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleInsertButton;

/// Preview line showing the canonical expression the draft encodes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRulePreviewLine;

/// Validation status line of the current draft.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubRuleIssueLine;

/// DUAL-11-02: the shared logical draft and nothing else.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct RulesSubRuleState {
    pub draft: LogicalDraft,
}

impl Default for RulesSubRuleState {
    fn default() -> Self {
        Self {
            draft: logical::default_logical_draft(edit::DEFAULT_RULE_TARGET),
        }
    }
}

/// The visual logical sub-rule builder card. The mounted structure reflects the
/// draft it is built with; the interaction observer restamps and, when the
/// condition rows change, rebuilds the list body from the same reduction.
pub fn rules_subrules_scene(palette: &UiPalette, state: &RulesSubRuleState) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = logical::LOGICAL_OPERATOR_CHOICES
        .iter()
        .enumerate()
        .map(|(index, operator)| {
            Box::new(operator_chip(
                index,
                (*operator).to_owned(),
                (*operator) == state.draft.operator,
                palette,
            )) as Box<dyn Scene>
        })
        .collect();
    let preset_buttons: Vec<Box<dyn Scene>> = logical::SUB_RULE_CONDITION_PRESETS
        .iter()
        .enumerate()
        .map(|(index, preset)| {
            Box::new(preset_button(index, (*preset).to_owned(), palette)) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                    column_gap: Val::Px(space::S8),
                }
                RulesSubRuleRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Zap, 24.0, palette) } ),
                            ( Text({ "逻辑子规则构建器 (Sub-Rules)".to_owned() }) TextRole(Role::BodyStrong) ),
                            ( Text({
                                format!("已选逻辑: {}", state.draft.operator)
                            }) SubRuleOperatorSelection TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [ { chips } ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                }
                SubRuleConditionList
                Children [
                    ( { condition_rows_scene(&state.draft, palette) } ),
                ]
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
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [ { preset_buttons } ]
                    ),
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                        }
                        SubRuleTargetField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                state.draft.target.clone(),
                                "出站目标 e.g. PROXY".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text(preview_label(&state.draft)) SubRulePreviewLine TextRole(Role::Caption) ),
                            ( Text(issue_label(&state.draft)) SubRuleIssueLine TextRole(Role::Caption) ),
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
                        SubRuleInsertButton
                        Children [
                            ( Text({ "插入到分流规则".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

/// DUAL-11-02: the canonical expression preview the shared builder encodes.
pub fn preview_label(draft: &LogicalDraft) -> String {
    format!("预览: {}", logical::draft_expression(draft))
}

/// DUAL-11-02: the honest validation status of the current draft.
pub fn issue_label(draft: &LogicalDraft) -> String {
    match logical::draft_issue(draft) {
        Some(issue) => format!("校验未通过: {issue}"),
        None => "校验通过 · 可插入".to_owned(),
    }
}

fn condition_rows_scene(draft: &LogicalDraft, palette: &UiPalette) -> impl Scene + use<> {
    let rows: Vec<Box<dyn Scene>> = if draft.conditions.is_empty() {
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::all(Val::Px(space::S6)),
            }
            Children [
                ( Text({ "尚未添加子条件".to_owned() }) TextRole(Role::Caption) ),
            ]
        }) as Box<dyn Scene>]
    } else {
        draft
            .conditions
            .iter()
            .enumerate()
            .map(|(index, condition)| {
                Box::new(condition_row(index, condition.clone(), palette)) as Box<dyn Scene>
            })
            .collect()
    };

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [ { rows } ]
    }
}

fn condition_row(index: usize, condition: String, palette: &UiPalette) -> impl Scene + use<> {
    let label = format!("#{} {condition}", index + 1);
    bsn! {
        Node {
            width: percent(100),
            padding: UiRect::all(Val::Px(space::S6)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ palette.surface_elevated })
        SubRuleConditionRow(index)
        Children [
            ( Text(label) TextRole(Role::Caption) ),
            (
                Node {
                    min_height: px(22.0),
                    padding: UiRect::horizontal(Val::Px(space::S6)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                SubRuleRemoveConditionButton(index)
                Children [
                    ( Text({ "移除".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    }
}

fn operator_chip(
    index: usize,
    operator: String,
    selected: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let fill = if selected {
        palette.accent_container
    } else {
        palette.surface_elevated
    };
    bsn! {
        Node {
            min_height: px(26.0),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ fill })
        Button
        SubRuleOperatorChip(index)
        Children [
            ( Text(operator) TextRole(Role::Caption) ),
        ]
    }
}

fn preset_button(index: usize, preset: String, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            min_height: px(24.0),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ palette.surface_elevated })
        Button
        SubRulePresetButton(index)
        Children [
            ( Text({
                format!("+ {preset}")
            }) TextRole(Role::Caption) ),
        ]
    }
}

/// Rebuild the condition list body from the shared draft.
fn rebuild_condition_rows(
    commands: &mut Commands<'_, '_>,
    list: bevy::ecs::entity::Entity,
    draft: &LogicalDraft,
    palette: &UiPalette,
) {
    commands.entity(list).despawn_children();
    commands
        .spawn_scene(condition_rows_scene(draft, palette))
        .insert(ChildOf(list));
}

/// DUAL-11-02: apply a chip/preset/remove/insert action to the shared draft.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn on_rules_subrules_activated(
    activate: On<Activate>,
    chips: Query<&SubRuleOperatorChip>,
    presets: Query<&SubRulePresetButton>,
    removers: Query<&SubRuleRemoveConditionButton>,
    insert_buttons: Query<(), With<SubRuleInsertButton>>,
    target_wrappers: Query<&Children, With<SubRuleTargetField>>,
    text_fields: Query<&TextField>,
    lists: Query<bevy::ecs::entity::Entity, With<SubRuleConditionList>>,
    mut state: Option<ResMut<RulesSubRuleState>>,
    palette: Res<UiPalette>,
    mut commands: Commands,
    mut previews: Query<
        &mut Text,
        (
            With<SubRulePreviewLine>,
            Without<SubRuleIssueLine>,
            Without<SubRuleOperatorSelection>,
        ),
    >,
    mut issues: Query<
        &mut Text,
        (
            With<SubRuleIssueLine>,
            Without<SubRulePreviewLine>,
            Without<SubRuleOperatorSelection>,
        ),
    >,
    mut selections: Query<
        &mut Text,
        (
            With<SubRuleOperatorSelection>,
            Without<SubRulePreviewLine>,
            Without<SubRuleIssueLine>,
        ),
    >,
    mut chip_fills: Query<(&mut BackgroundColor, &SubRuleOperatorChip)>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };

    if let Ok(chip) = chips.get(activate.entity) {
        let Some(operator) = logical::LOGICAL_OPERATOR_CHOICES.get(chip.0) else {
            return;
        };
        if logical::select_operator(&mut state.draft, operator) {
            restamp_subrules(
                state,
                &palette,
                &mut previews,
                &mut issues,
                &mut selections,
                &mut chip_fills,
            );
        }
        return;
    }

    let mut conditions_changed = false;
    if let Ok(preset) = presets.get(activate.entity) {
        if let Some(condition) = logical::SUB_RULE_CONDITION_PRESETS.get(preset.0) {
            conditions_changed = logical::add_condition(&mut state.draft, condition);
        }
    } else if let Ok(remover) = removers.get(activate.entity) {
        conditions_changed = logical::remove_condition(&mut state.draft, remover.0);
    }
    if conditions_changed {
        for list in lists.iter() {
            rebuild_condition_rows(&mut commands, list, &state.draft, &palette);
        }
        restamp_subrules(
            state,
            &palette,
            &mut previews,
            &mut issues,
            &mut selections,
            &mut chip_fills,
        );
        return;
    }

    if !insert_buttons.contains(activate.entity) {
        return;
    }
    // The target field is the live source of the outbound target.
    if let Some(target) = crate::pages::rules_builder::field_text(&target_wrappers, &text_fields) {
        logical::set_target(&mut state.draft, &target);
    }
    restamp_subrules(
        state,
        &palette,
        &mut previews,
        &mut issues,
        &mut selections,
        &mut chip_fills,
    );
    let Some(handle) = handle else {
        return;
    };
    // The shared builder gates the submit: an invalid draft is never forwarded,
    // and its issue stays visible on the status line.
    if logical::build_logical_rule(&state.draft).is_err() {
        return;
    }
    handle.submit(UiCommand::AddCustomRule {
        rule_type: logical::draft_operator(&state.draft),
        payload: logical::draft_payload(&state.draft.conditions),
        target: state.draft.target.trim().to_owned(),
    });
}

/// Restamp every text/fill the card owns from the shared draft.
#[allow(clippy::type_complexity)]
fn restamp_subrules(
    state: &RulesSubRuleState,
    palette: &UiPalette,
    previews: &mut Query<
        &mut Text,
        (
            With<SubRulePreviewLine>,
            Without<SubRuleIssueLine>,
            Without<SubRuleOperatorSelection>,
        ),
    >,
    issues: &mut Query<
        &mut Text,
        (
            With<SubRuleIssueLine>,
            Without<SubRulePreviewLine>,
            Without<SubRuleOperatorSelection>,
        ),
    >,
    selections: &mut Query<
        &mut Text,
        (
            With<SubRuleOperatorSelection>,
            Without<SubRulePreviewLine>,
            Without<SubRuleIssueLine>,
        ),
    >,
    chip_fills: &mut Query<(&mut BackgroundColor, &SubRuleOperatorChip)>,
) {
    let want_preview = preview_label(&state.draft);
    for mut text in previews.iter_mut() {
        if text.0 != want_preview {
            text.0 = want_preview.clone();
        }
    }
    let want_issue = issue_label(&state.draft);
    for mut text in issues.iter_mut() {
        if text.0 != want_issue {
            text.0 = want_issue.clone();
        }
    }
    let want_selection = format!("已选逻辑: {}", state.draft.operator);
    for mut text in selections.iter_mut() {
        if text.0 != want_selection {
            text.0 = want_selection.clone();
        }
    }
    for (mut fill, chip) in chip_fills.iter_mut() {
        let selected = logical::LOGICAL_OPERATOR_CHOICES
            .get(chip.0)
            .is_some_and(|candidate| *candidate == state.draft.operator);
        let want = if selected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        fill.0 = want;
    }
}
