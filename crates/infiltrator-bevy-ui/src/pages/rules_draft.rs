//! Native staged rules controls; only the explicit Save action submits a command.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::localized_widgets::localized_button_scene;
use crate::pages::rules::RulesProjectionUpdated;
use crate::pages::rules_builder::{
    DiscardRuleFormButton, RuleFormMutationButton, RulesBuilderState,
};
use crate::pages::rules_statistics::RulesStatisticsState;
use crate::route::{ActiveRoute, Route, RouteChanged};
use crate::surface::{LatestSurfaceSnapshot, rules_projection};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{FlexDirection, FlexWrap, Node, percent, px};
use bevy::ui_widgets::Activate;
use infiltrator_application::rule_list_editor::RuleListEditor;
use infiltrator_application::rule_list_projection::{draft_page, editor_preview, editor_status};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_document::RuleListOperationId;
use infiltrator_contract::surface_snapshot::PageData;

#[derive(Resource, Default)]
pub struct RulesDraftState {
    pub model: RuleListEditor,
    pub request: Option<(RequestId, RuleListOperationId)>,
    pub repaint: bool,
    pub input_composing: bool,
}
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuleListControl {
    #[default]
    Save,
    Discard,
    Settings,
}
#[derive(Component, Clone, Default)]
pub struct RuleListStatus;
#[derive(Component, Clone, Default)]
pub struct RuleDraftMutationButton;
#[derive(Component, Clone, Default)]
pub struct RuleListEditorCard;
#[derive(Component, Clone, Default)]
pub struct RuleListPreview;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct RuleMutationControl {
    disabled: &'static mut ButtonDisabled,
    form: Option<&'static RuleFormMutationButton>,
}

pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
        RuleListEditorCard
        Children [
            Text(String::new()) RuleListStatus TextRole(Role::Caption)
            --
            Text(String::new()) RuleListPreview TextRole(Role::Body)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(8.0) }
            Children [
                @{ (localized_button_scene(LocalizedText::plain("rules_save_btn"), ButtonVariant::Primary, palette), bsn! { RuleListControl::Save ButtonDisabled(true) }) }
                --
                @{ (localized_button_scene(LocalizedText::plain("rules_draft_discard"), ButtonVariant::Default, palette), bsn! { RuleListControl::Discard ButtonDisabled(true) }) }
                --
                @{ (localized_button_scene(LocalizedText::plain("dns_query_settings"), ButtonVariant::Default, palette), bsn! { RuleListControl::Settings ButtonDisabled(true) }) }
            ]
        ]
    }
}
pub fn observe(
    latest: Res<LatestSurfaceSnapshot>,
    route: Res<ActiveRoute>,
    mut state: ResMut<RulesDraftState>,
    mut commands: Commands,
) {
    if latest.is_changed() {
        state.repaint |= state.model.observe_page(&latest.0.pages.rules);
    }
    if route.0 != Some(Route::Rules) || state.model.base.is_none() {
        return;
    }
    if !state.repaint && !route.is_changed() && !latest.is_changed() {
        return;
    }
    let Some(applied) = latest.0.pages.rules.data.as_ref() else {
        return;
    };
    let page = draft_page(applied, &state.model);
    let mut snapshot = latest.0.clone();
    snapshot.pages.rules = PageData::ready(page);
    commands.trigger(RulesProjectionUpdated(rules_projection(&snapshot)));
    state.repaint = false;
}
pub fn activate(
    event: On<Activate>,
    statistics: Option<Res<RulesStatisticsState>>,
    actions: Query<&RuleListControl>,
    mut state: ResMut<RulesDraftState>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if statistics.is_some_and(|state| state.model.confirmation.is_some()) {
        return;
    }
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    match action {
        RuleListControl::Settings => {
            if state.model.can_guide() {
                commands.trigger(RouteChanged(Route::Settings));
            }
        }
        RuleListControl::Discard => {
            state.repaint |= state.model.discard();
        }
        RuleListControl::Save => {
            if state.input_composing {
                return;
            }
            let Some(sink) = sink else {
                state.model.failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "The rule command service is unavailable",
                    true,
                ));
                return;
            };
            let (operation, request) = match state.model.begin() {
                Ok(pending) => pending,
                Err(failure) => {
                    state.model.failure = Some(failure);
                    return;
                }
            };
            if let Some(id) = sink.submit_tracked(UiCommand::CommitRuleList { request }) {
                state.request = Some((id, operation));
            } else {
                state.model.finish(
                    operation,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "Rule commit has no terminal response",
                        true,
                    )),
                );
            }
        }
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<RulesDraftState>) {
    if let Some((id, operation)) = state.request
        && id == event.request_id
        && matches!(event.command, UiCommand::CommitRuleList { .. })
        && state.model.finish(operation, event.unit_result())
    {
        state.request = None;
    }
}
pub fn render(
    state: Res<RulesDraftState>,
    builder: Res<RulesBuilderState>,
    locale: Res<UiLocale>,
    mut controls: Query<(&RuleListControl, &mut ButtonDisabled)>,
    mut labels: Query<&mut Text, With<RuleListStatus>>,
    mut previews: Query<&mut Text, (With<RuleListPreview>, Without<RuleListStatus>)>,
    mut mutations: Query<
        RuleMutationControl,
        (With<RuleDraftMutationButton>, Without<RuleListControl>),
    >,
) {
    for mut control in &mut mutations {
        control.disabled.0 = !state.model.editable()
            || state.input_composing
            || (control.form.is_some() && !builder.binding.current(&state.model));
    }
    for (control, mut disabled) in &mut controls {
        disabled.0 = match control {
            RuleListControl::Settings => !state.model.can_guide(),
            RuleListControl::Save => !state.model.can_save() || state.input_composing,
            RuleListControl::Discard => {
                state.model.pending.is_some()
                    || state.model.awaiting_read
                    || (!state.model.dirty() && !state.model.source_changed())
            }
        };
    }
    let status = editor_status(&state.model, locale.code());
    for mut text in &mut labels {
        text.0.clone_from(&status);
    }
    let preview = editor_preview(&state.model, locale.code()).join("\n");
    for mut text in &mut previews {
        text.0.clone_from(&preview);
    }
}

pub fn sync_form_discard(
    state: Res<RulesDraftState>,
    mut buttons: Query<&mut ButtonDisabled, With<DiscardRuleFormButton>>,
) {
    for mut disabled in &mut buttons {
        disabled.0 = state.model.pending.is_some() || state.model.awaiting_read;
    }
}
