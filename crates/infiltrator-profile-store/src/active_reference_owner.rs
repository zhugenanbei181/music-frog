//! Typed durable records for the product-wide active profile and Core owner.
//!
//! The owner state, switch stage, owner proof and tokens are the neutral
//! [`lurker_profile_store::owner_switch`] machinery. This module keeps only the
//! product reference payload, its legacy-migration marker and the codec that
//! maps the product record to the unchanged on-disk JSON.

use crate::layout::product_layout;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use lurker_profile_store::layout::ProfileStoreLayout;
use lurker_profile_store::owner_switch::codec::OwnerSwitchCodec;
use lurker_profile_store::owner_switch::owner::ActiveCoreOwnerState;
use serde::{Deserialize, Serialize};

pub const ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION: u32 = 1;
pub const ACTIVE_CORE_OWNER_STATE_FILE: &str = "active-core-owner.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum LegacySelectionMigration {
    PendingDesktopMihomoV0,
    MigratedDesktopMihomoV0,
    RejectedDesktopMihomoV0,
    ExplicitSelection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveProfileReferenceRecord {
    pub schema_version: u32,
    pub selection: ActiveProfileReferenceState,
    pub legacy_migration: LegacySelectionMigration,
    #[serde(default)]
    pub owner: ActiveCoreOwnerState<ActiveProfileReferenceState>,
}

impl ActiveProfileReferenceRecord {
    pub fn pending_legacy_migration() -> Self {
        Self {
            schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
            selection: ActiveProfileReferenceState::Unselected {
                revision: ActiveProfileReferenceRevision::new(0),
            },
            legacy_migration: LegacySelectionMigration::PendingDesktopMihomoV0,
            owner: ActiveCoreOwnerState::Unowned,
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.schema_version != ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!(
                    "active-profile reference schema {} is unsupported",
                    self.schema_version
                ),
            ));
        }
        Ok(())
    }
}

/// Injected codec for the product's durable owner-switch record.
///
/// The neutral machinery is generic over `Selection` and `Record`; this codec
/// is the only place that knows the product reference payload, the
/// legacy-migration marker and the exact persisted JSON.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductCodec;

impl OwnerSwitchCodec for ProductCodec {
    type Selection = ActiveProfileReferenceState;
    type Record = ActiveProfileReferenceRecord;

    fn layout() -> &'static ProfileStoreLayout {
        product_layout()
    }

    fn selection_revision(selection: &Self::Selection) -> EngineSwitchRevision {
        EngineSwitchRevision::new(selection.revision().value())
    }

    fn selection_engine(selection: &Self::Selection) -> Option<&EngineId> {
        match selection {
            ActiveProfileReferenceState::Selected(reference) => Some(&reference.engine_id),
            ActiveProfileReferenceState::Unselected { .. } => None,
        }
    }

    fn pending() -> Self::Record {
        ActiveProfileReferenceRecord::pending_legacy_migration()
    }

    fn selection(record: &Self::Record) -> &Self::Selection {
        &record.selection
    }

    fn owner(record: &Self::Record) -> &ActiveCoreOwnerState<Self::Selection> {
        &record.owner
    }

    fn set_selection(record: &mut Self::Record, selection: Self::Selection) {
        record.selection = selection;
        record.legacy_migration = LegacySelectionMigration::ExplicitSelection;
    }

    fn set_owner(record: &mut Self::Record, owner: ActiveCoreOwnerState<Self::Selection>) {
        record.owner = owner;
    }

    fn validate(record: &Self::Record) -> Result<(), PortError> {
        record.validate()
    }

    fn encode(record: &Self::Record) -> Result<Vec<u8>, PortError> {
        serde_json::to_vec_pretty(record).map_err(|error| PortError::Io(error.to_string()))
    }

    fn decode(bytes: &[u8]) -> Result<Self::Record, PortError> {
        serde_json::from_slice(bytes)
            .map_err(|error| PortError::Io(format!("invalid active-profile reference: {error}")))
    }
}

#[cfg(test)]
#[path = "active_reference_owner_tests.rs"]
mod tests;
