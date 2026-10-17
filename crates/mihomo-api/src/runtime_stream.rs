//! Scope-fenced Mihomo log and traffic stream adapter.

use crate::runtime_configuration::RuntimeScopeResolver;
use crate::runtime_wire::traffic_data;
use async_trait::async_trait;
use futures_util::{StreamExt, stream::unfold};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamPort, RuntimeStreamTerminal, ScopedRuntimeStream,
    ScopedRuntimeStreamEvent,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;
use mihomo_driver::api::client::{MihomoClient, StreamEvent, StreamTerminal};
use mihomo_driver::network_error::network_error;
use std::convert::identity;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Clone)]
pub struct MihomoRuntimeStreamPort {
    client: MihomoClient,
    scope_resolver: Option<RuntimeScopeResolver>,
}

impl MihomoRuntimeStreamPort {
    pub fn new(client: MihomoClient, scope_resolver: Option<RuntimeScopeResolver>) -> Self {
        Self {
            client,
            scope_resolver,
        }
    }

    fn validate_scope(
        &self,
        requested: &CoreScope,
        capability: Capability,
    ) -> Result<(), PortError> {
        if requested.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                capability,
                format!(
                    "Mihomo runtime streams cannot serve engine {}",
                    requested.identity.engine.as_str()
                ),
            ));
        }
        let resolver = self.scope_resolver.as_ref().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo stream access has no Core scope resolver",
            )
        })?;
        match resolver()? {
            current if current == *requested => Ok(()),
            _ => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "Mihomo stream request belongs to a retired Core scope",
                false,
            ))),
        }
    }
}

#[async_trait]
impl RuntimeStreamPort for MihomoRuntimeStreamPort {
    async fn logs(
        &self,
        scope: CoreScope,
        level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        self.validate_scope(&scope, Capability::Logs)?;
        let receiver = self
            .client
            .stream_logs_events(level.as_deref())
            .await
            .map_err(network_error)?;
        self.validate_scope(&scope, Capability::Logs)?;
        let resolver = self.scope_resolver.clone().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo stream access has no Core scope resolver",
            )
        })?;
        Ok(scope_stream(receiver, scope, resolver, identity))
    }

    async fn traffic(
        &self,
        scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<TrafficData>, PortError> {
        self.validate_scope(&scope, Capability::Connections)?;
        let receiver = self
            .client
            .stream_traffic_events()
            .await
            .map_err(network_error)?;
        self.validate_scope(&scope, Capability::Connections)?;
        let resolver = self.scope_resolver.clone().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo stream access has no Core scope resolver",
            )
        })?;
        Ok(scope_stream(receiver, scope, resolver, traffic_data))
    }
}

fn map_terminal(terminal: StreamTerminal) -> RuntimeStreamTerminal {
    match terminal {
        StreamTerminal::Cancelled => RuntimeStreamTerminal::Cancelled,
        StreamTerminal::Failed(failure) => RuntimeStreamTerminal::Failed(failure),
        StreamTerminal::Unsupported(failure) => RuntimeStreamTerminal::Unsupported(failure),
    }
}

pub(crate) fn scope_stream<T, U, F>(
    receiver: UnboundedReceiver<StreamEvent<T>>,
    scope: CoreScope,
    resolver: RuntimeScopeResolver,
    map_item: F,
) -> ScopedRuntimeStream<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: FnMut(T) -> U + Send + 'static,
{
    unfold(
        ScopeStreamState {
            receiver,
            scope,
            resolver,
            map_item,
            progress: StreamMapProgress::Receiving,
        },
        |state| async move {
            let ScopeStreamState {
                mut receiver,
                scope,
                resolver,
                mut map_item,
                progress,
            } = state;
            match progress {
                StreamMapProgress::Terminated => None,
                StreamMapProgress::Receiving => {
                    let Some(source_event) = receiver.recv().await else {
                        return Some((
                            ScopedRuntimeStreamEvent {
                                scope: scope.clone(),
                                event: RuntimeStreamEvent::Terminal(
                                    RuntimeStreamTerminal::Cancelled,
                                ),
                            },
                            ScopeStreamState {
                                receiver,
                                scope,
                                resolver,
                                map_item,
                                progress: StreamMapProgress::Terminated,
                            },
                        ));
                    };

                    let active_scope = resolver();
                    match active_scope {
                        Ok(active_scope) if active_scope == scope => {
                            let event = match source_event {
                                StreamEvent::Connecting => RuntimeStreamEvent::Connecting,
                                StreamEvent::Connected => RuntimeStreamEvent::Connected,
                                StreamEvent::Item(item) => RuntimeStreamEvent::Item(map_item(item)),
                                StreamEvent::Reconnecting(failure) => {
                                    RuntimeStreamEvent::Reconnecting(failure)
                                }
                                StreamEvent::Terminal(terminal) => {
                                    RuntimeStreamEvent::Terminal(map_terminal(terminal))
                                }
                            };
                            let progress = if matches!(&event, RuntimeStreamEvent::Terminal(_)) {
                                StreamMapProgress::Terminated
                            } else {
                                StreamMapProgress::Receiving
                            };
                            Some((
                                ScopedRuntimeStreamEvent {
                                    scope: scope.clone(),
                                    event,
                                },
                                ScopeStreamState {
                                    receiver,
                                    scope,
                                    resolver,
                                    map_item,
                                    progress,
                                },
                            ))
                        }
                        Ok(_) => {
                            let transition = scope_failure_event(
                                ScopeStreamState {
                                    receiver,
                                    scope,
                                    resolver,
                                    map_item,
                                    progress: StreamMapProgress::Terminated,
                                },
                                Failure::new(
                                    ErrorCode::InvalidState,
                                    "Mihomo stream event arrived after its Core scope retired",
                                    false,
                                ),
                            );
                            Some((transition.event, transition.next))
                        }
                        Err(error) => {
                            let transition = scope_failure_event(
                                ScopeStreamState {
                                    receiver,
                                    scope,
                                    resolver,
                                    map_item,
                                    progress: StreamMapProgress::Terminated,
                                },
                                Failure::from(error),
                            );
                            Some((transition.event, transition.next))
                        }
                    }
                }
            }
        },
    )
    .boxed()
}

fn scope_failure_event<T, U, F>(
    state: ScopeStreamState<T, F>,
    failure: Failure,
) -> ScopeStreamTransition<T, U, F>
where
    F: FnMut(T) -> U + Send + 'static,
{
    let ScopeStreamState {
        receiver,
        scope,
        resolver,
        map_item,
        ..
    } = state;
    ScopeStreamTransition {
        event: ScopedRuntimeStreamEvent {
            scope: scope.clone(),
            event: RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)),
        },
        next: ScopeStreamState {
            receiver,
            scope,
            resolver,
            map_item,
            progress: StreamMapProgress::Terminated,
        },
    }
}

struct ScopeStreamTransition<T, U, F> {
    event: ScopedRuntimeStreamEvent<U>,
    next: ScopeStreamState<T, F>,
}

struct ScopeStreamState<T, F> {
    receiver: UnboundedReceiver<StreamEvent<T>>,
    scope: CoreScope,
    resolver: RuntimeScopeResolver,
    map_item: F,
    progress: StreamMapProgress,
}

#[derive(Clone, Copy)]
enum StreamMapProgress {
    Receiving,
    Terminated,
}

#[cfg(test)]
#[path = "runtime_stream_tests.rs"]
mod tests;
