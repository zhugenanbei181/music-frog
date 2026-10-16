//! Scope- and observation-revision-fenced connection access.

use crate::error::PortError;
use crate::runtime_stream::ScopedRuntimeStream;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::connection::{
    ConnectionContractError, ConnectionId, validate_connection_ids,
};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::outbound::identity::SnapshotRevision;
use infiltrator_domain::runtime::ConnectionSnapshot;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// One immutable controller observation bound to its Core owner and identity
/// set revision. Counters may advance without invalidating a close request;
/// adding/removing/replacing a connection advances `revision`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionObservationSnapshot {
    scope: CoreScope,
    revision: SnapshotRevision,
    snapshot: ConnectionSnapshot,
    connection_ids: Vec<ConnectionId>,
}

impl ConnectionObservationSnapshot {
    pub fn new(
        scope: CoreScope,
        revision: SnapshotRevision,
        snapshot: ConnectionSnapshot,
    ) -> Result<Self, ConnectionContractError> {
        let connection_ids = validate_connection_ids(
            &scope.identity.engine,
            snapshot
                .connections
                .iter()
                .map(|connection| connection.id.clone()),
        )?;
        Ok(Self {
            scope,
            revision,
            snapshot,
            connection_ids,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn revision(&self) -> SnapshotRevision {
        self.revision
    }

    pub fn snapshot(&self) -> &ConnectionSnapshot {
        &self.snapshot
    }

    pub fn connection_ids(&self) -> &[ConnectionId] {
        &self.connection_ids
    }
}

/// The set of connections addressed by a revision-fenced mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionControlTarget {
    One(ConnectionId),
    All,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionControlError {
    ForeignEngine,
    DuplicateConnectionId,
}

impl Display for ConnectionControlError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignEngine => formatter.write_str("connection ID belongs to another engine"),
            Self::DuplicateConnectionId => {
                formatter.write_str("connection receipt contains a duplicate ID")
            }
        }
    }
}

impl Error for ConnectionControlError {}

/// A close request cannot be constructed without an owner scope and the
/// revision from which its connection identities were observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionControlRequest {
    scope: CoreScope,
    expected_revision: SnapshotRevision,
    target: ConnectionControlTarget,
}

impl ConnectionControlRequest {
    pub fn new(
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        target: ConnectionControlTarget,
    ) -> Result<Self, ConnectionControlError> {
        if let ConnectionControlTarget::One(id) = &target
            && id.engine_id() != &scope.identity.engine
        {
            return Err(ConnectionControlError::ForeignEngine);
        }
        Ok(Self {
            scope,
            expected_revision,
            target,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn expected_revision(&self) -> SnapshotRevision {
        self.expected_revision
    }

    pub fn target(&self) -> &ConnectionControlTarget {
        &self.target
    }
}

/// Readback receipt for a successful close operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionControlReceipt {
    scope: CoreScope,
    observed_revision: SnapshotRevision,
    closed: Vec<ConnectionId>,
}

impl ConnectionControlReceipt {
    pub fn new(
        scope: CoreScope,
        observed_revision: SnapshotRevision,
        closed: Vec<ConnectionId>,
    ) -> Result<Self, ConnectionControlError> {
        if closed
            .iter()
            .any(|id| id.engine_id() != &scope.identity.engine)
        {
            return Err(ConnectionControlError::ForeignEngine);
        }
        let mut unique = HashSet::new();
        if closed.iter().any(|id| !unique.insert(id)) {
            return Err(ConnectionControlError::DuplicateConnectionId);
        }
        Ok(Self {
            scope,
            observed_revision,
            closed,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub const fn observed_revision(&self) -> SnapshotRevision {
        self.observed_revision
    }

    pub fn closed(&self) -> &[ConnectionId] {
        &self.closed
    }
}

/// Narrow owner port for live connection observations and mutations.
#[async_trait]
pub trait ConnectionPort: Send + Sync {
    async fn observe(&self, scope: CoreScope) -> Result<ConnectionObservationSnapshot, PortError>;

    async fn stream(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<ConnectionObservationSnapshot>, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "this host does not expose scoped connection streaming",
        ))
    }

    async fn close(
        &self,
        request: ConnectionControlRequest,
    ) -> Result<ConnectionControlReceipt, PortError>;
}

/// Explicit unsupported implementation for hosts which do not own a live
/// controller connection surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedConnectionPort;

#[async_trait]
impl ConnectionPort for UnsupportedConnectionPort {
    async fn observe(&self, _scope: CoreScope) -> Result<ConnectionObservationSnapshot, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "this host does not expose live connection observations",
        ))
    }

    async fn close(
        &self,
        _request: ConnectionControlRequest,
    ) -> Result<ConnectionControlReceipt, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "this host does not expose live connection control",
        ))
    }

    async fn stream(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<ConnectionObservationSnapshot>, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "this host does not expose scoped connection streaming",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, EngineId,
    };
    use infiltrator_domain::runtime::Connection;

    fn scope(engine: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new(engine).expect("engine ID"),
                CoreInstanceId::new("active").expect("instance ID"),
                None,
            ),
            generation: CoreGeneration::new(7),
            session_token: None,
        }
    }

    fn id(engine: &str, value: &str) -> ConnectionId {
        ConnectionId::new(EngineId::new(engine).unwrap(), value).unwrap()
    }

    #[test]
    fn observation_validates_engine_scoped_unique_native_ids() {
        let observed = ConnectionObservationSnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(3),
            ConnectionSnapshot {
                connections: vec![Connection {
                    id: "native-id".to_owned(),
                    ..Connection::default()
                }],
                ..ConnectionSnapshot::default()
            },
        )
        .expect("valid observation");
        assert_eq!(observed.connection_ids(), &[id("mihomo", "native-id")]);

        let duplicate = ConnectionSnapshot {
            connections: vec![
                Connection {
                    id: "same".to_owned(),
                    ..Connection::default()
                },
                Connection {
                    id: "same".to_owned(),
                    ..Connection::default()
                },
            ],
            ..ConnectionSnapshot::default()
        };
        assert_eq!(
            ConnectionObservationSnapshot::new(
                scope("mihomo"),
                SnapshotRevision::new(4),
                duplicate,
            )
            .unwrap_err(),
            ConnectionContractError::DuplicateConnectionId
        );
    }

    #[test]
    fn close_request_rejects_a_foreign_engine_id() {
        let active = scope("mihomo");
        assert_eq!(
            ConnectionControlRequest::new(
                active.clone(),
                SnapshotRevision::new(1),
                ConnectionControlTarget::One(id("sing-box", "foreign")),
            )
            .unwrap_err(),
            ConnectionControlError::ForeignEngine
        );
    }

    #[test]
    fn receipt_cannot_claim_a_foreign_engine_close() {
        assert_eq!(
            ConnectionControlReceipt::new(
                scope("mihomo"),
                SnapshotRevision::new(2),
                vec![id("sing-box", "foreign")],
            )
            .unwrap_err(),
            ConnectionControlError::ForeignEngine
        );
    }

    #[test]
    fn receipt_cannot_claim_the_same_close_twice() {
        let connection = id("mihomo", "same");
        assert_eq!(
            ConnectionControlReceipt::new(
                scope("mihomo"),
                SnapshotRevision::new(2),
                vec![connection.clone(), connection],
            )
            .unwrap_err(),
            ConnectionControlError::DuplicateConnectionId
        );
    }

    #[tokio::test]
    async fn engine_without_connection_support_reports_typed_unsupported() {
        let error = UnsupportedConnectionPort
            .observe(scope("sing-box"))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::Connections,
                ..
            }
        ));
    }
}
