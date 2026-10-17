//! Engine-neutral owner-switch journal and lease ports.
//!
//! The journal is the single durable fence for replacing the active Core
//! owner. Every lease below is a capability issued for one exact revision;
//! using a lease after the journal advanced is a stale-lease failure, never an
//! implicit retry.
//!
//! Recovery is split deliberately: [`EngineSwitchJournal::recover_owner_switch`]
//! returns the pure stage vocabulary from `lurker-core-contract`, while the
//! resumable leases are obtained separately from
//! [`EngineSwitchJournal::recover_old_owner_after_terminal`] and
//! [`EngineSwitchJournal::recover_candidate_after_terminal`] once the caller
//! holds the exact terminal evidence. This keeps the contract crate free of
//! port traits.

use crate::error::PortError;
use async_trait::async_trait;
use lurker_core_contract::engine_switch::{
    EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot, EngineSwitchStage,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};

/// Durable, revision-fenced owner-switch journal.
///
/// Implementations must commit each transition atomically and must never
/// infer an old-owner or candidate terminal from a dropped process-local
/// lease. `Selection` is the host's selection identity; the journal also
/// records the target `EngineId` for the selection-changed stage.
#[async_trait]
pub trait EngineSwitchJournal: Send + Sync {
    type Selection: Clone + Eq + Send + Sync + 'static;

    /// Read selection and owner state without changing the journal.
    async fn snapshot(&self) -> Result<EngineSwitchSnapshot<Self::Selection>, PortError>;

    /// Bind the first owner of an unowned journal.
    async fn bind_owner(
        &self,
        expected_revision: EngineSwitchRevision,
        selection: Self::Selection,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<Self::Selection>, PortError>;

    /// Reserve a durable switch while the exact previous owner is live.
    async fn begin_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
        previous: Self::Selection,
        owner_identity: CoreIdentity,
        owner_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerSwitchLease<Self::Selection>>, PortError>;

    /// Observe an unfinished switch without resuming it.
    async fn recover_owner_switch(
        &self,
        expected_revision: EngineSwitchRevision,
    ) -> Result<EngineSwitchRecovery<Self::Selection>, PortError>;

    /// Rehydrate the stopped-old-owner stage after the exact terminal
    /// generation is known. Accepts only `ClosingOld`, `OldStopped` or
    /// `SelectionChanged`; a terminal generation older than the bound
    /// generation is rejected.
    async fn recover_old_owner_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<Self::Selection>>, PortError>;

    /// Release a reserved candidate after its exact never-started terminal
    /// generation is known. The selection stays at the new target; the owner
    /// becomes unowned.
    async fn recover_candidate_after_terminal(
        &self,
        expected_revision: EngineSwitchRevision,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<Self::Selection>, PortError>;
}

/// Binding of one exact owner generation.
pub trait EngineOwnerLease: Send + Sync {
    fn identity(&self) -> &CoreIdentity;
    fn generation(&self) -> CoreGeneration;
}

/// Switch reservation while the previous owner may still be live.
#[async_trait]
pub trait EngineOwnerSwitchLease<S>: Send + Sync
where
    S: Clone + Eq + Send + Sync + 'static,
{
    fn stage(&self) -> &EngineSwitchStage<S>;

    /// Cancel from the exact `ClosingOld` stage. The previous selection and
    /// its exact bound owner fence are restored; this is the only rollback
    /// that may leave the previous owner usable.
    async fn cancel_before_old_owner_close(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<S>, PortError>;

    /// Record the exact terminal generation of the previous owner and advance
    /// to `OldStopped`. A terminal generation older than the bound generation
    /// is rejected.
    async fn confirm_old_owner_stopped(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerStoppedSwitchLease<S>>, PortError>;
}

/// Stage available only after the previous owner has an exact terminal.
#[async_trait]
pub trait EngineOwnerStoppedSwitchLease<S>: Send + Sync
where
    S: Clone + Eq + Send + Sync + 'static,
{
    fn stage(&self) -> &EngineSwitchStage<S>;

    /// Commit the new selection and target engine. The journal retains the
    /// target engine for the later `BindingNew` stage. Repeating the same
    /// target from a recovered `SelectionChanged` stage is idempotent; a
    /// different target is rejected.
    async fn compare_and_set_target(
        self: Box<Self>,
        selection: S,
        engine_id: EngineId,
    ) -> Result<Box<dyn EngineSwitchSelectionChangedLease<S>>, PortError>;
}

/// Stage created by the selection compare-and-set, before any candidate
/// owner admission exists.
#[async_trait]
pub trait EngineSwitchSelectionChangedLease<S>: Send + Sync
where
    S: Clone + Eq + Send + Sync + 'static,
{
    fn stage(&self) -> &EngineSwitchStage<S>;

    /// Reserve the new owner admission. The candidate identity must belong to
    /// `engine_id` and must not have started (`generation` is zero). Provider
    /// dispatch happens before this reservation in the coordinator.
    async fn reserve_new_owner(
        self: Box<Self>,
        engine_id: EngineId,
        candidate_identity: CoreIdentity,
        candidate_generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerBindingLease<S>>, PortError>;
}

/// Candidate admission held until the new owner is bound.
#[async_trait]
pub trait EngineOwnerBindingLease<S>: Send + Sync
where
    S: Clone + Eq + Send + Sync + 'static,
{
    fn stage(&self) -> &EngineSwitchStage<S>;

    /// Bind the exact candidate identity and generation, clearing the switch.
    async fn finalize(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn EngineOwnerLease>, PortError>;

    /// Release a reserved candidate only after its exact never-started
    /// terminal generation proves it never ran. The journal returns to
    /// `SelectionChanged` so the caller may retry the reservation.
    async fn discard_unstarted_candidate(
        self: Box<Self>,
        terminal_generation: CoreGeneration,
    ) -> Result<EngineSwitchSnapshot<S>, PortError>;

    /// Release a reservation when dispatch or construction failed before a
    /// Core application existed. The journal becomes unowned while keeping
    /// the committed selection; callers must use this only on the
    /// pre-application failure path.
    async fn abort_before_core_application(
        self: Box<Self>,
    ) -> Result<EngineSwitchSnapshot<S>, PortError>;
}
