//! Scope-fenced Mihomo log and traffic stream adapter.

use crate::client::{MihomoClient, StreamEvent};
use crate::network_error::network_error;
use crate::runtime_configuration::RuntimeScopeResolver;
use async_trait::async_trait;
use futures_util::{StreamExt, stream::unfold};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamPort, RuntimeStreamTerminal, ScopedRuntimeStream,
    ScopedRuntimeStreamEvent,
};
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
        Ok(scope_stream(
            receiver,
            scope,
            resolver,
            std::convert::identity,
        ))
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
        Ok(scope_stream(receiver, scope, resolver, Into::into))
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
        (
            receiver,
            scope,
            resolver,
            map_item,
            StreamMapProgress::Receiving,
        ),
        |(mut receiver, scope, resolver, mut map_item, progress)| async move {
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
                            (
                                receiver,
                                scope,
                                resolver,
                                map_item,
                                StreamMapProgress::Terminated,
                            ),
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
                                    RuntimeStreamEvent::Terminal(terminal)
                                }
                            };
                            let next = if matches!(&event, RuntimeStreamEvent::Terminal(_)) {
                                StreamMapProgress::Terminated
                            } else {
                                StreamMapProgress::Receiving
                            };
                            Some((
                                ScopedRuntimeStreamEvent {
                                    scope: scope.clone(),
                                    event,
                                },
                                (receiver, scope, resolver, map_item, next),
                            ))
                        }
                        Ok(_) => Some(scope_failure_event(
                            receiver,
                            scope,
                            resolver,
                            map_item,
                            Failure::new(
                                ErrorCode::InvalidState,
                                "Mihomo stream event arrived after its Core scope retired",
                                false,
                            ),
                        )),
                        Err(error) => Some(scope_failure_event(
                            receiver,
                            scope,
                            resolver,
                            map_item,
                            Failure::from(error),
                        )),
                    }
                }
            }
        },
    )
    .boxed()
}

fn scope_failure_event<T, U, F>(
    receiver: UnboundedReceiver<StreamEvent<T>>,
    scope: CoreScope,
    resolver: RuntimeScopeResolver,
    map_item: F,
    failure: Failure,
) -> (
    ScopedRuntimeStreamEvent<U>,
    (
        UnboundedReceiver<StreamEvent<T>>,
        CoreScope,
        RuntimeScopeResolver,
        F,
        StreamMapProgress,
    ),
)
where
    F: FnMut(T) -> U + Send + 'static,
{
    (
        ScopedRuntimeStreamEvent {
            scope: scope.clone(),
            event: RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)),
        },
        (
            receiver,
            scope,
            resolver,
            map_item,
            StreamMapProgress::Terminated,
        ),
    )
}

#[derive(Clone, Copy)]
enum StreamMapProgress {
    Receiving,
    Terminated,
}
