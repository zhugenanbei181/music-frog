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
use infiltrator_application::dns_leak_actions::DnsLeakActionState;
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
pub fn activate(
    event: On<Activate>,
    buttons: Query<(Has<TestDnsLeakButton>, Has<RetryLeakButton>)>,
    sink: Option<Res<CommandSinkHandle>>,
    mut actions: ResMut<LeakActions>,
) {
    let Ok((run, retry)) = buttons.get(event.entity) else {
        return;
    };
    if !run && !retry || retry && !actions.state.can_retry() {
        return;
    }
    let token = match actions.state.begin() {
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
    let Some(token) = actions.state.pending else {
        return;
    };
    if actions.state.finish(token, event.unit_result()) {
        actions.request = None;
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
    let busy =
        actions.state.pending.is_some() || matches!(report.operation, DnsLeakOperation::Running);
    for (mut disabled, run, retry) in &mut buttons {
        if !run && !retry {
            continue;
        }
        let wanted = busy || retry && !actions.state.can_retry();
        if disabled.0 != wanted {
            disabled.0 = wanted;
        }
    }
    let copy = if let Some(failure) = &actions.state.failure {
        failure.message.clone()
    } else if busy {
        LocalizedText::plain("dns_leak_probing").render(&locale)
    } else {
        project_leak(report, locale.code())
            .feedback
            .unwrap_or_default()
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
