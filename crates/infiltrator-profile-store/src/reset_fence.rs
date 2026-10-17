//! Product-wide profile-reset intent parsing and mutation fences.
//!
//! Intent parsing, the pending-reset predicates and the root-to-profile fence
//! are the neutral [`lurker_profile_store::owner_switch::reset`] machinery.
//! This module pins it to the product reference payload and keeps the
//! engine-store namespace initialization step.

use crate::active_reference_owner::ProductCodec;
use crate::store::EngineProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_ports::error::PortError;
use lurker_profile_store::owner_switch::reset;
use std::path::Path;

pub const ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 =
    reset::ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION;
pub const LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION: u32 =
    reset::LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION;
pub const PROFILE_RESET_PENDING_MUTATION_MESSAGE: &str =
    reset::PROFILE_RESET_PENDING_MUTATION_MESSAGE;

pub const ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE: &str = ".active-profile-reset-default.previous";
pub const ACTIVE_PROFILE_RESET_DEFAULT_ABSENT_FILE: &str =
    ".active-profile-reset-default.was-absent";
pub const ACTIVE_PROFILE_RESET_INTENT_FILE: &str = ".active-profile-reset-intent.json";
pub const ACTIVE_PROFILE_RESET_COMPLETE_FILE: &str = ".active-profile-reset-complete.json";

pub type ActiveProfileResetIntentRecord =
    reset::ActiveProfileResetIntentRecord<ActiveProfileReferenceState>;
pub type LegacyActiveProfileResetIntentRecord =
    reset::LegacyActiveProfileResetIntentRecord<ActiveProfileReferenceState>;
pub type ParsedActiveProfileResetIntentRecord =
    reset::ParsedActiveProfileResetIntentRecord<ActiveProfileReferenceState>;

pub fn parse_reset_intent_record(
    bytes: &[u8],
) -> Result<ParsedActiveProfileResetIntentRecord, PortError> {
    reset::parse_reset_intent_record::<ActiveProfileReferenceState>(bytes)
}

pub async fn ensure_reset_not_pending_for_dir(config_dir: &Path) -> Result<(), PortError> {
    reset::ensure_reset_not_pending_for_dir::<ProductCodec, ActiveProfileReferenceState>(config_dir)
        .await
}

pub async fn reset_pending_for_dir(config_dir: &Path) -> Result<bool, PortError> {
    reset::reset_pending_for_dir::<ProductCodec, ActiveProfileReferenceState>(config_dir).await
}

pub async fn ensure_no_pending_reset_in_engine_namespaces(
    config_root: &Path,
    access: reset::ActiveProfileResetMutationAccess,
) -> Result<(), PortError> {
    reset::ensure_no_pending_reset_in_engine_namespaces::<ProductCodec, ActiveProfileReferenceState>(
        config_root, access,
    )
    .await
}

/// Acquire the shared root reference lock, inspect every engine reset intent,
/// then acquire this engine's profile-write boundary in root-to-profile order.
pub async fn acquire_reset_fenced_profile_mutation_for_dir(
    config_dir: &Path,
) -> Result<reset::ResetFencedProfileMutationGuard, PortError> {
    reset::acquire_reset_fenced_profile_mutation_for_dir::<
        ProductCodec,
        ActiveProfileReferenceState,
    >(config_dir)
    .await
}

pub async fn initialize_namespace_with_reset_fence<S: SecureStore>(
    store: &EngineProfileStore<S>,
) -> Result<(), PortError> {
    store
        .initialize_profile_namespace_with(|| async {
            acquire_reset_fenced_profile_mutation_for_dir(store.config_dir()).await
        })
        .await
}
