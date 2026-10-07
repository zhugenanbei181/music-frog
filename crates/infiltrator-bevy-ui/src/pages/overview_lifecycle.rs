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
use bevy::ui::{
    BackgroundColor, BorderRadius, Display, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::core_control_projection::project_core_control;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::capability::{Availability, Capability};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::core_control::{CoreControlAction, CoreControlProjection};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlButton;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlCaption;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlIssue;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CoreControlIssueText;

#[derive(Clone, Debug)]
pub struct PendingCoreControl {
    pub request_id: RequestId,
    pub action: CoreControlAction,
    pub generation: u64,
    pub revision: u64,
    pub completed: bool,
}
#[derive(Resource, Clone, Debug, Default)]
pub struct CoreControlState {
    pub pending: Option<PendingCoreControl>,
    pub failure: Option<Failure>,
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
    if !has_sink {
        return project_core_control(core, None, false, None);
    }
    project_core_control(
        core,
        availability(snapshot),
        state.pending.is_some(),
        state.failure.as_ref(),
    )
}

pub fn core_control_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { min_width: px(0.0), max_width: px(320.0), flex_direction: FlexDirection::Column, row_gap: px(space::S4) }
        Children [
            Node {
                min_height: px(palette.control_height_px), padding: UiRect::horizontal(Val::Px(space::S12)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.accent })
            CoreControlButton Button ButtonDisabled(true)
            Children [ LocalizedText::plain("start_proxy") CoreControlCaption TextRole(Role::BodyStrong) ]
            --
            Node { width: percent(100), min_width: px(0.0), display: Display::None }
            CoreControlIssue
            Children [ LocalizedText::plain("common_message") CoreControlIssueText TextRole(Role::Caption) ]
        ]
    }
}

fn on_core_control_activated(
    activated: On<Activate>,
    buttons: Query<&ButtonDisabled>,
    controls: Query<&CoreControlButton>,
    core: Res<LatestCoreLifecycle>,
    snapshot: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<CoreControlState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if !controls.contains(activated.entity)
        || buttons
            .get(activated.entity)
            .is_ok_and(|disabled| disabled.0)
    {
        return;
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
        state.failure = Some(Failure::new(
            ErrorCode::NotReady,
            "core command sink supplies no terminal acknowledgment",
            true,
        ));
        return;
    };
    state.failure = None;
    state.pending = Some(PendingCoreControl {
        request_id,
        action,
        generation: core.0.generation,
        revision: core.0.revision,
        completed: false,
    });
}
fn finish_core_control(result: On<CommandExecutedEvent>, mut state: ResMut<CoreControlState>) {
    let Some(pending) = state.pending.as_mut() else {
        return;
    };
    let matches_action = matches!(
        (&result.command, pending.action),
        (UiCommand::StartCore, CoreControlAction::Start)
            | (UiCommand::StopCore, CoreControlAction::Stop)
    );
    if pending.request_id != result.request_id || !matches_action {
        return;
    }
    if result.result.is_ok() {
        pending.completed = true;
    } else {
        state.failure = result.result.as_ref().err().cloned();
        state.pending = None;
    }
}
fn reconcile_core_control(core: Res<LatestCoreLifecycle>, mut state: ResMut<CoreControlState>) {
    let Some(pending) = &state.pending else {
        return;
    };
    if !pending.completed {
        return;
    }
    let advanced = core.0.generation > pending.generation || core.0.revision > pending.revision;
    let reached = match pending.action {
        CoreControlAction::Start => matches!(
            core.0.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ),
        CoreControlAction::Stop => core.0.lifecycle == CoreLifecycle::Stopped,
    };
    if advanced && core.0.lifecycle == CoreLifecycle::Failed {
        state.failure = core.0.failure.clone();
        state.pending = None;
    } else if advanced && reached {
        state.pending = None;
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
