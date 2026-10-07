//! Visual logical sub-rule builder card for the Bevy Rules page (DUAL-11-02).
//!
//! The card holds one [`LogicalDraft`] in [`RulesSubRuleState`] and every
//! mutation, the canonical preview and the validation gate come from the shared
//! `infiltrator_domain::rules::logical` reduction — the same one the Iced panel
//! consumes. Inserting submits the ordinary `AddCustomRule` intent, so the
//! application builds and persists the composed rule through
//! `infiltrator_domain::rules::edit` exactly like the wizard does.

use crate::localized_widgets::localized_field_scene;
use crate::pages::rules_builder::field_text;
use crate::pages::rules_draft::{RuleDraftMutationButton, RulesDraftState};
use crate::pages::rules_statistics::RulesStatisticsState;
use bevy::ecs::component::Component;
use bevy::ecs::entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::logical_rule_projection::project_logical_rule;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_domain::rules::{edit, logical};

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
    let display = project_logical_rule(&state.draft, "en-US");
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Zap, 24.0, palette) }
                                    --
                                    LocalizedText::plain("rules_subrule_builder_title") TextRole(Role::BodyStrong)
                                    --
                                    Text({
                                            display.selection.clone()
                                    }) SubRuleOperatorSelection TextRole(Role::Caption)
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    { chips }
                                ]
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
                                @{ condition_rows_scene(&state.draft, palette) }
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
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    { preset_buttons }
                                ]
                                --
                                Node {
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                SubRuleTargetField
                                Children [
                                    @{ localized_field_scene(state.draft.target.clone(), LocalizedText::plain("field_subrule_target"),
                                            palette,
                                    ) }
                                ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    Text({display.preview.clone()}) SubRulePreviewLine TextRole(Role::Caption)
                                    --
                                    Text({display.issue.clone()}) SubRuleIssueLine TextRole(Role::Caption)
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
                                SubRuleInsertButton RuleDraftMutationButton ButtonDisabled(true)
                                Children [
                                    LocalizedText::plain("rules_insert_routing_action") TextRole(Role::BodyStrong)
                                ]
                            ]
            }),
        ],
        palette,
    )
}

fn condition_rows_scene(draft: &LogicalDraft, palette: &UiPalette) -> impl Scene + use<> {
    let rows: Vec<Box<dyn Scene>> = if draft.conditions.is_empty() {
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        padding: UiRect::all(Val::Px(space::S6)),
                    }
                    Children [
                        LocalizedText::plain("subrules_no_conditions") TextRole(Role::Caption)
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
            Children [
                { rows }
            ]
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
                Text(label) TextRole(Role::Caption)
                --
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
                    LocalizedText::plain("aggregator_custom_remove") TextRole(Role::Caption)
                ]
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
                Text(operator) TextRole(Role::Caption)
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
                Text({
                        format!("+ {preset}")
                }) TextRole(Role::Caption)
            ]
    }
}

/// Rebuild the condition list body from the shared draft.
fn rebuild_condition_rows(
    commands: &mut Commands<'_, '_>,
    list: entity::Entity,
    draft: &LogicalDraft,
    palette: &UiPalette,
) {
    commands.entity(list).despawn_children();
    commands
        .spawn_scene(condition_rows_scene(draft, palette))
        .insert(ChildOf(list));
}

/// All access belongs to this single logical draft and its bounded condition list.
#[derive(SystemParam)]
pub struct SubRuleInteraction<'w, 's> {
    chips: Query<'w, 's, &'static SubRuleOperatorChip>,
    presets: Query<'w, 's, &'static SubRulePresetButton>,
    removers: Query<'w, 's, &'static SubRuleRemoveConditionButton>,
    insert_buttons: Query<'w, 's, (), With<SubRuleInsertButton>>,
    target_wrappers: Query<'w, 's, &'static Children, With<SubRuleTargetField>>,
    text_fields: Query<'w, 's, &'static TextField>,
    lists: Query<'w, 's, entity::Entity, With<SubRuleConditionList>>,
    state: ResMut<'w, RulesSubRuleState>,
    palette: Res<'w, UiPalette>,
    editor: ResMut<'w, RulesDraftState>,
    statistics: Option<Res<'w, RulesStatisticsState>>,
}
pub(crate) fn on_rules_subrules_activated(
    activate: On<Activate>,
    mut interaction: SubRuleInteraction,
    mut commands: Commands,
) {
    if interaction
        .statistics
        .as_ref()
        .is_some_and(|state| state.model.confirmation.is_some())
    {
        return;
    }
    let state = &mut interaction.state;
    if let Ok(chip) = interaction.chips.get(activate.entity) {
        if let Some(operator) = logical::LOGICAL_OPERATOR_CHOICES.get(chip.0) {
            logical::select_operator(&mut state.draft, operator);
        }
        return;
    }
    let changed = if let Ok(preset) = interaction.presets.get(activate.entity) {
        logical::SUB_RULE_CONDITION_PRESETS
            .get(preset.0)
            .is_some_and(|preset| logical::add_condition(&mut state.draft, preset))
    } else if let Ok(remover) = interaction.removers.get(activate.entity) {
        logical::remove_condition(&mut state.draft, remover.0)
    } else {
        false
    };
    if changed {
        for list in &interaction.lists {
            rebuild_condition_rows(&mut commands, list, &state.draft, &interaction.palette);
        }
        return;
    }
    if !interaction.insert_buttons.contains(activate.entity) {
        return;
    }
    if let Some(target) = field_text(&interaction.target_wrappers, &interaction.text_fields) {
        logical::set_target(&mut state.draft, &target);
    }
    if let Ok(entry) = logical::build_logical_rule(&state.draft) {
        interaction.editor.repaint |= interaction.editor.model.prepend([entry]);
    }
}
