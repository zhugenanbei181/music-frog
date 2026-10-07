//! Scoped native events replay the shared statistics workbench.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::rules::ClearRuleHitCountersButton;
use crate::pages::rules_draft::RulesDraftState;
use crate::pages::rules_statistics_focus;
use crate::pages::rules_statistics_render::render;
use crate::pages::rules_statistics_scene::{scene, spawn_confirmation};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, Plugin, PostUpdate, Startup, Update};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::InputFocus;
use bevy::scene::Scene;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::rule_statistics_inspector_projection::StatisticsRowKey;
use infiltrator_application::rule_statistics_workbench::{
    RuleStatisticsWorkbench, StatisticsResetRequest, StatisticsTab,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, sync_button_disabled};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Resource, Default)]
pub struct RulesStatisticsState {
    pub model: RuleStatisticsWorkbench,
    pub request: Option<(RequestId, StatisticsResetRequest)>,
    pub row_keys: Vec<StatisticsRowKey>,
    pub blocked: Vec<Entity>,
    pub previous_focus: Option<Entity>,
    pub holding_focus: bool,
}
#[derive(Component, Clone, Copy, Default)]
pub struct StatisticsCard;
#[derive(Component, Clone, Copy, Default)]
pub struct StatisticsRows;
#[derive(Component, Clone, Copy, Default)]
pub struct StatisticsConfirmation;
#[derive(Component, Clone, Copy, Default)]
pub struct StatisticsConfirmationCard;
#[derive(Component, Clone, Copy, Default)]
#[require(Button, ButtonDisabled)]
pub enum StatisticsControl {
    #[default]
    Inspect,
    PrepareCleanup,
    ConfirmCleanup,
    CancelCleanup,
    Reset,
    DismissFailure,
    Previous,
    Next,
    Tab(StatisticsTab),
}
#[derive(Component, Clone, Copy, Default)]
pub enum StatisticsLine {
    #[default]
    Status,
    Source,
    Total,
    Dead,
    Cidr,
    Latency,
    Last,
    Empty,
    Feedback,
    Page,
    Confirmation,
}

pub fn statistics_scene(palette: &UiPalette) -> impl Scene + use<> {
    scene(palette)
}
pub struct RulesStatisticsPlugin;
impl Plugin for RulesStatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RulesStatisticsState>()
            .init_resource::<InputFocus>()
            .add_systems(Startup, spawn_confirmation)
            .add_observer(activate)
            .add_observer(finish)
            .add_systems(Update, observe)
            .add_systems(
                PostUpdate,
                (render, rules_statistics_focus::sync, ApplyDeferred)
                    .chain()
                    .before(sync_button_disabled),
            );
    }
}
fn observe(
    latest: Res<LatestSurfaceSnapshot>,
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<RulesStatisticsState>,
) {
    if latest.is_changed() {
        state.model.observe(&latest.0.pages.rules);
    }
    if state.model.confirmation.is_some()
        && (route.0 != Some(Route::Rules)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
    {
        state.model.cancel_cleanup();
    }
}
fn activate(
    event: On<Activate>,
    controls: Query<&StatisticsControl>,
    launchers: Query<(), With<ClearRuleHitCountersButton>>,
    route: Res<ActiveRoute>,
    mut state: ResMut<RulesStatisticsState>,
    mut draft: ResMut<RulesDraftState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if route.0 != Some(Route::Rules) {
        return;
    }
    let reset = StatisticsControl::Reset;
    let control = if launchers.contains(event.entity) {
        &reset
    } else {
        let Ok(control) = controls.get(event.entity) else {
            return;
        };
        control
    };
    if state.model.confirmation.is_some()
        && !matches!(
            control,
            StatisticsControl::CancelCleanup | StatisticsControl::ConfirmCleanup
        )
    {
        return;
    }
    match *control {
        StatisticsControl::Tab(tab) => state.model.select_tab(tab),
        StatisticsControl::Previous => state.model.previous_page(),
        StatisticsControl::Next => state.model.next_page(),
        StatisticsControl::Inspect => {
            state.model.inspect(&draft.model);
        }
        StatisticsControl::PrepareCleanup => {
            state.model.prepare_cleanup(&draft.model);
        }
        StatisticsControl::CancelCleanup => {
            state.model.cancel_cleanup();
        }
        StatisticsControl::ConfirmCleanup => {
            if state.model.confirm_cleanup(&mut draft.model).is_some() {
                draft.repaint = true;
            }
        }
        StatisticsControl::DismissFailure => state.model.clear_failure = None,
        StatisticsControl::Reset => {
            let Some(request) = state.model.begin_reset() else {
                return;
            };
            let command = UiCommand::ClearRuleHitCounters {
                expected_source: request.source.clone(),
            };
            if let Some(id) = sink.and_then(|sink| sink.submit_tracked(command)) {
                state.request = Some((id, request));
            } else {
                state.model.finish_reset(
                    &request,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "Rule statistics command service has no terminal response",
                        true,
                    )),
                );
            }
        }
    }
}
fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<RulesStatisticsState>) {
    let Some((id, request)) = state.request.clone() else {
        return;
    };
    let UiCommand::ClearRuleHitCounters { expected_source } = &event.command else {
        return;
    };
    if event.request_id != id || expected_source != &request.source {
        return;
    }
    if state.model.finish_reset(
        &request,
        event
            .result
            .clone()
            .and_then(CommandOutput::into_statistics_reset),
    ) {
        state.request = None;
    }
}
