//! Custom Rule Builder and Game Preset Injector scene for Bevy UI (自定义规则向导与游戏预设).
//!
//! DUAL-11-11/12: the wizard reads its type selection from
//! [`RulesBuilderState`] and its payload/target from mounted text fields, then
//! submits the same shared intent the Iced wizard does (which the application
//! applies through `infiltrator_domain::rules::edit`).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, percent, px,
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
use infiltrator_domain::rules::edit;

use crate::command::{CommandSinkHandle, UiCommand};

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

/// DUAL-11-11: the selected wizard rule type, shared across chip selection.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct RulesBuilderState {
    pub rule_type: String,
}

impl Default for RulesBuilderState {
    fn default() -> Self {
        Self {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
        }
    }
}

/// Custom Rule Builder & Game Presets scene.
pub fn rules_builder_scene(palette: &UiPalette) -> impl Scene + use<> {
    let type_chips: Vec<Box<dyn Scene>> = edit::CUSTOM_RULE_TYPE_CHOICES
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            Box::new(rule_type_chip(index, (*choice).to_owned(), palette)) as Box<dyn Scene>
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
                }
                RulesBuilderRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Zap, 24.0, palette) } ),
                            ( Text({ "添加自定义规则向导 (Add Custom Rule)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    (
                        Node {
                            column_gap: Val::Px(space::S8),
                            align_items: AlignItems::Center,
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
                                InjectGamePresetsButton
                                Children [
                                    ( Text({ "一键注入游戏分流预设".to_owned() }) TextRole(Role::BodyStrong) ),
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
                                AddCustomRuleButton
                                Children [
                                    ( Text({ "+ 确认添加规则".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
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
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [ { type_chips } ]
                    ),
                    ( Text({ "已选类型: DOMAIN-SUFFIX" }) RuleBuilderSelection TextRole(Role::Caption) ),
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
                    (
                        Node {
                            flex_grow: 2.0,
                            min_width: px(0.0),
                        }
                        RulePayloadField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                String::new(),
                                "匹配内容 e.g. github.com".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                        }
                        RuleTargetField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                edit::DEFAULT_RULE_TARGET.to_owned(),
                                "出站目标".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

fn rule_type_chip(index: usize, rule_type: String, palette: &UiPalette) -> impl Scene + use<> {
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
        Children [
            ( Text(rule_type) TextRole(Role::Caption) ),
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

/// DUAL-11-11/12: select a rule type or submit the wizard/preset intent.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(crate) fn on_rules_builder_activated(
    activate: On<Activate>,
    chips: Query<&RuleTypeChip>,
    add_buttons: Query<(), With<AddCustomRuleButton>>,
    preset_buttons: Query<(), With<InjectGamePresetsButton>>,
    payload_wrappers: Query<&Children, With<RulePayloadField>>,
    target_wrappers: Query<&Children, With<RuleTargetField>>,
    text_fields: Query<&TextField>,
    mut builder: Option<ResMut<RulesBuilderState>>,
    palette: Res<UiPalette>,
    mut chip_fills: Query<(&mut BackgroundColor, &RuleTypeChip)>,
    mut selection: Query<&mut Text, With<RuleBuilderSelection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if let Ok(chip) = chips.get(activate.entity) {
        let Some(rule_type) = edit::CUSTOM_RULE_TYPE_CHOICES.get(chip.0) else {
            return;
        };
        let rule_type = (*rule_type).to_owned();
        if let Some(builder) = builder.as_deref_mut() {
            builder.rule_type = rule_type.clone();
        }
        restamp_type_chips(&palette, &mut chip_fills, &rule_type);
        for mut text in &mut selection {
            text.0 = format!("已选类型: {rule_type}");
        }
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    if add_buttons.contains(activate.entity) {
        let rule_type = builder
            .as_deref()
            .map(|builder| builder.rule_type.clone())
            .unwrap_or_else(|| "DOMAIN-SUFFIX".to_owned());
        let payload = field_text(&payload_wrappers, &text_fields).unwrap_or_default();
        let target = field_text(&target_wrappers, &text_fields).unwrap_or_default();
        handle.submit(UiCommand::AddCustomRule {
            rule_type,
            payload,
            target,
        });
    } else if preset_buttons.contains(activate.entity) {
        let target = field_text(&target_wrappers, &text_fields).unwrap_or_default();
        handle.submit(UiCommand::ApplyGameRoutingPresets { target });
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
