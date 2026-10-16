//! Scope- and revision-fenced connection port for the Mihomo controller.

use crate::client::MihomoClient;
use crate::network_error::network_error;
use crate::runtime_configuration::RuntimeScopeResolver;
use crate::runtime_stream::scope_stream;
use futures_util::{StreamExt, stream};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::identity::SnapshotRevision;
use infiltrator_domain::runtime::ConnectionSnapshot;
use infiltrator_ports::connection::{
    ConnectionControlReceipt, ConnectionControlRequest, ConnectionControlTarget,
    ConnectionObservationSnapshot, ConnectionPort,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamTerminal, ScopedRuntimeStream, ScopedRuntimeStreamEvent,
};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// Revision clock shared by every clone of one Mihomo client. A connection
/// revision changes when its identity set changes, not when byte counters tick.
#[derive(Default)]
pub(crate) enum ConnectionRevisionState {
    #[default]
    Unobserved,
    Observed {
        scope: CoreScope,
        identities: Vec<(String, String)>,
        revision: u64,
    },
}

impl ConnectionRevisionState {
    fn advance(
        &mut self,
        scope: &CoreScope,
        snapshot: &ConnectionSnapshot,
    ) -> Result<SnapshotRevision, PortError> {
        let mut identities = snapshot
            .connections
            .iter()
            .map(|connection| (connection.id.clone(), connection.start.clone()))
            .collect::<Vec<_>>();
        identities.sort_unstable();

        match self {
            Self::Unobserved => {
                *self = Self::Observed {
                    scope: scope.clone(),
                    identities,
                    revision: 1,
                };
                Ok(SnapshotRevision::new(1))
            }
            Self::Observed {
                scope: observed_scope,
                identities: observed_identities,
                revision,
            } => {
                let next_revision = if observed_scope != scope {
                    1
                } else if *observed_identities == identities {
                    *revision
                } else {
                    revision.checked_add(1).ok_or_else(|| {
                        PortError::Failed("connection observation revision exhausted".to_owned())
                    })?
                };
                *observed_scope = scope.clone();
                *observed_identities = identities;
                *revision = next_revision;
                Ok(SnapshotRevision::new(next_revision))
            }
        }
    }
}

#[derive(Clone)]
pub struct MihomoConnectionPort {
    client: MihomoClient,
    scope_resolver: Option<RuntimeScopeResolver>,
    revision_state: Arc<Mutex<ConnectionRevisionState>>,
}

impl MihomoConnectionPort {
    pub fn new(client: MihomoClient, scope_resolver: Option<RuntimeScopeResolver>) -> Self {
        Self {
            revision_state: client.connection_revision_state(),
            client,
            scope_resolver,
        }
    }

    fn current_scope(&self) -> Result<CoreScope, PortError> {
        self.scope_resolver.as_ref().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo connection access has no Core scope resolver",
            )
        })?()
    }

    fn validate_scope(&self, requested: &CoreScope) -> Result<(), PortError> {
        if requested.identity.engine.as_str() != "mihomo" {
            return Err(PortError::unsupported(
                Capability::Connections,
                format!(
                    "Mihomo connection operations cannot serve engine {}",
                    requested.identity.engine.as_str()
                ),
            ));
        }
        if self.current_scope()? == *requested {
            Ok(())
        } else {
            Err(stale_scope())
        }
    }

    async fn read_locked(
        &self,
        scope: CoreScope,
        revision_state: &mut MutexGuard<'_, ConnectionRevisionState>,
    ) -> Result<ConnectionObservationSnapshot, PortError> {
        self.validate_scope(&scope)?;
        let snapshot = self
            .client
            .get_connections()
            .await
            .map(Into::into)
            .map_err(network_error)?;
        self.validate_scope(&scope)?;
        let revision = revision_state.advance(&scope, &snapshot)?;
        ConnectionObservationSnapshot::new(scope, revision, snapshot)
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

#[async_trait::async_trait]
impl ConnectionPort for MihomoConnectionPort {
    async fn observe(&self, scope: CoreScope) -> Result<ConnectionObservationSnapshot, PortError> {
        let mut revision_state = self.revision_state.lock().await;
        self.read_locked(scope, &mut revision_state).await
    }

    async fn stream(
        &self,
        scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<ConnectionObservationSnapshot>, PortError> {
        self.validate_scope(&scope)?;
        let receiver = self
            .client
            .stream_connections_events()
            .await
            .map_err(network_error)?;
        self.validate_scope(&scope)?;
        let resolver = self.scope_resolver.clone().ok_or_else(|| {
            PortError::unsupported(
                Capability::CoreLifecycle,
                "Mihomo connection streaming has no Core scope resolver",
            )
        })?;
        let stream = scope_stream(receiver, scope, resolver, Into::into);
        let revision_state = self.revision_state.clone();
        let observed = stream::unfold(
            (stream, revision_state, ConnectionStreamProgress::Receiving),
            |(mut stream, revision_state, progress)| async move {
                match progress {
                    ConnectionStreamProgress::Terminated => None,
                    ConnectionStreamProgress::Receiving => {
                        let event = stream.next().await?;
                        let scope = event.scope;
                        let event: RuntimeStreamEvent<ConnectionObservationSnapshot> =
                            match event.event {
                                RuntimeStreamEvent::Item(snapshot) => {
                                    let mut state = revision_state.lock().await;
                                    match state.advance(&scope, &snapshot).and_then(|revision| {
                                        ConnectionObservationSnapshot::new(
                                            scope.clone(),
                                            revision,
                                            snapshot,
                                        )
                                        .map_err(|error| PortError::Failed(error.to_string()))
                                    }) {
                                        Ok(observation) => RuntimeStreamEvent::Item(observation),
                                        Err(error) => RuntimeStreamEvent::Terminal(
                                            RuntimeStreamTerminal::Failed(Failure::from(error)),
                                        ),
                                    }
                                }
                                RuntimeStreamEvent::Connecting => RuntimeStreamEvent::Connecting,
                                RuntimeStreamEvent::Connected => RuntimeStreamEvent::Connected,
                                RuntimeStreamEvent::Reconnecting(failure) => {
                                    RuntimeStreamEvent::Reconnecting(failure)
                                }
                                RuntimeStreamEvent::Terminal(terminal) => {
                                    RuntimeStreamEvent::Terminal(terminal)
                                }
                            };
                        let next_progress = if matches!(&event, RuntimeStreamEvent::Terminal(_)) {
                            ConnectionStreamProgress::Terminated
                        } else {
                            ConnectionStreamProgress::Receiving
                        };
                        Some((
                            ScopedRuntimeStreamEvent { scope, event },
                            (stream, revision_state, next_progress),
                        ))
                    }
                }
            },
        );
        Ok(Box::pin(observed))
    }

    async fn close(
        &self,
        request: ConnectionControlRequest,
    ) -> Result<ConnectionControlReceipt, PortError> {
        let scope = request.scope().clone();
        let mut revision_state = self.revision_state.lock().await;
        let observed = self.read_locked(scope.clone(), &mut revision_state).await?;
        if observed.revision() != request.expected_revision() {
            return Err(stale_revision());
        }

        let closing = match request.target() {
            ConnectionControlTarget::One(id) => vec![id.clone()],
            ConnectionControlTarget::All => observed.connection_ids().to_vec(),
        };
        if let Some(missing) = closing
            .iter()
            .find(|id| !observed.connection_ids().contains(id))
        {
            return Err(PortError::NotFound(format!(
                "connection `{}` is not present in the accepted observation",
                missing.value()
            )));
        }

        self.validate_scope(&scope)?;
        match request.target() {
            ConnectionControlTarget::One(id) => self
                .client
                .close_connection(id.value())
                .await
                .map_err(network_error)?,
            ConnectionControlTarget::All if !closing.is_empty() => self
                .client
                .close_all_connections()
                .await
                .map_err(network_error)?,
            ConnectionControlTarget::All => {
                return ConnectionControlReceipt::new(scope, observed.revision(), closing)
                    .map_err(|error| PortError::Failed(error.to_string()));
            }
        }

        self.validate_scope(&scope)?;
        let readback = self.read_locked(scope.clone(), &mut revision_state).await?;
        if let Some(remaining) = closing
            .iter()
            .find(|id| readback.connection_ids().contains(id))
        {
            return Err(PortError::Failed(format!(
                "Mihomo did not remove connection `{}` in readback",
                remaining.value()
            )));
        }
        ConnectionControlReceipt::new(scope, readback.revision(), closing)
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

#[derive(Clone, Copy)]
enum ConnectionStreamProgress {
    Receiving,
    Terminated,
}

fn stale_scope() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        "connection request belongs to a stale or foreign Core scope",
        false,
    ))
}

fn stale_revision() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        "connection identity set changed after it was observed",
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use infiltrator_contract::connection::ConnectionId;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, EngineId,
    };
    use infiltrator_domain::runtime::Connection;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    fn scope(generation: u64) -> CoreScope {
        scope_for("mihomo", generation)
    }

    fn scope_for(engine: &str, generation: u64) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new(engine).unwrap(),
                CoreInstanceId::new("active").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(generation),
            session_token: None,
        }
    }

    fn snapshot(ids: &[&str]) -> ConnectionSnapshot {
        ConnectionSnapshot {
            connections: ids
                .iter()
                .map(|id| Connection {
                    id: (*id).to_owned(),
                    ..Connection::default()
                })
                .collect(),
            ..ConnectionSnapshot::default()
        }
    }

    fn scope_resolver(active: CoreScope) -> RuntimeScopeResolver {
        Arc::new(move || Ok(active.clone()))
    }

    #[test]
    fn revision_advances_only_when_scope_or_connection_identity_set_changes() {
        let mut state = ConnectionRevisionState::default();
        let active_scope = scope(1);
        assert_eq!(
            state
                .advance(&active_scope, &snapshot(&["a"]))
                .unwrap()
                .value(),
            1
        );
        let counter_only_change = ConnectionSnapshot {
            upload_total: 10,
            ..snapshot(&["a"])
        };
        assert_eq!(
            state
                .advance(&active_scope, &counter_only_change)
                .unwrap()
                .value(),
            1
        );
        assert_eq!(
            state
                .advance(&active_scope, &snapshot(&["a", "b"]))
                .unwrap()
                .value(),
            2
        );
        assert_eq!(
            state
                .advance(&scope(2), &snapshot(&["a", "b"]))
                .unwrap()
                .value(),
            1
        );
    }

    #[test]
    fn stale_and_current_scopes_are_distinct_by_generation() {
        assert_eq!(stale_scope().error_code(), ErrorCode::InvalidState);
        assert_eq!(stale_revision().error_code(), ErrorCode::InvalidState);
        assert_ne!(scope(1), scope(2));
    }

    #[tokio::test]
    async fn observation_rejects_a_retired_core_scope_before_controller_io() {
        let client = MihomoClient::new("http://127.0.0.1:1", None).unwrap();
        let port = MihomoConnectionPort::new(client, Some(scope_resolver(scope(2))));

        let error = port.observe(scope(1)).await.unwrap_err();

        assert_eq!(error.error_code(), ErrorCode::InvalidState);
    }

    #[tokio::test]
    async fn connection_port_rejects_a_foreign_engine_before_controller_io() {
        let client = MihomoClient::new("http://127.0.0.1:1", None).unwrap();
        let port = MihomoConnectionPort::new(client, Some(scope_resolver(scope(1))));

        let error = port.observe(scope_for("sing-box", 1)).await.unwrap_err();

        assert_eq!(error.error_code(), ErrorCode::Unsupported);
    }

    #[tokio::test]
    async fn connection_stream_terminates_when_its_core_generation_retires() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (release_item, item_released) = oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            item_released.await.unwrap();
            websocket
                .send(Message::text(
                    serde_json::json!({
                        "downloadTotal": 11,
                        "uploadTotal": 22,
                        "connections": [{"id": "connection-1", "metadata": {}}]
                    })
                    .to_string(),
                ))
                .await
                .unwrap();
        });

        let active_scope = Arc::new(Mutex::new(scope(1)));
        let resolver_scope = active_scope.clone();
        let resolver: RuntimeScopeResolver =
            Arc::new(move || Ok(resolver_scope.lock().unwrap().clone()));
        let client = MihomoClient::new(&format!("http://{address}"), None).unwrap();
        let port = MihomoConnectionPort::new(client, Some(resolver));
        let mut stream = port.stream(scope(1)).await.unwrap();

        for expected in [
            RuntimeStreamEvent::Connecting,
            RuntimeStreamEvent::Connected,
        ] {
            let event = timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(event.scope, scope(1));
            assert_eq!(event.event, expected);
        }

        *active_scope.lock().unwrap() = scope(2);
        release_item.send(()).unwrap();
        let event = timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap();
        let RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)) = event.event
        else {
            panic!("retired connection streams must end with a typed scope failure");
        };
        assert_eq!(failure.code, ErrorCode::InvalidState);
        assert!(!failure.retryable);
        assert!(
            timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn close_rejects_a_stale_identity_revision_without_mutating_the_controller() {
        let mut server = mockito::Server::new_async().await;
        let observed = server
            .mock("GET", "/connections")
            .with_status(200)
            .with_body(r#"{"connections":[{"id":"c1","metadata":{},"rule":"MATCH"}]}"#)
            .expect(1)
            .create_async()
            .await;
        let active_scope = scope(1);
        let client = MihomoClient::new(&server.url(), None).unwrap();
        let port = MihomoConnectionPort::new(client, Some(scope_resolver(active_scope.clone())));
        let request = ConnectionControlRequest::new(
            active_scope,
            SnapshotRevision::new(0),
            ConnectionControlTarget::One(
                ConnectionId::new(EngineId::new("mihomo").unwrap(), "c1").unwrap(),
            ),
        )
        .unwrap();

        let error = port.close(request).await.unwrap_err();

        assert_eq!(error.error_code(), ErrorCode::InvalidState);
        observed.assert_async().await;
    }

    #[tokio::test]
    async fn close_uses_scoped_identity_and_requires_controller_readback() {
        let mut server = mockito::Server::new_async().await;
        let before = server
            .mock("GET", "/connections")
            .with_status(200)
            .with_body(r#"{"connections":[{"id":"c1","metadata":{},"rule":"MATCH"}]}"#)
            .expect(2)
            .create_async()
            .await;
        let closed = server
            .mock("DELETE", "/connections/c1")
            .with_status(204)
            .expect(1)
            .create_async()
            .await;
        let after = server
            .mock("GET", "/connections")
            .with_status(200)
            .with_body(r#"{"connections":[]}"#)
            .expect(1)
            .create_async()
            .await;
        let active_scope = scope(1);
        let client = MihomoClient::new(&server.url(), None).unwrap();
        let port = MihomoConnectionPort::new(client, Some(scope_resolver(active_scope.clone())));
        let observed = port.observe(active_scope.clone()).await.unwrap();
        let id = observed.connection_ids()[0].clone();
        let request = ConnectionControlRequest::new(
            active_scope.clone(),
            observed.revision(),
            ConnectionControlTarget::One(id.clone()),
        )
        .unwrap();

        let receipt = port.close(request).await.unwrap();

        assert_eq!(receipt.scope(), &active_scope);
        assert_eq!(receipt.closed(), &[id]);
        assert_eq!(receipt.observed_revision().value(), 2);
        before.assert_async().await;
        closed.assert_async().await;
        after.assert_async().await;
    }

    #[allow(dead_code)]
    fn _typed_id_is_engine_scoped(value: &str) -> ConnectionId {
        ConnectionId::new(EngineId::new("mihomo").unwrap(), value).unwrap()
    }
}
