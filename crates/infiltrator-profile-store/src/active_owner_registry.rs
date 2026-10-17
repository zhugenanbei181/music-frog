//! Process-local owner registrations paired with the cross-process owner lock.
//!
//! The registry carries no selection payload; it is the neutral
//! [`lurker_profile_store::owner_switch::registry`] implementation. This module
//! keeps the product-facing release helper that the Mihomo config manager calls
//! after an explicit legacy migration.

use lurker_core_ports::error::PortError;
use lurker_profile_store::owner_switch::owner::ActiveOwnerToken;
use lurker_profile_store::owner_switch::registry;
use std::path::Path;

pub fn remove_local_owner_registration(
    config_root: &Path,
    token: &ActiveOwnerToken,
) -> Result<(), PortError> {
    registry::remove_local_owner_registration(config_root, token)
}
