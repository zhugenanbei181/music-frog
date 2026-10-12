//! Product-wide active-profile reference persistence.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;

/// Durable owner-switch progress. Each variant is a legal transaction stage;
/// the store persists it so a process restart cannot erase an unfinished
/// cross-process reservation. In every variant, `owner_generation` is the
/// stable generation recorded when the old owner was bound; `terminal_generation`
/// is a later lifecycle snapshot fence and must not replace it. Owner identity
/// is the stable engine+instance projection of `CoreIdentity`; runtime version
/// evidence is intentionally allowed to change after readiness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileOwnerSwitchState {
    ClosingOld {
        previous_reference: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        /// Stable generation recorded by `into_owner_binding`, not the
        /// current/terminal lifecycle generation from the Core snapshot.
        owner_generation: CoreGeneration,
    },
    OldStopped {
        previous_reference: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
        terminal_generation: CoreGeneration,
    },
    ReferenceChanged {
        previous_reference: ActiveProfileReferenceState,
        selection: ActiveProfileReferenceState,
        owner_identity: CoreIdentity,
        terminal_generation: CoreGeneration,
    },
    BindingNew {
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    },
}

/// Restart recovery result for a durable owner-switch marker. Resumable stages
/// return their typed lease; ambiguous process-exit stages stay fail-closed
/// until the corresponding owner/candidate terminal evidence is available.
pub enum ActiveProfileOwnerSwitchRecovery {
    NoPendingSwitch,
    ResumeClosingOld(Box<dyn ActiveProfileOwnerSwitchLease>),
    ResumeOldStopped(Box<dyn ActiveProfileOwnerStoppedSwitchLease>),
    ResumeReferenceChanged(Box<dyn ActiveProfileReferenceChangedLease>),
    ResumeBindingNew(Box<dyn ActiveProfileOwnerBindingLease>),
    OwnerStillLive {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
    /// ClosingOld survived a process restart without an exact Stopped
    /// readback. Android/Desktop process evidence is an OPEN integration
    /// boundary; the durable switch remains fenced until that evidence is
    /// passed to `recover_old_owner_after_terminal`.
    OwnerTerminalEvidenceRequired {
        identity: CoreIdentity,
        bound_generation: CoreGeneration,
    },
    CandidateStillLive {
        engine_id: EngineId,
    },
    /// BindingNew survived a process restart. Recovery requires the exact
    /// never-started Stopped/no-session candidate snapshot; the store does not
    /// infer candidate state from a dropped in-process lease.
    CandidateTerminalEvidenceRequired {
        selection: ActiveProfileReferenceState,
        engine_id: EngineId,
    },
}

/// Disjoint terminal evidence accepted when reconciling a persisted Core
/// owner after its process-local registration has disappeared. Android's
/// `ManagedChildExited` case is constructed only after exact child identity
/// inspection and terminal VPN readback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileOwnerRecoveryEvidence {
    NeverStarted {
        terminal: CoreLifecycleSnapshot,
    },
    ManagedChildExited {
        context: CoreProcessStartContext,
        terminal: CoreLifecycleSnapshot,
    },
}

/// Revision-fenced persistence for the unique profile reference that selects
/// which engine provider may own the product's Core.
#[async_trait]
pub trait ActiveProfileReferenceStore: Send + Sync {
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
        _evidence: ActiveProfileOwnerRecoveryEvidence,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "active profile owner terminal recovery is unavailable for this store",
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

    /// Reserve a durable provider switch while the exact old Core owner is
    /// active. The root lock is held only while writing the reservation; the
    /// returned lease does not hold it across Core shutdown.
    async fn begin_owner_switch(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        expected_owner: CoreIdentity,
        expected_binding_generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerSwitchLease>, PortError> {
        let _ = (
            expected_revision,
            expected_owner,
            expected_binding_generation,
        );
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "active profile owner switching is unavailable for this store",
        ))
    }

    /// Rehydrate a persisted switch stage after restart. This never clears a
    /// reservation whose owner or candidate process state is ambiguous.
    async fn recover_owner_switch(&self) -> Result<ActiveProfileOwnerSwitchRecovery, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "active profile owner switch recovery is unavailable for this store",
        ))
    }

    /// Advance a crashed `ClosingOld` reservation only after an external
    /// process/lifecycle seam supplies the exact old owner's Stopped snapshot.
    /// The store also verifies that the old owner kernel fence is no longer
    /// held; it never infers terminal state from a missing process-local lease.
    async fn recover_old_owner_after_terminal(
        &self,
        _terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "active profile old-owner terminal recovery is unavailable for this store",
        ))
    }

    /// Clear a crashed candidate reservation only after its exact application
    /// snapshot proves never-started (`generation == 0`), Stopped, and no
    /// session token.
    async fn recover_candidate_after_terminal(
        &self,
        _terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "active profile candidate recovery is unavailable for this store",
        ))
    }
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
            infiltrator_contract::capability::Capability::Profiles,
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

/// The reservation is durable at `ClosingOld`; the old Core owner releases
/// its own binding through `ActiveProfileOwnerLease::release_after_terminal`.
/// This lease then advances only after that exact terminal has been stored.
#[async_trait]
pub trait ActiveProfileOwnerSwitchLease: Send + Sync {
    fn state(&self) -> &ActiveProfileOwnerSwitchState;

    /// Cancel a just-started switch while the old owner is still live. This
    /// is available only from the exact `ClosingOld` stage and restores the
    /// old bound owner without changing the active reference revision.
    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError>;

    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<Box<dyn ActiveProfileOwnerStoppedSwitchLease>, PortError>;
}

/// Stage available only after the old owner's exact terminal readback.
#[async_trait]
pub trait ActiveProfileOwnerStoppedSwitchLease: Send + Sync {
    fn state(&self) -> &ActiveProfileOwnerSwitchState;

    async fn compare_and_set_target(
        self: Box<Self>,
        target: ActiveProfileReferenceTarget,
    ) -> Result<Box<dyn ActiveProfileReferenceChangedLease>, PortError>;
}

/// Stage created by the revision CAS; it reserves the new engine before any
/// provider dispatch can happen.
#[async_trait]
pub trait ActiveProfileReferenceChangedLease: Send + Sync {
    fn state(&self) -> &ActiveProfileOwnerSwitchState;

    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
    ) -> Result<Box<dyn ActiveProfileOwnerBindingLease>, PortError>;
}

/// Kernel owner admission is held before dispatch. Finalization binds the
/// precise identity discovered from the new provider and clears the durable
/// switching marker.
#[async_trait]
pub trait ActiveProfileOwnerBindingLease: Send + Sync {
    fn state(&self) -> &ActiveProfileOwnerSwitchState;

    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError>;

    /// Release a reserved candidate only after its own exact Stopped readback
    /// at generation zero proves it was never started and has no session. The
    /// active reference remains at the candidate engine for a later retry.
    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<ActiveProfileReferenceState, PortError>;

    /// Release a provider reservation when dispatch/construction failed
    /// before a CoreApplication or process actor existed. Callers must use
    /// this only on that pre-application failure path.
    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<ActiveProfileReferenceState, PortError>;
}
