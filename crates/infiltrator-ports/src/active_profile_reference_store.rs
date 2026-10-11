//! Product-wide active-profile reference persistence.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity};
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;

/// Revision-fenced persistence for the unique profile reference that selects
/// which engine provider may own the product's Core.
#[async_trait]
pub trait ActiveProfileReferenceStore: Send + Sync {
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

    /// Compare the current revision before publication and return the exact
    /// committed readback. A stale revision must fail without writing.
    async fn compare_and_set(
        &self,
        expected_revision: ActiveProfileReferenceRevision,
        target: ActiveProfileReferenceTarget,
    ) -> Result<ActiveProfileReferenceState, PortError>;
}

pub trait ActiveProfileReferenceLease: Send + Sync {
    fn state(&self) -> &ActiveProfileReferenceState;

    /// Convert the exclusive selection lease into a non-blocking owner guard
    /// after the selected Core actor and its capability gate are bound. The
    /// guard prevents cross-engine CAS while allowing same-engine profile
    /// selection to continue.
    fn into_owner_binding(
        self: Box<Self>,
        identity: CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<Box<dyn ActiveProfileOwnerLease>, PortError>;
}

pub trait ActiveProfileOwnerLease: Send + Sync {
    fn identity(&self) -> &CoreIdentity;
    fn generation(&self) -> CoreGeneration;

    /// Release owner admission only after the CoreApplication has observed
    /// the exact owner in `Stopped` with no session token. Dropping an
    /// unconfirmed lease must leave the owner fence fail-closed.
    fn release_after_terminal(&self, terminal: &CoreLifecycleSnapshot) -> Result<(), PortError>;
}
