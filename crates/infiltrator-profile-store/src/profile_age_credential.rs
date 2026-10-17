//! Owner-authorized, encrypted persistence for per-profile Age private keys.

use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::ActiveCoreOwnerState;
use crate::active_reference_owner_coordinator::{
    ActiveProfileOwnerCoordinator, validate_reference_write_authorization,
};
use crate::file_lock::{ProfileWriteGuard, lock_profile_writes_for_dir};
use crate::reset_fence::{
    ActiveProfileResetMutationAccess, ensure_no_pending_reset_in_engine_namespaces,
    initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use age::x25519::Identity;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeContentDecryption, ProfileAgeCredentialMutation,
    ProfileAgeCredentialObservation, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialReadRequest, ProfileAgeCredentialRevision, ProfileAgeCredentialState,
    ProfileAgeCredentialTarget, ProfileAgeCredentialWriteRequest, ProfileAgeDecryptedContent,
    ProfileAgeEncryptedContent,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_platform::instance_lock::ThreadedInstanceLockGuard;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use serde::{Deserialize, Serialize};
use std::ptr;
use std::str;
use std::sync::atomic::{Ordering, compiler_fence};

const AGE_CREDENTIAL_SERVICE: &str = "MusicFrog-Infiltrator";
const AGE_CREDENTIAL_SCHEMA_VERSION: u8 = 1;
const AGE_CREDENTIAL_KEY_PREFIX: &str = "age";

enum ProfileAgeCredentialFence {
    Authorized(ProfileAgeCredentialWriteGuards),
    StaleSelection {
        observed: ActiveProfileReferenceState,
    },
}

struct ProfileAgeCredentialWriteGuards {
    _root: ThreadedInstanceLockGuard,
    _owner_state: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

struct ProfileAgeCredentialUseGuards {
    _root: ThreadedInstanceLockGuard,
    _owner_state: ThreadedInstanceLockGuard,
    _profile: ProfileWriteGuard,
}

pub(crate) struct ProfileAgeCredentialRetirement {
    profile_id: ProfileId,
    previous: PersistedAgeCredentialRecord,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedAgeCredentialRecord {
    schema_version: u8,
    revision: u64,
    credential: PersistedAgeCredentialState,
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum PersistedAgeCredentialState {
    NotConfigured,
    Configured { private_key: AgePrivateKey },
}

#[derive(Serialize)]
struct AgeCredentialRecordWrite<'a> {
    schema_version: u8,
    revision: u64,
    credential: AgeCredentialStateWrite<'a>,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum AgeCredentialStateWrite<'a> {
    NotConfigured,
    Configured { private_key: &'a str },
}

struct SecretStorePayload(Vec<u8>);

impl SecretStorePayload {
    fn as_str(&self) -> &str {
        str::from_utf8(&self.0).expect("serde_json emits valid UTF-8")
    }
}

impl Drop for SecretStorePayload {
    fn drop(&mut self) {
        scrub_bytes(&mut self.0);
    }
}

impl PersistedAgeCredentialRecord {
    fn empty() -> Self {
        Self {
            schema_version: AGE_CREDENTIAL_SCHEMA_VERSION,
            revision: 0,
            credential: PersistedAgeCredentialState::NotConfigured,
        }
    }

    fn observation(
        &self,
        engine_id: EngineId,
        profile_id: ProfileId,
    ) -> ProfileAgeCredentialObservation {
        let state = match &self.credential {
            PersistedAgeCredentialState::NotConfigured => ProfileAgeCredentialState::NotConfigured,
            PersistedAgeCredentialState::Configured { .. } => ProfileAgeCredentialState::Configured,
        };
        ProfileAgeCredentialObservation::new(
            engine_id,
            profile_id,
            ProfileAgeCredentialRevision::new(self.revision),
            state,
        )
    }

    fn matches(&self, expected: &Self) -> bool {
        if self.schema_version != expected.schema_version || self.revision != expected.revision {
            return false;
        }
        match (&self.credential, &expected.credential) {
            (
                PersistedAgeCredentialState::NotConfigured,
                PersistedAgeCredentialState::NotConfigured,
            ) => true,
            (
                PersistedAgeCredentialState::Configured { private_key: left },
                PersistedAgeCredentialState::Configured { private_key: right },
            ) => left.matches(right),
            _ => false,
        }
    }
}

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn profile_age_credential(
        &self,
        request: ProfileAgeCredentialReadRequest,
    ) -> Result<ProfileAgeCredentialReadOutcome, PortError> {
        let fence =
            acquire_age_credential_fence(self, &request.expected_selection, &request.authorization)
                .await?;
        let _guards = match fence {
            ProfileAgeCredentialFence::Authorized(guards) => guards,
            ProfileAgeCredentialFence::StaleSelection { observed } => {
                return Ok(ProfileAgeCredentialReadOutcome::StaleSelection {
                    expected: request.expected_selection,
                    observed,
                });
            }
        };

        let selection = request.expected_selection.clone();
        let record = read_credential_record(self, &request.profile_id).await?;
        Ok(ProfileAgeCredentialReadOutcome::Observed {
            current: record.observation(self.engine_id(), request.profile_id),
            selection,
        })
    }

    pub async fn mutate_profile_age_credential(
        &self,
        request: ProfileAgeCredentialWriteRequest,
    ) -> Result<ProfileAgeCredentialMutation, PortError> {
        if request.expected.engine_id != self.engine_id() {
            return Err(invalid_age_credential_request(
                "Age credential snapshot belongs to another engine",
            ));
        }

        let fence =
            acquire_age_credential_fence(self, &request.expected_selection, &request.authorization)
                .await?;
        let _guards = match fence {
            ProfileAgeCredentialFence::Authorized(guards) => guards,
            ProfileAgeCredentialFence::StaleSelection { observed } => {
                return Ok(ProfileAgeCredentialMutation::StaleSelection {
                    expected: request.expected_selection,
                    observed,
                });
            }
        };

        let current = read_credential_record(self, &request.expected.profile_id).await?;
        let current_observation =
            current.observation(self.engine_id(), request.expected.profile_id.clone());
        if current_observation != request.expected {
            return Ok(ProfileAgeCredentialMutation::StaleCredential {
                expected: request.expected,
                observed: current_observation,
            });
        }

        match request.target {
            ProfileAgeCredentialTarget::Keep => Ok(ProfileAgeCredentialMutation::Unchanged {
                current: current_observation,
            }),
            ProfileAgeCredentialTarget::Set(private_key) => {
                if matches!(
                    &current.credential,
                    PersistedAgeCredentialState::Configured { private_key: observed }
                        if observed.matches(&private_key)
                ) {
                    return Ok(ProfileAgeCredentialMutation::Unchanged {
                        current: current_observation,
                    });
                }
                let revision = next_age_credential_revision(&current_observation)?;
                let target = PersistedAgeCredentialRecord {
                    schema_version: AGE_CREDENTIAL_SCHEMA_VERSION,
                    revision: revision.value(),
                    credential: PersistedAgeCredentialState::Configured { private_key },
                };
                write_credential_record(self, &request.expected.profile_id, &target).await?;
                let readback = read_credential_record(self, &request.expected.profile_id).await?;
                if !readback.matches(&target) {
                    return Err(PortError::Failed(
                        "Age credential secure storage returned a mismatched readback".into(),
                    ));
                }
                Ok(ProfileAgeCredentialMutation::Updated {
                    previous: current_observation,
                    current: readback.observation(self.engine_id(), request.expected.profile_id),
                })
            }
            ProfileAgeCredentialTarget::Clear => {
                if matches!(
                    current.credential,
                    PersistedAgeCredentialState::NotConfigured
                ) {
                    return Ok(ProfileAgeCredentialMutation::Unchanged {
                        current: current_observation,
                    });
                }
                let revision = next_age_credential_revision(&current_observation)?;
                let target = PersistedAgeCredentialRecord {
                    schema_version: AGE_CREDENTIAL_SCHEMA_VERSION,
                    revision: revision.value(),
                    credential: PersistedAgeCredentialState::NotConfigured,
                };
                write_credential_record(self, &request.expected.profile_id, &target).await?;
                let readback = read_credential_record(self, &request.expected.profile_id).await?;
                if !readback.matches(&target) {
                    return Err(PortError::Failed(
                        "Age credential secure storage returned a mismatched readback".into(),
                    ));
                }
                Ok(ProfileAgeCredentialMutation::Cleared {
                    previous: current_observation,
                    current: readback.observation(self.engine_id(), request.expected.profile_id),
                })
            }
        }
    }

    /// Decrypt one armored subscription response while holding the same
    /// profile and Core-owner read fences used by credential mutations. The
    /// key remains inside this owner; only a redacted, zeroizing document
    /// value crosses back to the application use case.
    pub async fn decrypt_age_subscription_content(
        &self,
        profile_id: &ProfileId,
        content: ProfileAgeEncryptedContent,
    ) -> Result<ProfileAgeContentDecryption, PortError> {
        let _guards = acquire_age_credential_use_fence(self).await?;
        let record = read_credential_record(self, profile_id).await?;
        let credential_revision = ProfileAgeCredentialRevision::new(record.revision);
        let PersistedAgeCredentialState::Configured { private_key } = record.credential else {
            return Ok(ProfileAgeContentDecryption::CredentialNotConfigured);
        };
        let Ok(identity) = private_key.as_str().parse::<Identity>() else {
            return Ok(ProfileAgeContentDecryption::InvalidCredentialOrContent);
        };
        let Ok(plaintext) = age::decrypt(&identity, content.as_str().as_bytes()) else {
            return Ok(ProfileAgeContentDecryption::InvalidCredentialOrContent);
        };
        let content = match String::from_utf8(plaintext) {
            Ok(content) => content,
            Err(error) => {
                let mut plaintext = error.into_bytes();
                scrub_bytes(&mut plaintext);
                return Ok(ProfileAgeContentDecryption::InvalidCredentialOrContent);
            }
        };
        Ok(ProfileAgeContentDecryption::Decrypted {
            content: ProfileAgeDecryptedContent::new(content),
            credential_revision,
        })
    }
}

/// Advance the credential revision to a `NotConfigured` tombstone before a
/// profile ID is retired. The caller must hold the root selection lease and
/// profile write lock, in that order.
pub(crate) async fn retire_profile_age_credential_locked<S: SecureStore>(
    store: &EngineProfileStore<S>,
    profile_id: &ProfileId,
) -> Result<ProfileAgeCredentialRetirement, PortError> {
    let previous = read_credential_record(store, profile_id).await?;
    let previous_observation = previous.observation(store.engine_id(), profile_id.clone());
    let revision = next_age_credential_revision(&previous_observation)?;
    let retired = PersistedAgeCredentialRecord {
        schema_version: AGE_CREDENTIAL_SCHEMA_VERSION,
        revision: revision.value(),
        credential: PersistedAgeCredentialState::NotConfigured,
    };
    write_credential_record(store, profile_id, &retired).await?;
    let readback = read_credential_record(store, profile_id).await?;
    if !readback.matches(&retired) {
        return Err(PortError::Failed(
            "Age credential retirement returned a mismatched readback".into(),
        ));
    }
    Ok(ProfileAgeCredentialRetirement {
        profile_id: profile_id.clone(),
        previous,
    })
}

impl ProfileAgeCredentialRetirement {
    pub(crate) async fn rollback<S: SecureStore>(
        self,
        store: &EngineProfileStore<S>,
    ) -> Result<(), PortError> {
        write_credential_record(store, &self.profile_id, &self.previous).await?;
        let readback = read_credential_record(store, &self.profile_id).await?;
        if !readback.matches(&self.previous) {
            return Err(PortError::Failed(
                "Age credential restoration returned a mismatched readback".into(),
            ));
        }
        Ok(())
    }
}

async fn acquire_age_credential_fence<S: SecureStore>(
    store: &EngineProfileStore<S>,
    expected_selection: &ActiveProfileReferenceState,
    authorization: &ActiveProfileReferenceWriteAuthorization,
) -> Result<ProfileAgeCredentialFence, PortError> {
    initialize_namespace_with_reset_fence(store).await?;
    let file = ActiveProfileReferenceFile::new(store.config_root().to_path_buf());
    let (config_root, root_guard) = file.acquire_root_lock().await?;
    ensure_no_pending_reset_in_engine_namespaces(
        &config_root,
        ActiveProfileResetMutationAccess::OrdinaryMutation,
    )
    .await?;
    let mut record = file.read_active_profile_record_sync_or_pending()?;
    if &record.selection != expected_selection {
        return Ok(ProfileAgeCredentialFence::StaleSelection {
            observed: record.selection,
        });
    }
    let (owner_state, owner_state_guard) = ActiveProfileOwnerCoordinator::new(config_root.clone())
        .normalize_owner_state_unlocked(&mut record)
        .await?;
    validate_reference_write_authorization(&owner_state, authorization, &config_root)?;
    match &owner_state {
        ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {}
        ActiveCoreOwnerState::Switching { .. } => {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Age credentials cannot change during a Core owner switch",
                true,
            )));
        }
    }
    let profile_guard = lock_profile_writes_for_dir(store.config_dir()).await?;
    Ok(ProfileAgeCredentialFence::Authorized(
        ProfileAgeCredentialWriteGuards {
            _root: root_guard,
            _owner_state: owner_state_guard,
            _profile: profile_guard,
        },
    ))
}

async fn acquire_age_credential_use_fence<S: SecureStore>(
    store: &EngineProfileStore<S>,
) -> Result<ProfileAgeCredentialUseGuards, PortError> {
    initialize_namespace_with_reset_fence(store).await?;
    let file = ActiveProfileReferenceFile::new(store.config_root().to_path_buf());
    let (config_root, root_guard) = file.acquire_root_lock().await?;
    ensure_no_pending_reset_in_engine_namespaces(
        &config_root,
        ActiveProfileResetMutationAccess::OrdinaryMutation,
    )
    .await?;
    let mut record = file.read_active_profile_record_sync_or_pending()?;
    let (owner_state, owner_state_guard) = ActiveProfileOwnerCoordinator::new(config_root.clone())
        .normalize_owner_state_unlocked(&mut record)
        .await?;
    match owner_state {
        ActiveCoreOwnerState::Unowned | ActiveCoreOwnerState::Bound { .. } => {}
        ActiveCoreOwnerState::Switching { .. } => {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Age credentials cannot be used during a Core owner switch",
                true,
            )));
        }
    }
    let profile_guard = lock_profile_writes_for_dir(store.config_dir()).await?;
    Ok(ProfileAgeCredentialUseGuards {
        _root: root_guard,
        _owner_state: owner_state_guard,
        _profile: profile_guard,
    })
}

async fn read_credential_record<S: SecureStore>(
    store: &EngineProfileStore<S>,
    profile_id: &ProfileId,
) -> Result<PersistedAgeCredentialRecord, PortError> {
    let key = age_credential_key(&store.engine_id(), profile_id.as_str());
    let mut serialized = store
        .credential_store()
        .get(AGE_CREDENTIAL_SERVICE, &key)
        .await
        .map_err(|error| sanitize_secure_store_error(error, "read"))?;
    let Some(mut serialized) = serialized.take() else {
        return Ok(PersistedAgeCredentialRecord::empty());
    };
    let decoded = serde_json::from_str::<PersistedAgeCredentialRecord>(&serialized);
    scrub_string(&mut serialized);
    let record = decoded.map_err(|_| invalid_stored_age_credential())?;
    if record.schema_version != AGE_CREDENTIAL_SCHEMA_VERSION {
        return Err(PortError::unsupported(
            Capability::Profiles,
            "stored Age credential schema is unsupported",
        ));
    }
    Ok(record)
}

async fn write_credential_record<S: SecureStore>(
    store: &EngineProfileStore<S>,
    profile_id: &ProfileId,
    record: &PersistedAgeCredentialRecord,
) -> Result<(), PortError> {
    let credential = match &record.credential {
        PersistedAgeCredentialState::NotConfigured => AgeCredentialStateWrite::NotConfigured,
        PersistedAgeCredentialState::Configured { private_key } => {
            AgeCredentialStateWrite::Configured {
                private_key: private_key.as_str(),
            }
        }
    };
    let payload = SecretStorePayload(
        serde_json::to_vec(&AgeCredentialRecordWrite {
            schema_version: record.schema_version,
            revision: record.revision,
            credential,
        })
        .map_err(|_| PortError::Failed("could not encode Age credential state".into()))?,
    );
    let key = age_credential_key(&store.engine_id(), profile_id.as_str());
    store
        .credential_store()
        .set(AGE_CREDENTIAL_SERVICE, &key, payload.as_str())
        .await
        .map_err(|error| sanitize_secure_store_error(error, "write"))
}

fn next_age_credential_revision(
    current: &ProfileAgeCredentialObservation,
) -> Result<ProfileAgeCredentialRevision, PortError> {
    current.revision.next().ok_or_else(|| {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "Age credential revision is exhausted",
            false,
        ))
    })
}

fn age_credential_key(engine: &EngineId, profile: &str) -> String {
    format!("{AGE_CREDENTIAL_KEY_PREFIX}:{}:{profile}", engine.as_str())
}

fn invalid_age_credential_request(message: &str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidInput, message, false))
}

fn invalid_stored_age_credential() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        "secure Age credential record is invalid",
        false,
    ))
}

fn sanitize_secure_store_error(error: PortError, operation: &str) -> PortError {
    let reason = match operation {
        "read" => "Age credential secure read failed",
        "write" => "Age credential secure write failed",
        _ => "Age credential secure operation failed",
    };
    match error {
        PortError::Rejected(failure) => {
            PortError::Rejected(Failure::new(failure.code, reason, failure.retryable))
        }
        PortError::Unsupported { capability, .. } => PortError::Unsupported {
            capability,
            reason: "Age credential secure storage is unavailable".into(),
        },
        PortError::PermissionDenied(_) => PortError::PermissionDenied(
            "Age credential secure storage permission was denied".into(),
        ),
        PortError::NotFound(_) => PortError::NotFound(reason.into()),
        PortError::Io(_) => PortError::Io(reason.into()),
        PortError::Network(_) => PortError::Network(reason.into()),
        PortError::Failed(_) => PortError::Failed(reason.into()),
    }
}

fn scrub_string(value: &mut String) {
    scrub_bytes(unsafe { value.as_bytes_mut() });
}

fn scrub_bytes(value: &mut [u8]) {
    for byte in value {
        unsafe { ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
#[path = "profile_age_credential_tests.rs"]
mod tests;
