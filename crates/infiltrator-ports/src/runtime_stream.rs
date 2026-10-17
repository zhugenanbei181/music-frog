//! Core-scoped runtime stream contract.

use crate::error::PortError;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use infiltrator_contract::capability::Capability;
use infiltrator_domain::runtime::TrafficData;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeStreamEvent<T> {
    Connecting,
    Connected,
    Item(T),
    Reconnecting(Failure),
    Terminal(RuntimeStreamTerminal),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStreamTerminal {
    Cancelled,
    Failed(Failure),
    Unsupported(Failure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScopedRuntimeStreamEvent<T> {
    pub scope: CoreScope,
    pub event: RuntimeStreamEvent<T>,
}

pub type ScopedRuntimeStream<T> = BoxStream<'static, ScopedRuntimeStreamEvent<T>>;

#[async_trait]
pub trait RuntimeStreamPort: Send + Sync {
    async fn logs(
        &self,
        scope: CoreScope,
        level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError>;

    async fn traffic(
        &self,
        scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<TrafficData>, PortError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedRuntimeStreamPort;

#[async_trait]
impl RuntimeStreamPort for UnsupportedRuntimeStreamPort {
    async fn logs(
        &self,
        _scope: CoreScope,
        _level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        Err(PortError::unsupported(
            Capability::Logs,
            "runtime log streaming is not composed for this Core owner",
        ))
    }

    async fn traffic(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<TrafficData>, PortError> {
        Err(PortError::unsupported(
            Capability::Connections,
            "runtime traffic streaming is not composed for this Core owner",
        ))
    }
}
