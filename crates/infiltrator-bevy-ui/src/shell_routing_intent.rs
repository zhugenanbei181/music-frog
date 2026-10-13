//! Routing intent controls replay observed facts through scoped ECS queries.
use crate::app::{
    PendingRoutingIntentRequest, RoutingIntentActionState, RoutingIntentRequest,
    RoutingIntentStatusCapsule,
};
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::overview::OverviewProjectionUpdated;
use crate::pages::overview::OverviewRoutingIntentPill;
use crate::pages::overview_cards::{
    OverviewRoutingIntentCaption, OverviewRoutingIntentSegmentPill, OverviewRoutingIntentText,
};
use crate::route::OverviewSourceHandle;
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::routing_intent_application::RoutingIntentApplication;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_presentation::routing_intent_actions::{
    RoutingIntentActions, RoutingIntentFinishOutcome,
};
use infiltrator_presentation::routing_intent_projection::routing_intent_status_copy;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct RoutingIntentControl {
    visual: &'static mut ControlVisual,
    disabled: &'static mut ButtonDisabled,
    pill: Option<&'static OverviewRoutingIntentPill>,
    segment: Option<&'static OverviewRoutingIntentSegmentPill>,
}

#[derive(QueryFilter)]
pub struct RoutingIntentFilter {
    intent: Or<(
        With<OverviewRoutingIntentPill>,
        With<OverviewRoutingIntentSegmentPill>,
    )>,
}

#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct RequestRoutingIntentChange(pub RoutingIntent);

pub fn request(request: On<RequestRoutingIntentChange>, state: RoutingIntentActivation) {
    begin_request(request.0.clone(), state);
}

pub fn sync_segments(
    actions: Res<RoutingIntentActionState>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    mut controls: Query<(&OverviewRoutingIntentSegmentPill, &mut BackgroundColor)>,
    mut labels: Query<(&OverviewRoutingIntentText, &mut TextColor)>,
    mut captions: Query<&mut Text, With<OverviewRoutingIntentCaption>>,
) {
    let state = actions.0.render_snapshot();
    for mut caption in &mut captions {
        caption.0 = routing_intent_status_copy(&state, locale.code());
    }
    for (intent, mut background) in &mut controls {
        background.0 = if state.current.as_ref() == Some(&intent.0) {
            palette.accent
        } else if state.is_intent_selectable(&intent.0) {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
    }
    for (intent, mut color) in &mut labels {
        color.0 = if state.current.as_ref() == Some(&intent.0) {
            palette.on_accent
        } else if state.is_intent_selectable(&intent.0) {
            palette.accent
        } else {
            palette.ink_dim
        };
    }
}

pub fn sync(
    latest: Option<Res<LatestSurfaceSnapshot>>,
    source: Option<Res<OverviewSourceHandle>>,
    mut actions: ResMut<RoutingIntentActionState>,
    locale: Res<UiLocale>,
    mut controls: Query<RoutingIntentControl, RoutingIntentFilter>,
    capsules: Query<&Children, With<RoutingIntentStatusCapsule>>,
    mut labels: Query<&mut Text>,
) {
    observe_actions(&mut actions.0, latest.as_deref(), source.as_deref());
    let state = actions.0.render_snapshot();
    for mut control in &mut controls {
        let intent = control
            .pill
            .map(|pill| pill.0.clone())
            .or_else(|| control.segment.map(|pill| pill.0.clone()));
        if let Some(intent) = intent {
            control.visual.0 = state.current.as_ref() == Some(&intent);
            control.disabled.0 = !state.is_intent_selectable(&intent);
        }
    }
    let caption = routing_intent_status_copy(&state, locale.code());
    for children in &capsules {
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(*child) {
                text.0 = caption.clone();
            }
        }
    }
}

fn observe_actions(
    actions: &mut RoutingIntentActions,
    latest: Option<&LatestSurfaceSnapshot>,
    source: Option<&OverviewSourceHandle>,
) {
    if let Some(latest) = latest {
        actions.observe(
            latest.0.generation,
            latest.0.revision,
            RoutingIntentApplication::from_surface(&latest.0),
        );
    } else if let Some(source) = source {
        let value = source.0.current();
        actions.observe(
            0,
            u64::try_from(value.sampled_at.as_nanos()).unwrap_or(u64::MAX),
            value.routing_intent,
        );
    }
}

#[derive(SystemParam)]
pub struct RoutingIntentActivation<'w, 's> {
    pills: Query<'w, 's, &'static OverviewRoutingIntentPill>,
    segments: Query<'w, 's, &'static OverviewRoutingIntentSegmentPill>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
    handle: Option<Res<'w, OverviewSourceHandle>>,
    sink: Option<Res<'w, CommandSinkHandle>>,
    pending: ResMut<'w, PendingRoutingIntentRequest>,
    pub(crate) actions: ResMut<'w, RoutingIntentActionState>,
}

pub fn activate(activate: On<Activate>, state: RoutingIntentActivation) {
    let wanted = state
        .pills
        .get(activate.entity)
        .map(|pill| pill.0.clone())
        .ok()
        .or_else(|| {
            state
                .segments
                .get(activate.entity)
                .map(|pill| pill.0.clone())
                .ok()
        });
    if let Some(wanted) = wanted {
        begin_request(wanted, state);
    }
}

pub(crate) fn begin_request(wanted: RoutingIntent, state: RoutingIntentActivation) {
    let RoutingIntentActivation {
        latest,
        handle,
        sink,
        mut pending,
        mut actions,
        ..
    } = state;
    if actions.0.pending_change().is_some() {
        return;
    }
    observe_actions(&mut actions.0, latest.as_deref(), handle.as_deref());
    let Ok(transition) = actions.0.begin(wanted.clone()) else {
        return;
    };
    let Some(sink) = sink else {
        let _ = actions.0.finish(
            transition,
            Err(Failure::new(
                ErrorCode::NotReady,
                "routing intent command service is unavailable",
                true,
            )),
        );
        return;
    };
    let Some(request_id) = sink.submit_tracked(UiCommand::SetRoutingIntent(wanted)) else {
        let _ = actions.0.finish(
            transition,
            Err(Failure::new(
                ErrorCode::NotReady,
                "routing intent command service did not accept terminal feedback",
                true,
            )),
        );
        return;
    };
    pending.0 = Some(RoutingIntentRequest {
        transition,
        request_id,
    });
}

pub fn completed(
    event: On<CommandExecutedEvent>,
    mut pending: ResMut<PendingRoutingIntentRequest>,
    mut actions: ResMut<RoutingIntentActionState>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    handle: Option<Res<OverviewSourceHandle>>,
    mut commands: Commands,
) {
    let Some(request) = pending.0.as_ref() else {
        return;
    };
    if request.request_id != event.request_id
        || event.command != UiCommand::SetRoutingIntent(request.transition.target.clone())
    {
        return;
    }
    let request = pending
        .0
        .take()
        .expect("routing intent request was present");
    let result = match &event.result {
        Ok(CommandOutput::RoutingIntentApplied { observed }) => Ok(observed.clone()),
        Ok(_) => Err(Failure::new(
            ErrorCode::InvalidState,
            "routing intent command returned an unexpected output",
            false,
        )),
        Err(failure) => Err(failure.clone()),
    };
    let verified = result.as_ref().ok().cloned();
    match actions.0.finish(request.transition, result) {
        RoutingIntentFinishOutcome::Applied => {}
        RoutingIntentFinishOutcome::IgnoredStale => return,
    }
    if let Some(observed) = verified {
        if let Some(latest) = latest {
            commands.trigger(SurfaceSnapshotUpdated(
                RoutingIntentApplication::with_verified_intent(&latest.0, observed),
            ));
        } else if let Some(source) = handle {
            commands.trigger(OverviewProjectionUpdated(source.0.current()));
        }
    }
}
