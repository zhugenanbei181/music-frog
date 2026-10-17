//! Owner-scoped resource observation and garbage-collection operations.

use async_trait::async_trait;
use infiltrator_contract::resources::ResourceAvailability;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;

#[derive(Clone, Debug, PartialEq)]
pub struct ScopedResourceObservation {
    pub scope: CoreScope,
    pub memory_bytes: ResourceAvailability<u64>,
    pub cpu_percent: ResourceAvailability<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GarbageCollectionOutcome {
    Collected { before_bytes: u64, after_bytes: u64 },
    Unsupported,
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScopedGarbageCollection {
    pub scope: CoreScope,
    pub outcome: GarbageCollectionOutcome,
}

/// Resource adapters must fence every observation and GC readback to the
/// supplied owner scope. A collected result is valid only after memory has
/// been read again from that same scope.
#[async_trait]
pub trait CoreResourcePort: Send + Sync {
    async fn observe(&self, scope: CoreScope) -> Result<ScopedResourceObservation, PortError>;

    async fn collect_garbage(
        &self,
        scope: CoreScope,
        before_bytes: u64,
    ) -> Result<ScopedGarbageCollection, PortError>;
}

#[async_trait]
pub trait CpuResourceObserver: Send + Sync {
    async fn observe_cpu_percent(&self) -> ResourceAvailability<f32>;
}

pub struct UnsupportedCoreResourcePort;

#[async_trait]
impl CoreResourcePort for UnsupportedCoreResourcePort {
    async fn observe(&self, scope: CoreScope) -> Result<ScopedResourceObservation, PortError> {
        Ok(ScopedResourceObservation {
            scope,
            memory_bytes: ResourceAvailability::Unsupported,
            cpu_percent: ResourceAvailability::Unsupported,
        })
    }

    async fn collect_garbage(
        &self,
        scope: CoreScope,
        _before_bytes: u64,
    ) -> Result<ScopedGarbageCollection, PortError> {
        Ok(ScopedGarbageCollection {
            scope,
            outcome: GarbageCollectionOutcome::Unsupported,
        })
    }
}
