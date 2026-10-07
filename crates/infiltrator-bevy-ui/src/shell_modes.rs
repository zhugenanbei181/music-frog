//! Mode controls replay observed facts through scoped ECS queries.
use crate::app::{
    GlobalModeCapsule, ModeAckSlot, ModeActionState, PendingModeAck, SidebarScriptModePill,
};
use crate::pages::overview::OverviewModePill;
use crate::pages::overview::OverviewProjectionUpdated;
use crate::pages::overview_cards::{
    OverviewModeCaption, OverviewModeSegmentPill, OverviewModeSegmentText,
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
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_application::proxy_mode_projection::mode_status_copy;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::sync::{
    Mutex,
    mpsc::{TryRecvError, channel},
};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ModeControl {
    visual: &'static mut ControlVisual,
    disabled: &'static mut ButtonDisabled,
    mode: Option<&'static OverviewModePill>,
    segment: Option<&'static OverviewModeSegmentPill>,
    script: Option<&'static SidebarScriptModePill>,
}

#[derive(QueryFilter)]
pub struct ModeFilter {
    mode: Or<(
        With<OverviewModePill>,
        With<OverviewModeSegmentPill>,
        With<SidebarScriptModePill>,
    )>,
}

#[derive(Event, Clone, Copy)]
pub struct RequestModeChange(pub ProxyMode);

pub fn request(request: On<RequestModeChange>, state: ModeActivation) {
    begin_request(request.0, state);
}

pub fn sync_segments(
    actions: Res<ModeActionState>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    mut controls: Query<(&OverviewModeSegmentPill, &mut BackgroundColor)>,
    mut labels: Query<(&OverviewModeSegmentText, &mut TextColor)>,
    mut captions: Query<&mut Text, With<OverviewModeCaption>>,
) {
    let state = actions.0.render_snapshot();
    for mut caption in &mut captions {
        caption.0 = mode_status_copy(&state, locale.code());
    }
    for (mode, mut background) in &mut controls {
        background.0 = if state.current == Some(mode.0) {
            palette.accent
        } else if state.is_mode_selectable(mode.0) {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
    }
    for (mode, mut color) in &mut labels {
        color.0 = if state.current == Some(mode.0) {
            palette.on_accent
        } else if state.is_mode_selectable(mode.0) {
            palette.accent
        } else {
            palette.ink_dim
        };
    }
}

pub fn sync(
    latest: Option<Res<LatestSurfaceSnapshot>>,
    source: Option<Res<OverviewSourceHandle>>,
    mut actions: ResMut<ModeActionState>,
    locale: Res<UiLocale>,
    mut controls: Query<ModeControl, ModeFilter>,
    capsules: Query<&Children, With<GlobalModeCapsule>>,
    mut labels: Query<&mut Text>,
) {
    if let Some(latest) = latest {
        actions.0.observe(
            latest.0.generation,
            latest.0.revision,
            ProxyModeApplication::from_surface(&latest.0),
        );
    } else if let Some(source) = source {
        let value = source.0.current();
        actions.0.observe(
            0,
            u64::try_from(value.sampled_at.as_nanos()).unwrap_or(u64::MAX),
            value.proxy_mode,
        );
    }
    let state = actions.0.render_snapshot();
    for mut control in &mut controls {
        let mode = control
            .mode
            .map(|mode| mode.0)
            .or_else(|| control.segment.map(|mode| mode.0))
            .or_else(|| control.script.map(|_| ProxyMode::Script));
        if let Some(mode) = mode {
            control.visual.0 = state.current == Some(mode);
            control.disabled.0 = !state.is_mode_selectable(mode);
        }
    }
    let caption = mode_status_copy(&state, locale.code());
    for children in &capsules {
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(*child) {
                text.0 = caption.clone();
            }
        }
    }
}

#[derive(SystemParam)]
pub struct ModeActivation<'w, 's> {
    pills: Query<'w, 's, &'static OverviewModePill>,
    segments: Query<'w, 's, &'static OverviewModeSegmentPill>,
    scripts: Query<'w, 's, (), With<SidebarScriptModePill>>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
    handle: Option<Res<'w, OverviewSourceHandle>>,
    pending: ResMut<'w, PendingModeAck>,
    pub(crate) actions: ResMut<'w, ModeActionState>,
}
pub fn activate(activate: On<Activate>, state: ModeActivation) {
    let wanted = state
        .pills
        .get(activate.entity)
        .map(|pill| pill.0)
        .ok()
        .or_else(|| {
            state
                .segments
                .get(activate.entity)
                .map(|segment| segment.0)
                .ok()
        })
        .or_else(|| {
            state
                .scripts
                .contains(activate.entity)
                .then_some(ProxyMode::Script)
        });
    if let Some(wanted) = wanted {
        begin_request(wanted, state);
    }
}

pub(crate) fn begin_request(wanted: ProxyMode, state: ModeActivation) {
    let ModeActivation {
        latest,
        handle,
        mut pending,
        mut actions,
        ..
    } = state;
    if actions.0.pending.is_some() {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    if let Some(latest) = latest {
        actions.0.observe(
            latest.0.generation,
            latest.0.revision,
            ProxyModeApplication::from_surface(&latest.0),
        );
    } else {
        let value = handle.0.current();
        actions.0.observe(
            0,
            u64::try_from(value.sampled_at.as_nanos()).unwrap_or(u64::MAX),
            value.proxy_mode,
        );
    }
    let Ok(request) = actions.0.begin(wanted) else {
        return;
    };
    let (ack_tx, ack_rx) = channel();
    handle.0.set_mode(wanted, ack_tx);
    pending.0 = Some(ModeAckSlot {
        request,
        receiver: Mutex::new(ack_rx),
    });
}

#[derive(SystemParam)]
pub struct ModeAcknowledgement<'w> {
    pending: ResMut<'w, PendingModeAck>,
    actions: ResMut<'w, ModeActionState>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
    handle: Option<Res<'w, OverviewSourceHandle>>,
}
pub fn drain_ack(state: ModeAcknowledgement, mut commands: Commands) {
    let ModeAcknowledgement {
        mut pending,
        mut actions,
        latest,
        handle,
    } = state;
    let Some(slot) = pending.0.as_mut() else {
        return;
    };
    let request = slot.request;
    let outcome = match slot
        .receiver
        .get_mut()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .try_recv()
    {
        Ok(receipt) => Some(receipt),
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err(Failure::new(
            ErrorCode::NotReady,
            "Mode command channel disconnected",
            true,
        ))),
    };
    let Some(receipt) = outcome else {
        return;
    };
    pending.0 = None;
    if let Some(latest) = &latest {
        actions.0.observe(
            latest.0.generation,
            latest.0.revision,
            ProxyModeApplication::from_surface(&latest.0),
        );
    }
    let actual = receipt.as_ref().ok().copied();
    if !actions.0.finish(request, receipt) {
        return;
    }
    if let Some(actual) = actual {
        if let Some(latest) = latest {
            commands.trigger(SurfaceSnapshotUpdated(
                ProxyModeApplication::with_verified_mode(&latest.0, actual),
            ));
        } else if let Some(handle) = handle {
            commands.trigger(OverviewProjectionUpdated(handle.0.current()));
        }
    }
}
