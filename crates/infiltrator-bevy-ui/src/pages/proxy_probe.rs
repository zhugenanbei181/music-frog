//! Inspector latency actions use the shared leaf-probe intent and tracked terminal results.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxy_inspection::ProxyInspectionState;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::proxy_inspection_projection::lookup_inspection;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingProxyProbe {
    pub node: String,
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
    buttons: Query<&ButtonDisabled, With<ProbeInspectedProxy>>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProxyInspectionState>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    let Ok(disabled) = buttons.get(activated.entity) else {
        return;
    };
    if disabled.0 || state.pending.is_some() || !state.read.can_probe() {
        return;
    }
    let Some(node) = state.selected.clone() else {
        return;
    };
    if !latest
        .0
        .pages
        .proxies
        .data
        .as_ref()
        .and_then(|page| lookup_inspection(page, &node))
        .is_some_and(|detail| detail.can_probe)
    {
        return;
    }
    let Some(sink) = sink else { return };
    state.failure = None;
    if let Some(request_id) = sink.submit_tracked(UiCommand::TestProxyNode { node: node.clone() }) {
        state.pending = Some(PendingProxyProbe { node, request_id });
    } else {
        state.failure = Some(Failure::new(
            ErrorCode::NotReady,
            "proxy probe service supplies no terminal acknowledgment",
            true,
        ));
    }
}

pub fn finish_probe(result: On<CommandExecutedEvent>, mut state: ResMut<ProxyInspectionState>) {
    let Some(pending) = state.pending.as_ref() else {
        return;
    };
    if pending.request_id != result.request_id
        || !matches!(&result.command,UiCommand::TestProxyNode {node} if node == &pending.node)
    {
        return;
    }
    let selected = state.selected.as_deref() == Some(pending.node.as_str());
    state.pending = None;
    if selected {
        state.failure = result.result.as_ref().err().cloned();
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
    let enabled = sink.is_some()
        && state.pending.is_none()
        && state.read.can_probe()
        && state
            .selected
            .as_ref()
            .and_then(|name| {
                latest
                    .0
                    .pages
                    .proxies
                    .data
                    .as_ref()
                    .and_then(|page| lookup_inspection(page, name))
            })
            .is_some_and(|detail| detail.can_probe);
    for mut button in &mut buttons {
        if button.0 == enabled {
            button.0 = !enabled;
        }
    }
    let caption = LocalizedText::plain(if state.pending.is_some() {
        "core_control_pending"
    } else {
        "modal_speed_test_now"
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
    let message = state
        .failure
        .as_ref()
        .or(state.read.failure.as_ref())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.read.loading {
                LocalizedText::plain("proxy_inspection_refreshing").render(&locale)
            } else {
                String::new()
            }
        });
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
