//! Native core controls replay the shared decision and correlate terminal results with snapshot revisions.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::surface::{LatestCoreLifecycle, LatestSurfaceSnapshot};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Or, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderRadius, Display, FlexDirection, Node, UiRect, percent, px};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::core_control::{
    CoreControlAction, CoreControlLabel, CoreControlProjection,
};
use infiltrator_presentation::core_control_projection::project_core_control;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::page::SurfaceOrigin;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlButton;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlCaption;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlIssue;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlIssueText;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingCoreControl {
    pub request_id: RequestId,
    pub action: CoreControlAction,
    pub generation: u64,
    pub revision: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CoreControlPhase {
    #[default]
    Idle,
    AwaitingResult(PendingCoreControl),
    AwaitingSnapshot(PendingCoreControl),
    Failed(Failure),
}
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct CoreControlState {
    pub phase: CoreControlPhase,
}

fn availability(snapshot: &LatestSurfaceSnapshot) -> Option<&Availability> {
    snapshot
        .0
        .capabilities
        .entries
        .iter()
        .find(|entry| entry.capability == Capability::CoreLifecycle)
        .map(|entry| &entry.availability)
}
fn projection(
    core: &CoreLifecycleSnapshot,
    snapshot: &LatestSurfaceSnapshot,
    state: &CoreControlState,
    has_sink: bool,
) -> CoreControlProjection {
    if snapshot.0.origin == SurfaceOrigin::Demo {
        return CoreControlProjection {
            action: None,
            label: CoreControlLabel::Unavailable,
            issue: None,
        };
    }
    if !has_sink {
        return project_core_control(core, None, false, None);
    }
    let (pending, failure) = match &state.phase {
        CoreControlPhase::Idle => (false, None),
        CoreControlPhase::AwaitingResult(_) | CoreControlPhase::AwaitingSnapshot(_) => (true, None),
        CoreControlPhase::Failed(failure) => (false, Some(failure)),
    };
    project_core_control(core, availability(snapshot), pending, failure)
}

pub fn core_control_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { min_width: px(0.0), max_width: rem(320.0), flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
        Children [
            Node {
                min_height: rem(palette.control_height_px), padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.accent })
            CoreControlButton Button ButtonDisabled(true)
            Children [ LocalizedText::plain("start_core") CoreControlCaption TextRole(Role::BodyStrong) ]
            --
            Node { width: percent(100), min_width: px(0.0), display: Display::None }
            CoreControlIssue
            Children [ LocalizedText::plain("common_message") CoreControlIssueText TextRole(Role::Caption) ]
        ]
    }
}

fn on_core_control_activated(
    activated: On<Activate>,
    controls: Query<&CoreControlButton>,
    core: Res<LatestCoreLifecycle>,
    snapshot: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<CoreControlState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if !controls.contains(activated.entity) {
        return;
    }
    if snapshot.0.origin == SurfaceOrigin::Demo {
        return;
    }
    match &state.phase {
        CoreControlPhase::AwaitingResult(_) | CoreControlPhase::AwaitingSnapshot(_) => return,
        CoreControlPhase::Idle | CoreControlPhase::Failed(_) => {}
    }
    let Some(sink) = sink else {
        return;
    };
    let Some(action) = projection(&core.0, &snapshot, &state, true).action else {
        return;
    };
    let command = match action {
        CoreControlAction::Start => UiCommand::StartCore,
        CoreControlAction::Stop => UiCommand::StopCore,
    };
    let Some(request_id) = sink.submit_tracked(command) else {
        state.phase = CoreControlPhase::Failed(Failure::new(
            ErrorCode::NotReady,
            "core command sink supplies no terminal acknowledgment",
            true,
        ));
        return;
    };
    state.phase = CoreControlPhase::AwaitingResult(PendingCoreControl {
        request_id,
        action,
        generation: core.0.generation,
        revision: core.0.revision,
    });
}
fn finish_core_control(result: On<CommandExecutedEvent>, mut state: ResMut<CoreControlState>) {
    let pending = match &state.phase {
        CoreControlPhase::AwaitingResult(pending) => pending.clone(),
        CoreControlPhase::Idle
        | CoreControlPhase::AwaitingSnapshot(_)
        | CoreControlPhase::Failed(_) => return,
    };
    let matches_action = matches!(
        (&result.command, pending.action),
        (UiCommand::StartCore, CoreControlAction::Start)
            | (UiCommand::StopCore, CoreControlAction::Stop)
    );
    if pending.request_id != result.request_id || !matches_action {
        return;
    }
    state.phase = match &result.result {
        Ok(_) => CoreControlPhase::AwaitingSnapshot(pending),
        Err(failure) => CoreControlPhase::Failed(failure.clone()),
    };
}
fn reconcile_core_control(core: Res<LatestCoreLifecycle>, mut state: ResMut<CoreControlState>) {
    let pending = match &state.phase {
        CoreControlPhase::AwaitingSnapshot(pending) => pending.clone(),
        CoreControlPhase::Idle
        | CoreControlPhase::AwaitingResult(_)
        | CoreControlPhase::Failed(_) => return,
    };
    let advanced = core.0.generation > pending.generation || core.0.revision > pending.revision;
    let reached = match pending.action {
        CoreControlAction::Start => matches!(
            core.0.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ),
        CoreControlAction::Stop => core.0.lifecycle == CoreLifecycle::Stopped,
    };
    if advanced && core.0.lifecycle == CoreLifecycle::Failed {
        state.phase = CoreControlPhase::Failed(core.0.failure.clone().unwrap_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "core lifecycle failed without a reported failure",
                true,
            )
        }));
    } else if advanced && reached {
        state.phase = CoreControlPhase::Idle;
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct CoreControlViews {
    caption: Option<&'static CoreControlCaption>,
    issue: Option<&'static CoreControlIssue>,
    issue_text: Option<&'static CoreControlIssueText>,
    text: Option<&'static mut Text>,
    ink: Option<&'static mut TextColor>,
    copy: Option<&'static mut LocalizedText>,
    node: Option<&'static mut Node>,
    disabled: Option<&'static mut ButtonDisabled>,
    button: Option<&'static CoreControlButton>,
    fill: Option<&'static mut BackgroundColor>,
}
type CoreControlFilter = Or<(
    With<CoreControlCaption>,
    With<CoreControlIssue>,
    With<CoreControlIssueText>,
    With<CoreControlButton>,
)>;

fn sync_core_control(
    core: Res<LatestCoreLifecycle>,
    snapshot: Res<LatestSurfaceSnapshot>,
    state: Res<CoreControlState>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    sink: Option<Res<CommandSinkHandle>>,
    mut views: Query<CoreControlViews, CoreControlFilter>,
) {
    if !core.is_changed()
        && !snapshot.is_changed()
        && !state.is_changed()
        && !locale.is_changed()
        && !palette.is_changed()
        && !sink.as_ref().is_some_and(|sink| sink.is_changed())
    {
        return;
    }
    let control = projection(&core.0, &snapshot, &state, sink.is_some());
    for mut view in &mut views {
        if view.caption.is_some() {
            if let Some(ink) = &mut view.ink {
                ink.0 = if control.action.is_some() {
                    palette.on_accent
                } else {
                    palette.ink
                };
            }
            if let (Some(copy), Some(text)) = (&mut view.copy, &mut view.text) {
                copy.key = control.label.key();
                text.0 = copy.render(&locale);
            }
        } else if view.issue.is_some() {
            if let Some(node) = &mut view.node {
                node.display = if control.issue.is_some() {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        } else if view.button.is_some() {
            if let Some(fill) = &mut view.fill {
                fill.0 = match control.action {
                    Some(CoreControlAction::Stop) => palette.danger,
                    Some(CoreControlAction::Start) => palette.accent,
                    None => palette.surface_elevated,
                };
            }
            if let Some(disabled) = &mut view.disabled {
                disabled.0 = control.action.is_none();
            }
        } else if view.issue_text.is_some()
            && let (Some(copy), Some(text)) = (&mut view.copy, &mut view.text)
        {
            copy.params = vec![(
                "message",
                control
                    .issue
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_default(),
            )];
            text.0 = copy.render(&locale);
        }
    }
}

pub struct CoreControlPlugin;
impl Plugin for CoreControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CoreControlState>();
        app.add_observer(on_core_control_activated);
        app.add_observer(finish_core_control);
        app.add_systems(Update, (reconcile_core_control, sync_core_control).chain());
    }
}
