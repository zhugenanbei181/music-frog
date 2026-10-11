//! Native cross-source probe feedback remains on its complete diagnostic surface.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::dns::TestDnsLeakButton;
use crate::pages::dns_leak::LeakFeedback;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::dns_leak_actions::{
    DnsLeakActionPhase, DnsLeakActionState, DnsLeakFinish,
};
use infiltrator_application::dns_leak_projection::project_leak;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Resource, Default)]
pub struct LeakActions {
    pub state: DnsLeakActionState,
    request: Option<RequestId>,
}
#[derive(Component, Clone, Default)]
#[require(Button, ButtonDisabled)]
pub struct RetryLeakButton;
#[derive(Clone, Copy)]
enum ProbeAdmission {
    Start,
    Retry,
}
pub fn activate(
    event: On<Activate>,
    buttons: Query<(Option<&TestDnsLeakButton>, Option<&RetryLeakButton>)>,
    sink: Option<Res<CommandSinkHandle>>,
    mut actions: ResMut<LeakActions>,
) {
    let Ok((run, retry)) = buttons.get(event.entity) else {
        return;
    };
    let admission = match (run, retry) {
        (Some(_), None) => ProbeAdmission::Start,
        (None, Some(_)) => ProbeAdmission::Retry,
        (Some(_), Some(_)) | (None, None) => return,
    };
    let token = match match admission {
        ProbeAdmission::Start => actions.state.begin(),
        ProbeAdmission::Retry => actions.state.begin_retry(),
    } {
        Ok(token) => token,
        Err(_) => return,
    };
    if let Some(request) = sink.and_then(|sink| sink.submit_tracked(UiCommand::TestDnsLeak)) {
        actions.request = Some(request);
    } else {
        actions.state.finish(
            token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "DNS cross-source command service is unavailable",
                true,
            )),
        );
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut actions: ResMut<LeakActions>) {
    if actions.request != Some(event.request_id) || event.command != UiCommand::TestDnsLeak {
        return;
    }
    let token = match &actions.state.phase {
        DnsLeakActionPhase::Pending { token } => *token,
        DnsLeakActionPhase::Idle | DnsLeakActionPhase::Failed { .. } => return,
    };
    match actions.state.finish(token, event.unit_result()) {
        DnsLeakFinish::Stale => {}
        DnsLeakFinish::Applied => actions.request = None,
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct FeedbackText {
    text: &'static mut Text,
    node: &'static mut Node,
}
pub fn sync_controls(
    actions: Res<LeakActions>,
    latest: Res<LatestSurfaceSnapshot>,
    locale: Res<UiLocale>,
    mut buttons: Query<(
        &mut ButtonDisabled,
        Has<TestDnsLeakButton>,
        Has<RetryLeakButton>,
    )>,
    mut feedback: Query<FeedbackText, With<LeakFeedback>>,
) {
    let report = &latest.0.dns_leak;
    let busy = matches!(&actions.state.phase, DnsLeakActionPhase::Pending { .. })
        || matches!(report.operation, DnsLeakOperation::Running);
    for (mut disabled, run, retry) in &mut buttons {
        if !run && !retry {
            continue;
        }
        let wanted = busy || retry && !actions.state.can_retry();
        if disabled.0 != wanted {
            disabled.0 = wanted;
        }
    }
    let copy = match &actions.state.phase {
        DnsLeakActionPhase::Failed { failure } => failure.message.clone(),
        DnsLeakActionPhase::Pending { .. } => {
            LocalizedText::plain("dns_leak_probing").render(&locale)
        }
        DnsLeakActionPhase::Idle => project_leak(report, locale.code())
            .feedback
            .unwrap_or_default(),
    };
    for mut feedback in &mut feedback {
        if feedback.text.0 != copy {
            feedback.text.0 = copy.clone();
        }
        let display = if copy.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if feedback.node.display != display {
            feedback.node.display = display;
        }
    }
}
