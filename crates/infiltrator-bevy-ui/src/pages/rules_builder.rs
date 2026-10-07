//! Custom Rule Builder and Game Preset Injector scene for Bevy UI (自定义规则向导与游戏预设).
//!
//! DUAL-11-11/12: the wizard reads its type selection from
//! [`RulesBuilderState`] and its payload/target from mounted text fields, then
//! submits the same shared intent the Iced wizard does (which the application
//! applies through `infiltrator_domain::rules::edit`).

use crate::a11y::button_semantic_node;
use crate::localized_widgets::{localized_button_scene, localized_field_scene};
use crate::pages::rules_builder_input::{RuleBuilderField, RuleBuilderFieldKind};
use crate::pages::rules_draft::{RuleDraftMutationButton, RulesDraftState};
use crate::pages::rules_statistics::RulesStatisticsState;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexWrap, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::rule_form_binding::{RuleFormBinding, default_form_target};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rule_edit::RuleDraft;
use infiltrator_domain::rules::edit;

/// Marker for custom rule builder card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesBuilderRoot;

/// Marker for add custom rule action button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddCustomRuleButton;

/// Marker for inject game presets button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InjectGamePresetsButton;

/// DUAL-11-11: one selectable rule-type chip; payload is the index into
/// [`edit::CUSTOM_RULE_TYPE_CHOICES`].
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTypeChip(pub usize);

/// Marker on the wrapper of the wizard's match-payload text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulePayloadField;

/// Marker on the wrapper of the wizard's outbound-target text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTargetField;

/// Marker on the caption echoing the currently selected rule type.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleBuilderSelection;
#[derive(Component, Clone, Default)]
pub struct RuleFormStatus;
#[derive(Component, Clone, Copy, Default)]
pub struct DiscardRuleFormButton;
#[derive(Component, Clone, Copy, Default)]
pub struct RuleFormMutationButton;

/// DUAL-11-11: the selected wizard rule type, shared across chip selection.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct RulesBuilderState {
    pub binding: RuleFormBinding,
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub initialized: bool,
}

impl Default for RulesBuilderState {
    fn default() -> Self {
        Self {
            binding: RuleFormBinding::default(),
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: String::new(),
            target: edit::DEFAULT_RULE_TARGET.to_owned(),
            initialized: false,
        }
    }
}

/// Custom Rule Builder & Game Presets scene.
pub fn rules_builder_scene(palette: &UiPalette) -> impl Scene + use<> {
    let add_node = button_semantic_node("");
    let preset_node = button_semantic_node("");
    let type_chips: Vec<Box<dyn Scene>> = edit::CUSTOM_RULE_TYPE_CHOICES
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            Box::new(rule_type_chip(index, (*choice).to_owned(), palette)) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! { Text(String::new()) RuleFormStatus TextRole(Role::Caption) }),
            Box::new((
                localized_button_scene(
                    LocalizedText::plain("rules_form_discard"),
                    ButtonVariant::Default,
                    palette,
                ),
                bsn! { DiscardRuleFormButton ButtonDisabled(false) },
            )),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                min_width: px(0.0),
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: px(space::S8),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            RulesBuilderRoot
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Zap, 24.0, palette) }
                                    --
                                    LocalizedText::plain("rules_add_custom_title") TextRole(Role::BodyStrong)
                                ]
                                --
                                Node {
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: px(space::S8),
                                    column_gap: Val::Px(space::S8),
                                    align_items: AlignItems::Center,
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
                                    InjectGamePresetsButton RuleDraftMutationButton RuleFormMutationButton ButtonDisabled(true)
                                    preset_node LocalizedLabel::plain("rules_inject_game_presets")
                                    Children [
                                        LocalizedText::plain("rules_inject_game_presets") TextRole(Role::BodyStrong)
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
                                    AddCustomRuleButton RuleDraftMutationButton RuleFormMutationButton ButtonDisabled(true)
                                    add_node LocalizedLabel::plain("rules_add_confirm_action")
                                    Children [
                                        LocalizedText::plain("rules_add_confirm_action") TextRole(Role::BodyStrong)
                                    ]
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                min_width: px(0.0),
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: px(space::S8),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::vertical(Val::Px(space::S4)),
                            }
                            Children [
                                Node {
                                    flex_grow: 1.0,
                                    flex_basis: px(280.0),
                                    min_width: px(0.0),
                                    max_width: percent(100),
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: px(space::S6),
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    { type_chips }
                                ]
                                --
                                LocalizedText::new("rules_builder_selection", vec![("type", "DOMAIN-SUFFIX".to_owned())]) RuleBuilderSelection TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                                padding: UiRect::vertical(Val::Px(space::S4)),
                            }
                            Children [
                                Node {
                                    flex_grow: 2.0,
                                    min_width: px(0.0),
                                }
                                RulePayloadField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_rule_match"),
                                            palette,
                                    ) }
                                    RuleBuilderField { kind: RuleBuilderFieldKind::Payload } NativeTextField(40)
                                ]
                                --
                                Node {
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                RuleTargetField
                                Children [
                                    @{ localized_field_scene(edit::DEFAULT_RULE_TARGET.to_owned(), LocalizedText::plain("field_rule_target"),
                                            palette,
                                    ) }
                                    RuleBuilderField { kind: RuleBuilderFieldKind::Target } NativeTextField(41)
                                ]
                            ]
            }),
        ],
        palette,
    )
}

fn rule_type_chip(index: usize, rule_type: String, palette: &UiPalette) -> impl Scene + use<> {
    let semantic = button_semantic_node(&rule_type);
    bsn! {
            Node {
                min_height: px(28.0),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
            }
            BackgroundColor({ palette.surface_elevated })
            Button
            RuleTypeChip(index)
            RuleDraftMutationButton RuleFormMutationButton ButtonDisabled(true)
            semantic
            Children [
                Text(rule_type) TextRole(Role::Caption)
            ]
    }
}

/// Read the live text of the first field under a marked wrapper.
pub(crate) fn field_text<W: Component, F: QueryFilter>(
    wrappers: &Query<&Children, With<W>>,
    fields: &Query<&TextField, F>,
) -> Option<String> {
    wrappers
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

/// Only the mounted form's controls, drafts and presentation resources.
#[derive(SystemParam)]
pub struct RuleBuilderInteraction<'w, 's> {
    chips: Query<'w, 's, &'static RuleTypeChip>,
    add_buttons: Query<'w, 's, (), With<AddCustomRuleButton>>,
    preset_buttons: Query<'w, 's, (), With<InjectGamePresetsButton>>,
    discard_buttons: Query<'w, 's, (), With<DiscardRuleFormButton>>,
    payload_wrappers: Query<'w, 's, &'static Children, With<RulePayloadField>>,
    target_wrappers: Query<'w, 's, &'static Children, With<RuleTargetField>>,
    text_fields: Query<'w, 's, &'static TextField>,
    builder: Option<ResMut<'w, RulesBuilderState>>,
    palette: Res<'w, UiPalette>,
    chip_fills: Query<'w, 's, (&'static mut BackgroundColor, &'static RuleTypeChip)>,
    selection: Query<'w, 's, &'static mut LocalizedText, With<RuleBuilderSelection>>,
    draft: ResMut<'w, RulesDraftState>,
}

pub(crate) fn on_rules_builder_activated(
    activate: On<Activate>,
    interaction: RuleBuilderInteraction,
    statistics: Option<Res<RulesStatisticsState>>,
) {
    if statistics.is_some_and(|state| state.model.confirmation.is_some()) {
        return;
    }
    let RuleBuilderInteraction {
        chips,
        add_buttons,
        preset_buttons,
        discard_buttons,
        payload_wrappers,
        target_wrappers,
        text_fields,
        mut builder,
        palette,
        mut chip_fills,
        mut selection,
        mut draft,
    } = interaction;
    if !chips.contains(activate.entity)
        && !add_buttons.contains(activate.entity)
        && !preset_buttons.contains(activate.entity)
        && !discard_buttons.contains(activate.entity)
    {
        return;
    }
    if discard_buttons.contains(activate.entity) {
        if draft.model.pending.is_none()
            && !draft.model.awaiting_read
            && let Some(builder) = builder.as_deref_mut()
        {
            *builder = RulesBuilderState::default();
            builder.target = default_form_target(&draft.model);
            builder.binding.reset(&draft.model);
        }
        return;
    }
    if !draft.model.editable() || draft.input_composing {
        return;
    }
    let Some(builder) = builder.as_deref_mut() else {
        return;
    };
    builder.binding.observe(&draft.model);
    if let Err(failure) = builder.binding.require_current(&draft.model) {
        draft.model.failure = Some(failure);
        return;
    }
    if let Ok(chip) = chips.get(activate.entity) {
        let Some(rule_type) = edit::CUSTOM_RULE_TYPE_CHOICES.get(chip.0) else {
            return;
        };
        let rule_type = (*rule_type).to_owned();
        builder.binding.edit(&draft.model);
        builder.rule_type = rule_type.clone();
        restamp_type_chips(&palette, &mut chip_fills, &rule_type);
        for mut copy in &mut selection {
            copy.params = vec![("type", rule_type.clone())];
        }
        return;
    }
    if add_buttons.contains(activate.entity) {
        let rule_type = builder.rule_type.clone();
        let payload = field_text(&payload_wrappers, &text_fields).unwrap_or_default();
        let target = field_text(&target_wrappers, &text_fields).unwrap_or_default();
        let fields = RuleDraft {
            rule_type,
            payload,
            target,
        };
        match draft.model.add(&fields) {
            Ok(changed) => draft.repaint |= changed,
            Err(failure) => draft.model.failure = Some(failure),
        }
    } else if preset_buttons.contains(activate.entity) {
        let target = field_text(&target_wrappers, &text_fields).unwrap_or_default();
        draft.repaint |= draft.model.game_presets(&target);
    }
}

/// Restamp every rule-type chip fill for the active selection.
pub(crate) fn restamp_type_chips<F: QueryFilter>(
    palette: &UiPalette,
    chips: &mut Query<(&mut BackgroundColor, &RuleTypeChip), F>,
    active: &str,
) {
    // The chip carries an index; resolve it back to the shared type vocabulary.
    for (mut fill, chip) in chips.iter_mut() {
        let selected = edit::CUSTOM_RULE_TYPE_CHOICES
            .get(chip.0)
            .is_some_and(|candidate| *candidate == active);
        fill.0 = if selected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
    }
}
