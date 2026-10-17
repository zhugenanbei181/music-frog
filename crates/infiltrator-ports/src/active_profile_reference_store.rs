//! Product-wide active-profile reference persistence.
//!
//! The durable owner-switch part of this port is the engine-neutral
//! [`EngineSwitchJournal`]: the product persists `ActiveProfileReferenceState`
//! as its selection and never defines a second switch vocabulary. Only the
//! product-specific reference reads and the first-owner selection lease live
//! here.

use async_trait::async_trait;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity};
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_ports::engine_switch::{
    EngineSwitchJournal, OwnerRecoveryEvidence, OwnerRecoveryOutcome,
};
use lurker_core_ports::error::PortError;

/// Revision-fenced persistence for the unique profile reference that selects
/// which engine provider may own the product's Core.
///
/// The owner-switch journal is the neutral one: `snapshot`, `bind_owner`,
/// `begin_owner_switch`, `recover_owner_switch`,
/// `recover_old_owner_after_terminal` and `recover_candidate_after_terminal`
/// are inherited with `Selection = ActiveProfileReferenceState`. The product
/// only adds reference reads, the first-owner selection lease and the
/// product-wide CAS authorization.
#[async_trait]
pub trait ActiveProfileReferenceStore:
    EngineSwitchJournal<Selection = ActiveProfileReferenceState> + Send + Sync
{
    /// Observe the atomically published reference without creating the
    /// configuration root or repairing owner state. Concurrent mutations may
    /// advance the revision after this snapshot; any write must use a lease or
    /// a revision-fenced compare operation.
    async fn read(&self) -> Result<ActiveProfileReferenceState, PortError>;

    /// Acquire an exclusive selection lease. The returned lease must keep the
    /// exact state stable and block both revision writes and competing
    /// selection leases until it is dropped. Product composition holds it
    /// through backend creation and owner binding.
    async fn acquire_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError>;

    /// Acquire an exclusive lease for constructing a new Core owner. It must
    /// fail promptly while another owner binding for this product remains
    /// live; ordinary reads/profile edits use `acquire_selection_lease`.
    async fn acquire_provider_selection_lease(
        &self,
    ) -> Result<Box<dyn ActiveProfileReferenceLease>, PortError>;

    /// Recover a finalized owner only with a disjoint terminal proof: an exact
    /// never-started snapshot, or the exact persisted managed context plus its
    /// process-exit terminal snapshot. Android callers must also establish VPN
    /// terminal readback before constructing `ManagedChildExited`. Absence of
    /// the OS lock alone is never terminal evidence.
    async fn recover_bound_owner_after_terminal(
        &self,
        _evidence: OwnerRecoveryEvidence,
    ) -> Result<OwnerRecoveryOutcome<ActiveProfileReferenceState>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "active profile owner terminal recovery is unavailable for this store",
        ))
    }

    /// Adopt the exact owner binding this process just finalized through the
    /// neutral switch journal. The neutral `finalize` deliberately returns
    /// only the engine-neutral owner fence; the product owner lease (with
    /// start-context proof and terminal release) is rehydrated here, and must
    /// fail closed unless the persisted owner is `Bound` with the same
    /// identity/generation and this process still holds its kernel
    /// registration.
    async fn adopt_finalized_owner(
        &self,
        _identity: CoreIdentity,
        _generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "active Core owner lease adoption is unavailable for this store",
        ))
    }

    /// Compare the current revision before publication and return the exact
    /// committed readback. A stale revision must fail without writing. The
    /// authorization also fences ordinary offline selection from managed
    /// same-engine activation while an owner is bound.
    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceState, PortError>;
}

#[async_trait]
pub trait ActiveProfileReferenceLease: Send + Sync {
    fn state(&self) -> &ActiveProfileReferenceState;

    /// Convert the exclusive selection lease into a non-blocking owner guard
    /// after the selected Core actor and its capability gate are bound. The
    /// guard prevents cross-engine CAS while allowing same-engine profile
    /// selection to continue.
    async fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError>;
}

#[async_trait]
pub trait ActiveProfileOwnerLease: Send + Sync {
    /// Identity observation captured at binding time. Owner fences compare
    /// its stable engine+instance projection, not its version evidence.
    fn identity(&self) -> &CoreIdentity;
    fn generation(&self) -> CoreGeneration;

    /// Persist the exact managed process session before the host may spawn it.
    /// A terminal recovery must match this context; owners without durable
    /// session proof fail closed.
    async fn record_start_context(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "active Core owner session proof persistence is unavailable for this store",
        ))
    }

    /// Release owner admission only after the CoreApplication has observed
    /// the same engine+instance in `Stopped` with no session token. The bound
    /// generation fence remains exact for the owner lease; terminal version
    /// evidence may have changed after readiness. Dropping an unconfirmed
    /// lease must leave the owner fence fail-closed. Repeating the same exact
    /// terminal release is idempotent for cancellation-safe
    /// callers whose background persistence task may have committed already.
    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError>;
}
