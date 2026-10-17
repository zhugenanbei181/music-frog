//! test-intent: behavior
use crate::state::AppState;
use crate::test_mounts::command_harness::test_command_application_for;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::routing_intent_issue::controls;
use crate::view_root::interaction_regions::InteractionRegion;
use futures_util::StreamExt;
use iced::advanced::Shell;
use iced::advanced::clipboard::Null;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{Id, Operation, Tree};
use iced::mouse::{Button, Cursor, Event};
use iced::{Element, Point, Rectangle, Size, Task, Theme};
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_capabilities::{CoreOperation, ProxyControlOperation};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_test_support::routing_intent_fixtures::RoutingIntentScenarioController;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;
use tokio::runtime::Builder;

fn composed() -> (AppState, Arc<RoutingIntentScenarioController>) {
    let (mut state, _) = AppState::new();
    let controller = Arc::new(RoutingIntentScenarioController::default());
    let runtime = tokio_application_runtime().unwrap();
    let application = CoreApplication::new_with_overview(
        controller.clone(),
        controller.clone(),
        controller.clone(),
        runtime.clone(),
    );
    let command_application = test_command_application_for(
        [CoreOperation::ProxyControl(
            ProxyControlOperation::SetRoutingIntent,
        )],
        [],
    )
    .with_routing_intent_port(controller.clone());
    application
        .install_core_capability_application(command_application.core_capability_application())
        .expect("routing capability installs before use");
    application.install_command_handler(Arc::new(command_application));
    let ready = application.clone();
    runtime.block_on(Box::pin(async move {
        assert!(ready.adopt_if_running().await.unwrap());
    }));
    state.attach_product_application(application.clone());
    assert!(state.apply_shared_surface_snapshot(
        controller.snapshot(application.snapshot(), SurfaceKind::IcedDesktop)
    ));
    (state, controller)
}
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("real application command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("routing intent terminal message");
            };
            assert!(stream.next().await.is_none());
            message
        })
}
struct Region {
    id: Id,
    bounds: Option<Rectangle>,
}
impl Operation<()> for Region {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<()>)) {
        visit(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.id) {
            self.bounds = Some(bounds);
        }
    }
}
fn click(state: &AppState, region: InteractionRegion) -> Vec<Message> {
    let mut element: Element<'_, Message, Theme, ()> = controls(state);
    let mut tree = Tree::new(element.as_widget());
    let size = Size::new(600.0, 80.0);
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &(), &Limits::new(Size::ZERO, size));
    let layout = Layout::new(&node);
    let mut target = Region {
        id: region.id(),
        bounds: None,
    };
    element
        .as_widget_mut()
        .operate(&mut tree, layout, &(), &mut target);
    let bounds = target.bounds.expect("actual native button bounds");
    assert!(bounds.width > 0.0 && bounds.height > 0.0);
    let point = Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    );
    let mut messages = vec![];
    for event in [
        Event::ButtonPressed(Button::Left),
        Event::ButtonReleased(Button::Left),
    ] {
        element.as_widget_mut().update(
            &mut tree,
            &iced::Event::Mouse(event),
            layout,
            Cursor::Available(point),
            &(),
            &mut Null,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(size),
        );
    }
    messages
}

#[test]
fn native_retry_and_dismiss_execute_shared_commands_without_desktop_runtime_and_keep_actual_lifecycle()
 {
    let (mut state, controller) = composed();
    assert!(state.runtime.runtime.is_none());
    let lifecycle = state.runtime.core_lifecycle.clone();
    let task = state.update(Message::SetRoutingIntent(
        RoutingIntent::RouteAllThroughProxy,
    ));
    assert!(
        state
            .runtime
            .routing_intent_actions
            .pending_change()
            .is_some()
    );
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::FollowRules)
    );
    assert_eq!(
        state
            .update(Message::SetRoutingIntent(RoutingIntent::BypassProxy))
            .units(),
        0
    );
    for patch in [
        Message::SetIpv6Routing(true),
        Message::SetTunEnabled(true),
        Message::SetTunAutoRoute(true),
        Message::SetTunStrictRoute(true),
        Message::SetTunStack("system".into()),
        Message::SetSnifferEnabled(true),
    ] {
        assert_eq!(state.update(patch).units(), 0);
    }
    let _ = state.update(terminal(task));
    assert_eq!(controller.requests(), [RoutingIntent::RouteAllThroughProxy]);
    assert_eq!(state.runtime.core_lifecycle, lifecycle);
    assert_eq!(
        state.runtime.routing_intent_actions.failure().unwrap().code,
        ErrorCode::Network
    );
    let mut healthy = controller.snapshot(
        state.commands.as_ref().unwrap().snapshot(),
        SurfaceKind::IcedDesktop,
    );
    healthy.revision = state
        .runtime
        .routing_intent_actions
        .next_observation_revision();
    assert!(state.apply_shared_surface_snapshot(healthy));
    assert!(state.runtime.routing_intent_actions.failure().is_some());
    controller.accept_requests();
    let messages = click(&state, InteractionRegion::RoutingIntentRetry);
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::RetryRoutingIntent));
    let task = state.update(messages.into_iter().next().unwrap());
    let _ = state.update(terminal(task));
    assert_eq!(
        controller.requests(),
        [
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::RouteAllThroughProxy
        ]
    );
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert!(state.runtime.routing_intent_actions.failure().is_none());
    assert_eq!(state.runtime.core_lifecycle, lifecycle);
    controller.reject_requests(Failure::unsupported("routing intent control unavailable"));
    let task = state.update(Message::SetRoutingIntent(RoutingIntent::BypassProxy));
    let _ = state.update(terminal(task));
    assert_eq!(
        state.runtime.routing_intent_actions.failure().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(click(&state, InteractionRegion::RoutingIntentRetry).is_empty());
    let messages = click(&state, InteractionRegion::RoutingIntentDismiss);
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::DismissRoutingIntentFailure));
    let _ = state.update(messages.into_iter().next().unwrap());
    assert!(state.runtime.routing_intent_actions.failure().is_none());
    assert_eq!(
        controller.requests(),
        [
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::BypassProxy
        ]
    );
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    controller.reject_requests(Failure::new(
        ErrorCode::Authentication,
        "controller token rejected",
        false,
    ));
    let task = state.update(Message::SetRoutingIntent(RoutingIntent::BypassProxy));
    let _ = state.update(terminal(task));
    let messages = click(&state, InteractionRegion::RoutingIntentSettings);
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::Navigate(Route::Settings)));
    let _ = state.update(messages.into_iter().next().unwrap());
    assert_eq!(state.shell.current_route, Route::Settings);
    assert_eq!(
        state.runtime.routing_intent_actions.failure().unwrap().code,
        ErrorCode::Authentication
    );
    assert_eq!(
        controller.requests(),
        [
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::BypassProxy,
            RoutingIntent::BypassProxy
        ]
    );
}
