//! Inspector latency actions use the shared leaf-probe intent and tracked terminal results.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxy_inspection::ProxyInspectionReadPhase;
use crate::pages::proxy_inspection::{
    InspectionScope, ProxyInspectionPhase, ProxyInspectionState, ProxyProbePhase,
};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityStatus, CoreOperation, OutboundOperation,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::{
    details::OutboundProbeAvailability,
    inventory::{OutboundInventoryState, OutboundProbeState},
    probe::ProbeOutboundRequest,
};
use infiltrator_contract::runtime_settings::RuntimeProbeSettingsState;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingProxyProbe {
    pub request: ProbeOutboundRequest,
    pub request_id: RequestId,
}
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProbeInspectedProxy;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyProbeCaption;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyProbeStatus;
type ProbeCaptionFilter = (With<ProxyProbeCaption>, Without<ProxyProbeStatus>);
type ProbeStatusFilter = (With<ProxyProbeStatus>, Without<ProxyProbeCaption>);

pub fn probe(
    activated: On<Activate>,
    buttons: Query<(), With<ProbeInspectedProxy>>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProxyInspectionState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if buttons.get(activated.entity).is_err() {
        return;
    }
    let (outbound_id, observed_scope) = match &state.phase {
        ProxyInspectionPhase::Closed => return,
        ProxyInspectionPhase::Open(session) => {
            match &session.probe {
                ProxyProbePhase::Pending(_) => return,
                ProxyProbePhase::Idle | ProxyProbePhase::Failed(_) => {}
            }
            match &session.read.phase {
                ProxyInspectionReadPhase::Ready => {}
                ProxyInspectionReadPhase::Loading
                | ProxyInspectionReadPhase::Failed(_)
                | ProxyInspectionReadPhase::Unsupported(_) => {
                    return;
                }
            }
            let Some(detail) = session.read.detail.as_ref() else {
                return;
            };
            if detail.probe_availability != OutboundProbeAvailability::Supported {
                return;
            }
            (session.selected.clone(), session.scope.clone())
        }
    };
    let observed_scope = match observed_scope {
        InspectionScope::Active(scope) => scope,
        InspectionScope::NoActiveCore => return,
    };
    let current_scope = latest.0.core.scope();
    if current_scope.as_ref() != Some(&observed_scope) {
        return;
    }
    if probe_capability(&latest.0) != ProbeCapability::Available {
        return;
    }
    let Some(inventory) = latest
        .0
        .outbound_inventory
        .data
        .as_ref()
        .filter(|_| {
            matches!(
                latest.0.outbound_inventory.status,
                PageStatus::Ready | PageStatus::Empty
            )
        })
        .filter(|inventory| latest.0.core.scope().as_ref() == Some(inventory.scope()))
    else {
        return;
    };
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        return;
    };
    let Some(details) = latest
        .0
        .outbound_details
        .data
        .as_ref()
        .filter(|_| {
            matches!(
                latest.0.outbound_details.status,
                PageStatus::Ready | PageStatus::Empty
            )
        })
        .filter(|details| current_scope.as_ref() == Some(details.scope()))
    else {
        return;
    };
    if !entries.iter().any(|entry| entry.id == outbound_id)
        || !details
            .detail(&outbound_id)
            .is_some_and(|detail| detail.probe_availability == OutboundProbeAvailability::Supported)
    {
        return;
    }
    let settings = match &latest.0.probe_settings {
        RuntimeProbeSettingsState::Available { settings } => settings,
        RuntimeProbeSettingsState::Unavailable { failure } => {
            record_probe_phase(&mut state, ProxyProbePhase::Failed(failure.clone()));
            return;
        }
    };
    let request = match ProbeOutboundRequest::new(
        outbound_id,
        inventory.revision(),
        settings.url().to_owned(),
        u64::from(settings.timeout_ms()),
    ) {
        Ok(request) => request,
        Err(error) => {
            record_probe_phase(
                &mut state,
                ProxyProbePhase::Failed(Failure::new(
                    ErrorCode::InvalidInput,
                    error.to_string(),
                    false,
                )),
            );
            return;
        }
    };
    let Some(sink) = sink else { return };
    if let Some(request_id) = sink.submit_tracked(UiCommand::ProbeOutbound {
        request: request.clone(),
    }) {
        record_probe_phase(
            &mut state,
            ProxyProbePhase::Pending(PendingProxyProbe {
                request,
                request_id,
            }),
        );
    } else {
        record_probe_phase(
            &mut state,
            ProxyProbePhase::Failed(Failure::new(
                ErrorCode::NotReady,
                "proxy probe service supplies no terminal acknowledgment",
                true,
            )),
        );
    }
}

pub fn finish_probe(result: On<CommandExecutedEvent>, mut state: ResMut<ProxyInspectionState>) {
    let (pending, selected, observed_scope, detail_revision) = match &state.phase {
        ProxyInspectionPhase::Closed => return,
        ProxyInspectionPhase::Open(session) => match &session.probe {
            ProxyProbePhase::Pending(pending) => (
                pending.clone(),
                &session.selected == pending.request.outbound_id(),
                session.scope.clone(),
                session.read.detail_revision,
            ),
            ProxyProbePhase::Idle | ProxyProbePhase::Failed(_) => return,
        },
    };
    if pending.request_id != result.request_id
        || !matches!(
            &result.command,
            UiCommand::ProbeOutbound { request } if request == &pending.request
        )
        || !selected
    {
        return;
    }
    let phase = match &result.result {
        Err(failure) => ProxyProbePhase::Failed(failure.clone()),
        Ok(CommandOutput::OutboundProbe(snapshot))
            if pending.request.is_confirmed_by(snapshot)
                && matches!(&observed_scope, InspectionScope::Active(scope) if scope == snapshot.scope()) =>
        {
            if detail_revision.is_some_and(|revision| snapshot.detail_revision() <= revision) {
                ProxyProbePhase::Failed(Failure::new(
                    ErrorCode::InvalidState,
                    "outbound probe did not advance the detail observation revision",
                    false,
                ))
            } else {
                match snapshot.state() {
                    OutboundProbeState::Measured { .. } => ProxyProbePhase::Idle,
                    OutboundProbeState::Failed { failure } => {
                        ProxyProbePhase::Failed(failure.clone())
                    }
                    OutboundProbeState::TimedOut => ProxyProbePhase::Failed(Failure::new(
                        ErrorCode::Network,
                        "outbound probe timed out",
                        true,
                    )),
                    OutboundProbeState::Unsupported => ProxyProbePhase::Failed(Failure::new(
                        ErrorCode::Unsupported,
                        "outbound probe is unsupported",
                        false,
                    )),
                    OutboundProbeState::Unknown | OutboundProbeState::Probing => {
                        ProxyProbePhase::Failed(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound probe returned a non-terminal observation",
                            true,
                        ))
                    }
                }
            }
        }
        Ok(_) => ProxyProbePhase::Failed(Failure::new(
            ErrorCode::InvalidState,
            "outbound probe returned an unrelated typed result",
            false,
        )),
    };
    record_probe_phase(&mut state, phase);
}

fn record_probe_phase(state: &mut ProxyInspectionState, phase: ProxyProbePhase) {
    match &mut state.phase {
        ProxyInspectionPhase::Closed => {}
        ProxyInspectionPhase::Open(session) => session.probe = phase,
    }
}

pub fn sync_probe_controls(
    state: Res<ProxyInspectionState>,
    latest: Res<LatestSurfaceSnapshot>,
    locale: Res<UiLocale>,
    sink: Option<Res<CommandSinkHandle>>,
    mut buttons: Query<&mut ButtonDisabled, With<ProbeInspectedProxy>>,
    mut captions: Query<(&mut Text, &mut LocalizedText), ProbeCaptionFilter>,
    mut status: Query<(&mut Text, &mut LocalizedText), ProbeStatusFilter>,
) {
    let capability = probe_capability(&latest.0);
    let state_ready = match &state.phase {
        ProxyInspectionPhase::Closed => false,
        ProxyInspectionPhase::Open(session) => {
            let probe_ready = match (&session.probe, &session.read.phase) {
                (
                    ProxyProbePhase::Idle | ProxyProbePhase::Failed(_),
                    ProxyInspectionReadPhase::Ready,
                ) => true,
                (
                    ProxyProbePhase::Pending(_),
                    ProxyInspectionReadPhase::Loading
                    | ProxyInspectionReadPhase::Ready
                    | ProxyInspectionReadPhase::Failed(_)
                    | ProxyInspectionReadPhase::Unsupported(_),
                )
                | (
                    ProxyProbePhase::Idle | ProxyProbePhase::Failed(_),
                    ProxyInspectionReadPhase::Loading
                    | ProxyInspectionReadPhase::Failed(_)
                    | ProxyInspectionReadPhase::Unsupported(_),
                ) => false,
            };
            let scope_matches = match &session.scope {
                InspectionScope::NoActiveCore => false,
                InspectionScope::Active(scope) => latest.0.core.scope().as_ref() == Some(scope),
            };
            let read_supports_probe = session.read.detail.as_ref().is_some_and(|detail| {
                detail.probe_availability == OutboundProbeAvailability::Supported
            });
            let current_details_support_probe = latest
                .0
                .outbound_details
                .data
                .as_ref()
                .filter(|_| {
                    matches!(
                        latest.0.outbound_details.status,
                        PageStatus::Ready | PageStatus::Empty
                    )
                })
                .filter(|details| latest.0.core.scope().as_ref() == Some(details.scope()))
                .and_then(|details| details.detail(&session.selected))
                .is_some_and(|detail| {
                    detail.probe_availability == OutboundProbeAvailability::Supported
                });
            probe_ready && scope_matches && read_supports_probe && current_details_support_probe
        }
    };
    let enabled = sink.is_some() && capability == ProbeCapability::Available && state_ready;
    for mut button in &mut buttons {
        if button.0 == enabled {
            button.0 = !enabled;
        }
    }
    let caption = LocalizedText::plain(match &state.phase {
        ProxyInspectionPhase::Closed => "modal_speed_test_now",
        ProxyInspectionPhase::Open(session) => match &session.probe {
            ProxyProbePhase::Pending(_) => "core_control_pending",
            ProxyProbePhase::Idle | ProxyProbePhase::Failed(_) => "modal_speed_test_now",
        },
    });
    for (mut text, mut copy) in &mut captions {
        if *copy != caption {
            *copy = caption.clone();
        }
        let value = copy.render(&locale);
        if text.0 != value {
            text.0 = value;
        }
    }
    let message = match &state.phase {
        ProxyInspectionPhase::Closed => String::new(),
        ProxyInspectionPhase::Open(session) => match &session.probe {
            ProxyProbePhase::Failed(failure) => failure.message.clone(),
            ProxyProbePhase::Idle | ProxyProbePhase::Pending(_) => match &session.read.phase {
                ProxyInspectionReadPhase::Loading => {
                    LocalizedText::plain("proxy_inspection_refreshing").render(&locale)
                }
                ProxyInspectionReadPhase::Ready => String::new(),
                ProxyInspectionReadPhase::Failed(failure)
                | ProxyInspectionReadPhase::Unsupported(failure) => failure.message.clone(),
            },
        },
    };
    for (mut text, mut copy) in &mut status {
        let desired = LocalizedText::new("common_message", vec![("message", message.clone())]);
        if *copy != desired {
            *copy = desired;
        }
        let value = copy.render(&locale);
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeCapability {
    Available,
    Unavailable,
}

fn probe_capability(snapshot: &SurfaceSnapshot) -> ProbeCapability {
    use infiltrator_contract::surface_snapshot::PageStatus;

    if snapshot.core_capability_snapshot.status != PageStatus::Ready {
        return ProbeCapability::Unavailable;
    }
    let Some(capabilities) = snapshot.core_capability_snapshot.data.as_ref() else {
        return ProbeCapability::Unavailable;
    };
    let Some(core_scope) = snapshot.core.scope() else {
        return ProbeCapability::Unavailable;
    };
    if capabilities.scope.engine_id != core_scope.identity.engine
        || capabilities.scope.core_version != core_scope.identity.version
    {
        return ProbeCapability::Unavailable;
    }
    match capabilities.resolution(CoreOperation::Outbound(OutboundOperation::Probe)) {
        Some(resolution)
            if matches!(
                &resolution.status,
                CoreCapabilityStatus::Available {
                    access: CapabilityAccess::Supported,
                    ..
                }
            ) =>
        {
            ProbeCapability::Available
        }
        Some(_) | None => ProbeCapability::Unavailable,
    }
}
