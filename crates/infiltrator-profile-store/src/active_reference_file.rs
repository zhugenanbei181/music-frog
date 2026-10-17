//! Durable storage for the product-wide active-profile reference record.
//!
//! The file owner, lock order and durability reconciliation are the neutral
//! [`lurker_profile_store::owner_switch::file`] machinery; this alias binds it
//! to the product record codec so every existing call site stays unchanged.

use crate::active_reference_owner::ProductCodec;
use lurker_profile_store::owner_switch::file::OwnerSwitchFile;

/// Stateless product-root file owner. Revision locks and typed transitions are
/// acquired by the protocol coordinator before calling these operations.
pub type ActiveProfileReferenceFile = OwnerSwitchFile<ProductCodec>;

pub const ACTIVE_PROFILE_REFERENCE_FILE: &str = "active-profile-reference.json";

#[cfg(test)]
use lurker_profile_store::owner_switch::file::reconcile_record_publication;

#[cfg(test)]
#[path = "active_reference_file_tests.rs"]
mod tests;
