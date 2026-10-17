//! Application-boundary scope fencing for runtime streams.

use crate::runtime_scope::active_scope;
use futures_util::{StreamExt, stream::unfold};
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamTerminal, ScopedRuntimeStream, ScopedRuntimeStreamEvent,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::core_scope::CoreScopeSource;
use std::sync::Arc;

pub(crate) fn scope_checked<T>(
    scope_source: Arc<dyn CoreScopeSource>,
    expected_scope: CoreScope,
    stream: ScopedRuntimeStream<T>,
) -> ScopedRuntimeStream<T>
where
    T: Send + 'static,
{
    unfold(
        (
            stream,
            scope_source,
            expected_scope,
            ScopeFenceProgress::Receiving,
        ),
        |(mut stream, scope_source, expected_scope, progress)| async move {
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
                                scope_source,
                                expected_scope,
                                ScopeFenceProgress::Terminated,
                            ),
                        ));
                    };

                    let event = match active_scope(scope_source.as_ref()) {
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
                                    scope_source,
                                    expected_scope,
                                    ScopeFenceProgress::Terminated,
                                ),
                            ));
                        }
                        Err(failure) => {
                            return Some((
                                terminal_event(
                                    expected_scope.clone(),
                                    RuntimeStreamTerminal::Failed(failure),
                                ),
                                (
                                    stream,
                                    scope_source,
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
                        (stream, scope_source, expected_scope, next),
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
