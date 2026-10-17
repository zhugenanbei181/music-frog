//! Product adapter over the neutral persisted-stage mapping.
//!
//! The neutral mapping is generic over the record codec and the selection
//! payload. This module pins it to [`ProductCodec`] and keeps the product
//! revision conversion.

use crate::active_reference_owner::{ActiveProfileReferenceRecord, ProductCodec};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRevision, EngineSwitchSnapshot,
};
use lurker_core_ports::error::PortError;
use lurker_profile_store::owner_switch::owner::ActiveCoreOwnerState;
use lurker_profile_store::owner_switch::stage;

pub type PersistedSwitchWire = stage::PersistedSwitchWire<ProductCodec>;

pub const fn engine_switch_revision(
    revision: ActiveProfileReferenceRevision,
) -> EngineSwitchRevision {
    EngineSwitchRevision::new(revision.value())
}

pub const fn active_reference_revision(
    revision: EngineSwitchRevision,
) -> ActiveProfileReferenceRevision {
    ActiveProfileReferenceRevision::new(revision.value())
}

pub fn switch_snapshot(
    record: &ActiveProfileReferenceRecord,
) -> Result<EngineSwitchSnapshot<ActiveProfileReferenceState>, PortError> {
    stage::switch_snapshot::<ProductCodec>(record)
}

pub fn persisted_wire(
    record: &ActiveProfileReferenceRecord,
) -> Result<PersistedSwitchWire, PortError> {
    stage::persisted_wire::<ProductCodec>(record)
}

pub fn persisted_owner_state(
    owner: &EngineOwnerState<ActiveProfileReferenceState>,
    wire: &PersistedSwitchWire,
) -> Result<ActiveCoreOwnerState<ActiveProfileReferenceState>, PortError> {
    stage::persisted_owner_state::<ProductCodec>(owner, wire)
}

#[cfg(test)]
#[path = "stage_mapping_tests.rs"]
mod tests;
