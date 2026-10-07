//! Rules Tracer sandbox view for simulating routing and sub-rule decision chains (分流追踪器沙盒).

use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::ime::ShellImeComposition;
use crate::localized_widgets::{localized_button_scene, localized_field_scene};
use crate::pages::rules::{LastRulesProjection, RulesProjectionUpdated};
use crate::pages::rules_tabs::RulesTabState;
use crate::pages::rules_tracer_sandbox;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::rule_trace_actions::RuleTraceActions;
use infiltrator_application::rule_trace_projection::{
    stage_text, trace_headline, trace_provenance, trace_status,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_trace_run::RuleTraceOperationId;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;
use infiltrator_contract::rules_workspace::RulesTab;

#[derive(Resource, Default)]
pub struct RulesTraceState {
    pub model: RuleTraceActions,
    pub request: Option<(RequestId, RuleTraceOperationId)>,
    pub override_request: Option<(RequestId, RuleTraceOperationId)>,
    pub drafts_initialized: bool,
    pub override_target: Option<String>,
}

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

/// DUAL-12-08: marker on the wrapper of the reverse-apply target text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerOverrideTargetField;

/// DUAL-12-08: marker on the "apply this rule's outbound" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplyTracerRuleOverrideButton;

#[derive(Component, Clone, Copy, Default)]
pub enum TraceControlKind {
    #[default]
    Simulate,
    Apply,
}

/// Text slots of the tracer card, patched in place from the shared snapshot.
/// Slot `0` is the headline, `1..=5` the five decision-chain stages (fixed
/// capacity), and `6` the honest empty/unsupported line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TracerText(pub u8);

const TRACER_HEADLINE_SLOT: u8 = 0;
const TRACER_STAGE_SLOTS: u8 = 5;
const TRACER_EMPTY_SLOT: u8 = 6;

fn tracer_stage_line(tracer: &RuleTracerSnapshot, stage: usize, code: &str) -> String {
    tracer
        .decision_chain
        .as_ref()
        .and_then(|chain| chain.nodes.get(stage))
        .map(|node| stage_text(node, code))
        .unwrap_or_default()
}

/// Patch the tracer card text slots from the shared projection so the card
/// stays reactive across projection updates without re-mounting the page.
pub(crate) fn apply_tracer_projection(
    update: On<RulesProjectionUpdated>,
    locale: Res<UiLocale>,
    mut texts: Query<(&mut Text, &TracerText)>,
) {
    let tracer = &update.0.tracer;
    for (mut text, slot) in texts.iter_mut() {
        text.0 = match slot.0 {
            TRACER_HEADLINE_SLOT => trace_headline(tracer.decision_chain.as_ref(), locale.code()),
            stage @ 1..=TRACER_STAGE_SLOTS => {
                tracer_stage_line(tracer, (stage - 1) as usize, locale.code())
            }
            _ => LocalizedText::plain("rule_trace_simulation_notice").render(&locale),
        };
    }
}

#[derive(SystemParam)]
pub struct TraceInput<'w, 's> {
    simulate_buttons: Query<'w, 's, (), With<SimulateRuleTraceButton>>,
    presets: Query<'w, 's, &'static TracerPresetChip>,
    query_fields: Query<'w, 's, &'static Children, With<TracerQueryField>>,
    source_fields: Query<'w, 's, &'static Children, With<TracerSourceIpField>>,
    text_fields: Query<'w, 's, &'static mut TextField, With<TracerNativeField>>,
    composition: Option<Res<'w, ShellImeComposition>>,
    route: Res<'w, ActiveRoute>,
    tab: Option<Res<'w, RulesTabState>>,
}
impl TraceInput<'_, '_> {
    fn active(&self) -> bool {
        self.route.0 == Some(Route::Rules)
            && self
                .tab
                .as_ref()
                .is_some_and(|tab| tab.tab == RulesTab::Tracer)
    }
}

/// DUAL-12-10: the simulate button reads both sandbox fields and submits the
/// shared context + query commands; no UI-local trace is fabricated.
pub(crate) fn on_tracer_action_activated(
    activate: On<Activate>,
    mut input: TraceInput,
    mut state: ResMut<RulesTraceState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if !input.active()
        || state.model.confirmation.is_some()
        || input
            .composition
            .as_ref()
            .is_some_and(|composition| composition.is_composing())
    {
        return;
    }
    if let Ok(preset) = input.presets.get(activate.entity) {
        if state.model.pending.is_some() {
            return;
        }
        state.model.set_query(preset.0.clone());
        for child in input
            .query_fields
            .iter()
            .flat_map(|children| children.iter())
        {
            if let Ok(mut field) = input.text_fields.get_mut(*child) {
                field.0.apply(TextFieldInput::SetText(preset.0.clone()));
            }
        }
        return;
    }
    if input.simulate_buttons.get(activate.entity).is_err() {
        return;
    }
    let query = input
        .query_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| input.text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    let src_ip = input
        .source_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| input.text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    if state.model.pending.is_some() {
        return;
    }
    state.model.set_query(query.to_owned());
    state.model.set_source_ip(src_ip.to_owned());
    if let Ok((operation, request)) = state.model.begin() {
        if let Some(id) = handle.and_then(|handle| {
            handle.submit_tracked(UiCommand::SimulateRuleTrace { operation, request })
        }) {
            state.request = Some((id, operation));
        } else {
            state.model.finish(
                operation,
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "Rule simulation command service has no terminal response",
                    true,
                )),
            );
        }
    }
}

pub fn finish_simulation(event: On<CommandExecutedEvent>, mut state: ResMut<RulesTraceState>) {
    let Some((id, operation)) = state.request else {
        return;
    };
    let UiCommand::SimulateRuleTrace {
        operation: completed,
        ..
    } = &event.command
    else {
        return;
    };
    if event.request_id == id
        && *completed == operation
        && state.model.finish(operation, event.unit_result())
    {
        state.request = None;
    }
}

#[derive(Component, Clone, Copy, Default)]
pub struct TracerNativeField {
    pub initialized: bool,
}

#[derive(SystemParam)]
pub struct TraceObservation<'w> {
    latest: Res<'w, LatestSurfaceSnapshot>,
    last: Option<Res<'w, LastRulesProjection>>,
    locale: Res<'w, UiLocale>,
    route: Res<'w, ActiveRoute>,
    tab: Option<Res<'w, RulesTabState>>,
    composition: Option<Res<'w, ShellImeComposition>>,
}
impl TraceObservation<'_> {
    fn active(&self) -> bool {
        self.route.0 == Some(Route::Rules)
            && self
                .tab
                .as_ref()
                .is_some_and(|tab| tab.tab == RulesTab::Tracer)
    }
}

pub fn sync_basic_drafts(
    mut state: ResMut<RulesTraceState>,
    mut fields: Query<(&mut TextField, &NativeTextField, &mut TracerNativeField)>,
) {
    let first_mount = !state.drafts_initialized;
    let frozen = state.model.busy() || state.model.confirmation.is_some();
    let mut inputs = 0;
    for (mut field, native, mut draft) in &mut fields {
        if !draft.initialized {
            let restored = match native.0 {
                0 if !first_mount => Some(state.model.query.clone()),
                1 if !first_mount => Some(state.model.source_ip.clone()),
                2 => state.override_target.clone(),
                _ => None,
            };
            if let Some(text) = restored {
                field.0.apply(TextFieldInput::SetText(text));
            }
            draft.initialized = true;
        }
        match native.0 {
            0 => {
                inputs |= 1;
                if !frozen {
                    state.model.set_query(field.0.text().to_owned());
                }
            }
            1 => {
                inputs |= 2;
                if !frozen {
                    state.model.set_source_ip(field.0.text().to_owned());
                }
            }
            2 if !frozen => state.override_target = Some(field.0.text().to_owned()),
            _ => {}
        }
    }
    state.drafts_initialized |= inputs == 3;
}
pub fn observe_simulation(
    observation: TraceObservation,
    mut state: ResMut<RulesTraceState>,
    mut texts: Query<(&mut Text, &TracerText)>,
    mut fields: Query<
        (&mut TextField, &NativeTextField, &mut TextFieldFocused),
        With<TracerNativeField>,
    >,
    mut buttons: Query<(&TraceControlKind, &mut ButtonDisabled), Without<TracerPresetChip>>,
    mut presets: Query<&mut ButtonDisabled, With<TracerPresetChip>>,
) {
    let active = observation.active();
    let previous = state.model.snapshot.report_id;
    state.model.observe(observation.latest.0.rule_trace.clone());
    let projected = observation
        .last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .map(|projection| &projection.tracer);
    let report = state.model.snapshot.report.as_ref().or(projected);
    for (mut text, slot) in &mut texts {
        let wanted = match slot.0 {
            TRACER_HEADLINE_SLOT => trace_headline(
                report.and_then(|report| report.decision_chain.as_ref()),
                observation.locale.code(),
            ),
            stage @ 1..=TRACER_STAGE_SLOTS => report
                .map(|report| {
                    tracer_stage_line(report, (stage - 1) as usize, observation.locale.code())
                })
                .unwrap_or_default(),
            _ => format!(
                "{}\n{}",
                trace_status(&state.model, observation.locale.code()),
                trace_provenance(&state.model, observation.locale.code())
            ),
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
    let pending = !active || state.model.busy() || state.model.confirmation.is_some();
    let composing = observation
        .composition
        .as_ref()
        .is_some_and(|composition| composition.is_composing());
    for mut disabled in &mut presets {
        if disabled.0 != (pending || composing) {
            disabled.0 = pending || composing;
        }
    }
    let mut suggested_target = None;
    for (mut field, native, mut focused) in &mut fields {
        if !active {
            focused.0 = false;
        }
        if native.0 == 2
            && previous != state.model.snapshot.report_id
            && let Some(target) = report.and_then(|report| report.suggested_override_target.clone())
        {
            field.0.apply(TextFieldInput::SetText(target.clone()));
            suggested_target = Some(target);
        }
        if field.0.is_disabled() != pending {
            field.0.set_disabled(pending);
        }
    }
    if let Some(target) = suggested_target {
        state.override_target = Some(target);
    }
    let can_apply = observation
        .latest
        .0
        .pages
        .rules
        .data
        .as_ref()
        .is_some_and(|page| page.tracer.can_reverse_apply)
        && projected
            .is_some_and(|report| report.source.is_some() && state.model.draft_matches(report))
        && state.model.current_failure().is_none();
    for (kind, mut disabled) in &mut buttons {
        let enabled =
            !pending && !composing && (matches!(kind, TraceControlKind::Simulate) || can_apply);
        if disabled.0 == enabled {
            disabled.0 = !enabled;
        }
    }
}

/// DUAL-12-08: the reverse-apply button reads the traced rule index and the
/// typed target from the shared projection, falling back to the shared
/// `suggested_override_target`, and submits the override intent. It refuses
/// when the shared `can_reverse_apply` fact is false — no UI-local success.
pub(crate) fn on_tracer_override_activated(
    activate: On<Activate>,
    observation: TraceObservation,
    buttons: Query<(), With<ApplyTracerRuleOverrideButton>>,
    target_fields: Query<&Children, With<TracerOverrideTargetField>>,
    text_fields: Query<&TextField>,
    last: Option<Res<LastRulesProjection>>,
    mut state: ResMut<RulesTraceState>,
) {
    if !observation.active() || buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    let tracer = &projection.tracer;
    if !tracer.can_reverse_apply {
        return;
    }
    let Some(rule_index) = tracer
        .decision_chain
        .as_ref()
        .and_then(|chain| chain.hit_rule_index)
    else {
        return;
    };
    let typed = target_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .unwrap_or_default();
    let new_target = if typed.trim().is_empty() {
        tracer.suggested_override_target.clone().unwrap_or_default()
    } else {
        typed.trim().to_owned()
    };
    if new_target.is_empty() {
        return;
    }
    state.model.prepare_override(tracer, rule_index, new_target);
}

/// Scene constructor for the Live Rule Tracer card. Data-driven from the
/// shared `RuleTracerSnapshot` the surface reader projects; no fabricated
/// replay content is rendered when the snapshot has no decision chain.
pub fn rules_tracer_scene(palette: &UiPalette, tracer: &RuleTracerSnapshot) -> impl Scene + use<> {
    let presets = if tracer.presets.is_empty() {
        RuleTracerSnapshot::default_presets()
    } else {
        tracer.presets.clone()
    };
    let preset_chips: Vec<Box<dyn Scene>> = presets
        .into_iter()
        .map(|preset| {
            Box::new((
                localized_button_scene(
                    LocalizedText::new("rule_trace_preset", vec![("query", preset.label)]),
                    ButtonVariant::Default,
                    palette,
                ),
                bsn! { TracerPresetChip({ preset.query }) },
            )) as Box<dyn Scene>
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
                        Text({ content }) TracerText({ slot }) TextRole(role)
                    ]
        })
    };

    let mut decision_rows: Vec<Box<dyn Scene>> = vec![tracer_slot(
        TRACER_HEADLINE_SLOT,
        String::new(),
        Role::BodyStrong,
    )];
    for stage in 0..TRACER_STAGE_SLOTS {
        decision_rows.push(tracer_slot(stage + 1, String::new(), Role::Caption));
    }
    decision_rows.push(tracer_slot(TRACER_EMPTY_SLOT, String::new(), Role::Caption));

    let mut tracer_children: Vec<Box<dyn Scene>> = vec![
        Box::new(rules_tracer_sandbox::scene(palette)),
        Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::bottom(Val::Px(space::S8)),
                    }
                    RulesTracerRoot
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Activity, 24.0, palette) }
                            --
                            LocalizedText::plain("rules_tracer_title") TextRole(Role::BodyStrong)
                        ]
                        --
                        @{ (localized_button_scene(LocalizedText::plain("rules_tracer_run_action"), ButtonVariant::Primary, palette),
                            bsn! { SimulateRuleTraceButton TraceControlKind::Simulate }) }
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
                        Node { flex_grow: 1.0 }
                        TracerQueryField
                        Children [
                            @{ (localized_field_scene(query_initial, LocalizedText::plain("field_tracer_target"), palette),
                                bsn! { TracerNativeField NativeTextField(0) }) }
                        ]
                        --
                        Node { width: px(240.0) }
                        TracerSourceIpField
                        Children [
                            @{ (localized_field_scene(src_ip_initial, LocalizedText::plain("field_tracer_source"), palette),
                                bsn! { TracerNativeField NativeTextField(1) }) }
                        ]
                    ]
        }),
        Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                        row_gap: Val::Px(space::S8),
                        flex_wrap: FlexWrap::Wrap,
                        padding: UiRect::vertical(Val::Px(space::S6)),
                    }
                    Children [
                        { preset_chips }
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
                        { decision_rows }
                    ]
        }),
    ];
    // DUAL-12-08: the reverse-apply chooser is always mounted so it exists
    // before the first trace (the page mounts once per navigation); the
    // observer enforces the shared `can_reverse_apply` fact at click time and
    // the field seeds from the shared `suggested_override_target`.
    let override_initial = tracer.suggested_override_target.clone().unwrap_or_default();
    tracer_children.push(Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S8),
                padding: UiRect::vertical(Val::Px(space::S6)),
            }
            Children [
                LocalizedText::plain("tracer_override_apply") TextRole(Role::Caption)
                --
                Node { flex_grow: 1.0 }
                TracerOverrideTargetField
                Children [
                    @{ (localized_field_scene(override_initial, LocalizedText::plain("field_tracer_outbound"), palette),
                        bsn! { TracerNativeField NativeTextField(2) }) }
                ]
                --
                @{ (localized_button_scene(LocalizedText::plain("rules_apply_outbound_action"), ButtonVariant::Primary, palette),
                    bsn! { ApplyTracerRuleOverrideButton TraceControlKind::Apply }) }
            ]
    }));
    surface_scene(tracer_children, palette)
}
