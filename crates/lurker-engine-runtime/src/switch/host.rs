//! Host seam for the parts of a switch that are engine- or product-specific.
//!
//! The coordinator owns the journal order; the host owns target validation,
//! candidate construction, old-owner termination and candidate application.
//! Every host method is fallible and must not hide a side effect: a failure
//! after the old owner was asked to stop is reported as an unconfirmed
//! termination, never as a rollback.

use async_trait::async_trait;
use lurker_core_contract::engine_switch::EngineSwitchTarget;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_backend::EngineBackendBundle;
use lurker_core_ports::error::PortError;

/// Operations the coordinator needs from the embedding host.
///
/// `prepare_candidate` must not start a Core, create an owner or reserve any
/// process resource: it only validates the registry-dispatched bundle and
/// reports the stable identity the candidate will bind. `apply_candidate` is
/// called strictly after the previous owner has an exact terminal and the new
/// reservation exists.
#[async_trait]
pub trait EngineSwitchHost<B, S>: Send + Sync
where
    B: EngineBackendBundle,
    S: Clone + Eq + Send + Sync + 'static,
{
    /// Validate the target selection, document and engine before any journal
    /// reservation or process call.
    async fn validate_target(&self, target: &EngineSwitchTarget<S>) -> Result<(), PortError>;

    /// Inspect the already-dispatched bundle and return the exact stable
    /// identity the unprepared candidate would bind. No Core may be started.
    async fn prepare_candidate(
        &self,
        engine_id: &EngineId,
        selection: &S,
        backend: &B,
    ) -> Result<CoreIdentity, PortError>;

    /// Stop the exact old owner and return its exact terminal generation.
    /// The generation must be the lifecycle generation observed terminal; it
    /// may be newer than the bound generation, never older.
    async fn terminate_old_owner(
        &self,
        identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<CoreGeneration, PortError>;

    /// Start and validate the new owner, returning the exact bound identity
    /// and generation. The bundle is consumed by the new owner.
    async fn apply_candidate(
        &self,
        backend: B,
        selection: &S,
    ) -> Result<(CoreIdentity, CoreGeneration), PortError>;

    /// Release a candidate bundle that never started. Called only on the
    /// pre-application rollback path.
    async fn close_candidate(&self, backend: B) -> Result<(), PortError>;
}
