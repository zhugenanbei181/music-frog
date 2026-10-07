//! Native confirmation owns cancel, retry and correlated transaction terminals.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::localized_widgets::localized_button_scene;
use crate::pages::rules_tracer::RulesTraceState;
use crate::route::{ActiveRoute, Route, RouteChanged};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::picking::events::PointerClick;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, GlobalZIndex, JustifyContent, Node,
    PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::Activate;
use infiltrator_application::rule_condition_projection::location_text;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_location::RuleLocation;

#[derive(Component, Clone, Copy, Default)]
pub struct TraceConfirmationRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct TraceConfirmationCard;
#[derive(Component, Clone, Copy, Default)]
pub enum TraceConfirmationAction {
    #[default]
    Cancel,
    Apply,
    Settings,
}
#[derive(Component, Clone, Copy, Default)]
pub struct TraceConfirmationCopy;

pub fn spawn(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(scene(&palette));
}
fn scene(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), display: Display::None, align_items: AlignItems::Center, justify_content: JustifyContent::Center }
        TraceConfirmationRoot GlobalZIndex(119)
        Children [
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100) }
            ModalScrim BackgroundColor({ palette.scrim }) TraceConfirmationAction::Cancel
            --
            Node { width: percent(90), max_width: px(560.0), padding: UiRect::all(px(16.0)), flex_direction: FlexDirection::Column, row_gap: px(12.0) }
            ModalDialogCard TraceConfirmationCard BackgroundColor({ palette.surface }) AccessibilityNode(semantic) LocalizedLabel::plain("rule_trace_confirm_title")
            Children [
                LocalizedText::plain("rule_trace_confirm_title") TextRole(Role::Heading)
                --
                Text(String::new()) TraceConfirmationCopy TextRole(Role::Body)
                --
                Node { width: percent(100), column_gap: px(12.0) }
                Children [
                    @{ (localized_button_scene(LocalizedText::plain("modal_cancel"), ButtonVariant::Default, palette), bsn! { TraceConfirmationAction::Cancel }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("rule_trace_confirm_apply"), ButtonVariant::Primary, palette), bsn! { TraceConfirmationAction::Apply }) }
                    --
                    @{ (localized_button_scene(LocalizedText::plain("dns_query_settings"), ButtonVariant::Default, palette), bsn! { TraceConfirmationAction::Settings }) }
                ]
            ]
        ]
    }
}
pub fn activate(
    event: On<Activate>,
    actions: Query<&TraceConfirmationAction>,
    mut state: ResMut<RulesTraceState>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    if state.model.confirmation.is_none() {
        return;
    }
    match action {
        TraceConfirmationAction::Cancel => {
            state.model.cancel_override();
        }
        TraceConfirmationAction::Settings => {
            if state
                .model
                .override_failure
                .as_ref()
                .is_some_and(|failure| {
                    matches!(
                        failure.code,
                        ErrorCode::Permission | ErrorCode::Authentication
                    )
                })
                && state.model.cancel_override()
            {
                commands.trigger(RouteChanged(Route::Settings));
            }
        }
        TraceConfirmationAction::Apply => {
            let Some((operation, request)) = state.model.begin_override() else {
                return;
            };
            if let Some(id) = sink.and_then(|sink| {
                sink.submit_tracked(UiCommand::ApplyTracerRuleOverride { request })
            }) {
                state.override_request = Some((id, operation));
            } else {
                state.model.finish_override(
                    operation,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "Rule apply command service has no terminal response",
                        true,
                    )),
                );
            }
        }
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<RulesTraceState>) {
    let Some((id, operation)) = state.override_request else {
        return;
    };
    let UiCommand::ApplyTracerRuleOverride { request } = &event.command else {
        return;
    };
    if event.request_id == id
        && state.model.confirmation.as_ref() == Some(request)
        && state.model.finish_override(operation, event.unit_result())
    {
        state.override_request = None;
    }
}
pub fn dismiss_scrim(
    event: On<PointerClick>,
    scrims: Query<&TraceConfirmationAction, With<ModalScrim>>,
    mut state: ResMut<RulesTraceState>,
) {
    if scrims.get(event.entity).is_ok() {
        state.model.cancel_override();
    }
}
pub fn render(
    route: Res<ActiveRoute>,
    locale: Res<UiLocale>,
    mut state: ResMut<RulesTraceState>,
    mut roots: Query<&mut Node, With<TraceConfirmationRoot>>,
    mut copy: Query<&mut Text, With<TraceConfirmationCopy>>,
    mut controls: Query<
        (&TraceConfirmationAction, &mut ButtonDisabled, &mut Node),
        Without<TraceConfirmationRoot>,
    >,
) {
    if route.0 != Some(Route::Rules) {
        state.model.cancel_override();
    }
    let open = state.model.confirmation.is_some();
    let pending = state.model.override_pending.is_some();
    for mut node in &mut roots {
        node.display = if open { Display::Flex } else { Display::None };
    }
    let message = state
        .model
        .confirmation
        .as_ref()
        .map(|request| {
            format!(
                "{}\n{}",
                location_text(
                    &RuleLocation {
                        table: request.rule_table.clone(),
                        index: request.rule_index
                    },
                    locale.code()
                ),
                LocalizedText::new(
                    "rule_trace_confirm_detail",
                    vec![
                        ("profile", request.expected_source.profile.clone()),
                        ("rule", request.expected_rule.clone()),
                        ("target", request.new_target.clone()),
                    ],
                )
                .render(&locale)
            )
        })
        .unwrap_or_default();
    let failure = state
        .model
        .override_failure
        .as_ref()
        .map(|failure| failure.message.as_str())
        .unwrap_or("");
    for mut text in &mut copy {
        text.0 = format!(
            "{message}\n{}\n{failure}",
            if pending {
                LocalizedText::plain("rule_trace_apply_running").render(&locale)
            } else {
                String::new()
            }
        );
    }
    let guide = state
        .model
        .override_failure
        .as_ref()
        .is_some_and(|failure| {
            matches!(
                failure.code,
                ErrorCode::Permission | ErrorCode::Authentication
            )
        });
    for (action, mut disabled, mut node) in &mut controls {
        disabled.0 = !open || pending;
        node.display = if matches!(action, TraceConfirmationAction::Settings) && !guide {
            Display::None
        } else {
            Display::Flex
        };
    }
}
