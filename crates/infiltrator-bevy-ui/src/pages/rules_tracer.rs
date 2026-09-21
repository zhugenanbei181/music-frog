//! Rules Tracer sandbox view for simulating routing and sub-rule decision chains (分流追踪器沙盒).

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
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
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::rules::RulesProjectionUpdated;

/// Marker on the Rules Tracer card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTracerRoot;

/// Marker on the simulate trace button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SimulateRuleTraceButton;

/// Marker for preset chips.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct TracerPresetChip(pub String);

/// Marker for trace result decision tree.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerDecisionTree;

/// DUAL-12-10: marker on the wrapper of the sandbox target-query text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerQueryField;

/// DUAL-12-10: marker on the wrapper of the simulated source-IP text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerSourceIpField;

/// Text slots of the tracer card, patched in place from the shared snapshot.
/// Slot `0` is the headline, `1..=5` the five decision-chain stages (fixed
/// capacity), and `6` the honest empty/unsupported line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerText(pub u8);

const TRACER_HEADLINE_SLOT: u8 = 0;
const TRACER_STAGE_SLOTS: u8 = 5;
const TRACER_EMPTY_SLOT: u8 = 6;

/// Headline line for the current shared snapshot.
fn tracer_headline(tracer: &RuleTracerSnapshot) -> String {
    match &tracer.decision_chain {
        Some(chain) if !chain.is_fallback => format!(
            "【匹配命中】规则 #{}: {} -> {}",
            chain.hit_rule_index.map(|i| i + 1).unwrap_or(0),
            chain.matched_rule_raw,
            chain.target_proxy
        ),
        Some(chain) => format!(
            "【默认兜底】{} -> {}",
            chain.matched_rule_raw, chain.target_proxy
        ),
        None => "分流追踪器沙盒 (等待查询)".to_owned(),
    }
}

/// One decision-chain stage line; empty for slots without a live chain.
fn tracer_stage_line(tracer: &RuleTracerSnapshot, stage: usize) -> String {
    match &tracer.decision_chain {
        Some(chain) => chain
            .nodes
            .get(stage)
            .map(|node| {
                format!(
                    "· {} | {} | {}",
                    node.stage.as_str(),
                    node.title,
                    node.detail
                )
            })
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// Honest empty/unsupported line — never a fabricated replay.
fn tracer_empty_line(tracer: &RuleTracerSnapshot) -> String {
    match &tracer.failure {
        Some(failure) => format!("分流追踪器不可用: {failure}"),
        None => "输入测试目标后执行模拟追踪，决策链路将在此回放".to_owned(),
    }
}

/// Patch the tracer card text slots from the shared projection so the card
/// stays reactive across projection updates without re-mounting the page.
pub(crate) fn apply_tracer_projection(
    update: On<RulesProjectionUpdated>,
    mut texts: Query<(&mut Text, &TracerText)>,
) {
    let tracer = &update.0.tracer;
    for (mut text, slot) in texts.iter_mut() {
        text.0 = match slot.0 {
            TRACER_HEADLINE_SLOT => tracer_headline(tracer),
            stage @ 1..=TRACER_STAGE_SLOTS => tracer_stage_line(tracer, (stage - 1) as usize),
            _ => tracer_empty_line(tracer),
        };
    }
}

/// DUAL-12-10: the simulate button reads both sandbox fields and submits the
/// shared context + query commands; no UI-local trace is fabricated.
pub(crate) fn on_tracer_action_activated(
    activate: On<Activate>,
    simulate_buttons: Query<(), With<SimulateRuleTraceButton>>,
    query_fields: Query<&Children, With<TracerQueryField>>,
    source_fields: Query<&Children, With<TracerSourceIpField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if simulate_buttons.get(activate.entity).is_err() {
        return;
    }
    let query = query_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    let src_ip = source_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    let src_ip = (!src_ip.trim().is_empty()).then(|| src_ip.trim().to_owned());
    handle.submit(UiCommand::SetRuleTracerContext { src_ip });
    if !query.trim().is_empty() {
        handle.submit(UiCommand::SimulateRuleTrace {
            query: query.trim().to_owned(),
        });
    }
}

/// Scene constructor for the Live Rule Tracer card. Data-driven from the
/// shared `RuleTracerSnapshot` the surface reader projects; no fabricated
/// replay content is rendered when the snapshot has no decision chain.
pub fn rules_tracer_scene(palette: &UiPalette, tracer: &RuleTracerSnapshot) -> impl Scene + use<> {
    let preset_labels: Vec<String> = if tracer.presets.is_empty() {
        RuleTracerSnapshot::default_presets()
            .into_iter()
            .map(|preset| preset.label)
            .collect()
    } else {
        tracer
            .presets
            .iter()
            .map(|preset| preset.label.clone())
            .collect()
    };

    let preset_chips: Vec<Box<dyn Scene>> = preset_labels
        .into_iter()
        .map(|label| {
            Box::new(bsn! {
                Node {
                    min_height: px(28.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.border })
                Button
                TracerPresetChip({ label.clone() })
                Children [
                    ( Text({ label.clone() }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    // DUAL-12-10: the sandbox environment inputs. The query seeds from the
    // shared snapshot; the source IP seeds from the shared simulated context.
    let query_initial = tracer.active_query.clone();
    let src_ip_initial = tracer.simulated_context.src_ip.clone().unwrap_or_default();

    // The tracer card keeps a fixed set of text slots (headline + 5 stages +
    // honest empty line) so the in-place projection patch can rewrite them
    // without re-mounting the page.
    let tracer_slot = |slot: u8, content: String, role: Role| -> Box<dyn Scene> {
        Box::new(bsn! {
            Node {
                width: percent(100),
            }
            Children [
                ( Text({ content }) TracerText({ slot }) TextRole(role) ),
            ]
        })
    };

    let mut decision_rows: Vec<Box<dyn Scene>> = vec![tracer_slot(
        TRACER_HEADLINE_SLOT,
        tracer_headline(tracer),
        Role::BodyStrong,
    )];
    for stage in 0..TRACER_STAGE_SLOTS {
        decision_rows.push(tracer_slot(
            stage + 1,
            tracer_stage_line(tracer, stage as usize),
            Role::Caption,
        ));
    }
    decision_rows.push(tracer_slot(
        TRACER_EMPTY_SLOT,
        tracer_empty_line(tracer),
        Role::Caption,
    ));

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                RulesTracerRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "实时分流追踪器沙盒 (Live Rule Tracer)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                        SimulateRuleTraceButton
                        Children [
                            ( Text({ "执行模拟追踪".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S6)),
                }
                Children [
                    (
                        Node { flex_grow: 1.0 }
                        TracerQueryField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                query_initial,
                                "目标域名或 IP (例如: google.com 或 1.1.1.1:443)".to_owned(),
                                palette,
                            ) } ),
                        ]
                    ),
                    (
                        Node { width: px(240.0) }
                        TracerSourceIpField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                src_ip_initial,
                                "模拟来源 IP (例如: 192.168.1.100)".to_owned(),
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
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S6)),
                }
                Children [
                    { preset_chips },
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                TracerDecisionTree
                Children [
                    { decision_rows },
                ]
            }),
        ],
        palette,
    )
}
