use crate::command_application::CommandApplication;
use crate::command_application::handler::{CommandHandler, ValidatedCoreCommandScope};
use crate::core_capability_test_support::{capability_application, core_scope};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_ports::error::PortError;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_contract::routing_intent::RoutingIntentStatus;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingRoutingPort {
    requests: Mutex<Vec<(CoreScope, RoutingIntent)>>,
}

#[async_trait::async_trait]
impl RoutingIntentPort for RecordingRoutingPort {
    async fn observe(&self, _scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        Ok(RoutingIntent::FollowRules)
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        self.requests
            .lock()
            .expect("routing requests lock")
            .push((scope.clone(), intent.clone()));
        Ok(intent)
    }
}

fn command_application(port: Arc<RecordingRoutingPort>) -> CommandApplication {
    CommandApplication::new()
        .with_core_capabilities(capability_application())
        .with_routing_intent_port(port)
}

#[tokio::test]
async fn typed_intent_uses_the_scoped_core_command_route() {
    let port = Arc::new(RecordingRoutingPort::default());
    let application = command_application(port.clone());
    let scope = core_scope();
    let intent = RoutingIntent::RouteAllThroughProxy;

    let output = application
        .handle_output_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(scope.clone())),
            CommandIntent::SetRoutingIntent {
                intent: intent.clone(),
            },
        )
        .await
        .expect("scoped routing intent returns its verified observation");

    assert_eq!(
        output,
        CommandOutput::RoutingIntentApplied {
            observed: intent.clone()
        }
    );
    assert_eq!(
        port.requests
            .lock()
            .expect("routing requests lock")
            .as_slice(),
        &[(scope, intent)]
    );
}

#[tokio::test]
async fn typed_observation_uses_scoped_read_command_route() {
    let port = Arc::new(RecordingRoutingPort::default());
    let application = command_application(port);
    let scope = core_scope();

    let output = application
        .handle_output_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(scope.clone())),
            CommandIntent::GetRoutingIntent,
        )
        .await
        .expect("typed route observation returns a readback snapshot");

    assert!(matches!(
        output,
        CommandOutput::RoutingIntentObserved { snapshot }
            if snapshot.current == Some(RoutingIntent::FollowRules)
                && snapshot.status == RoutingIntentStatus::Ready
    ));
    assert_eq!(scope, core_scope());
}

#[tokio::test]
async fn missing_scope_or_missing_adapter_stops_before_port_side_effects() {
    let port = Arc::new(RecordingRoutingPort::default());
    let without_adapter =
        CommandApplication::new().with_core_capabilities(capability_application());
    let missing_adapter = without_adapter
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::SetRoutingIntent {
                intent: RoutingIntent::FollowRules,
            },
        )
        .await
        .expect_err("missing adapter is typed unsupported");
    assert_eq!(missing_adapter.code, ErrorCode::Unsupported);

    let application = command_application(port.clone());
    let missing_scope = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(None),
            CommandIntent::SetRoutingIntent {
                intent: RoutingIntent::FollowRules,
            },
        )
        .await
        .expect_err("missing scope is rejected");
    assert_eq!(missing_scope.code, ErrorCode::NotReady);
    assert!(
        port.requests
            .lock()
            .expect("routing requests lock")
            .is_empty()
    );
}
