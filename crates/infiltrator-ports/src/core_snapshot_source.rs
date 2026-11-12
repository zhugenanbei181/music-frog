//! Narrow read-only port for the live Core snapshot.

use infiltrator_contract::snapshot::CoreSnapshot;

pub trait CoreSnapshotSource: Send + Sync {
    fn core_snapshot(&self) -> CoreSnapshot;
}
