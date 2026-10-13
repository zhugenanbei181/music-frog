//! Application-boundary scope fencing for runtime streams.

use futures_util::{StreamExt, stream::unfold};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamTerminal, ScopedRuntimeStream, ScopedRuntimeStreamEvent,
};
use std::sync::Arc;

pub(crate) fn scope_checked<T>(
    gateway: Arc<dyn RuntimeGateway>,
    expected_scope: CoreScope,
    stream: ScopedRuntimeStream<T>,
) -> ScopedRuntimeStream<T>
where
    T: Send + 'static,
{
    unfold(
        (
            stream,
            gateway,
            expected_scope,
            ScopeFenceProgress::Receiving,
        ),
        |(mut stream, gateway, expected_scope, progress)| async move {
            match progress {
                ScopeFenceProgress::Terminated => None,
                ScopeFenceProgress::Receiving => {
                    let Some(event) = stream.next().await else {
                        return Some((
                            terminal_event(
                                expected_scope.clone(),
                                RuntimeStreamTerminal::Cancelled,
                            ),
                            (
                                stream,
                                gateway,
                                expected_scope,
                                ScopeFenceProgress::Terminated,
                            ),
                        ));
                    };

                    let event = match gateway.current_core_scope() {
                        Ok(active_scope)
                            if event.scope == expected_scope && active_scope == expected_scope =>
                        {
                            event
                        }
                        Ok(_) => {
                            return Some((
                                terminal_event(
                                    expected_scope.clone(),
                                    RuntimeStreamTerminal::Failed(Failure::new(
                                        ErrorCode::InvalidState,
                                        "runtime stream event belongs to a retired or foreign Core scope",
                                        false,
                                    )),
                                ),
                                (
                                    stream,
                                    gateway,
                                    expected_scope,
                                    ScopeFenceProgress::Terminated,
                                ),
                            ));
                        }
                        Err(error) => {
                            return Some((
                                terminal_event(
                                    expected_scope.clone(),
                                    RuntimeStreamTerminal::Failed(Failure::from(error)),
                                ),
                                (
                                    stream,
                                    gateway,
                                    expected_scope,
                                    ScopeFenceProgress::Terminated,
                                ),
                            ));
                        }
                    };

                    let next = if matches!(
                        &event.event,
                        RuntimeStreamEvent::Terminal(_)
                    ) {
                        ScopeFenceProgress::Terminated
                    } else {
                        ScopeFenceProgress::Receiving
                    };
                    Some((
                        event,
                        (stream, gateway, expected_scope, next),
                    ))
                }
            }
        },
    )
    .boxed()
}

fn terminal_event<T>(
    scope: CoreScope,
    terminal: RuntimeStreamTerminal,
) -> ScopedRuntimeStreamEvent<T> {
    ScopedRuntimeStreamEvent {
        scope,
        event: RuntimeStreamEvent::Terminal(terminal),
    }
}

#[derive(Clone, Copy)]
enum ScopeFenceProgress {
    Receiving,
    Terminated,
}
