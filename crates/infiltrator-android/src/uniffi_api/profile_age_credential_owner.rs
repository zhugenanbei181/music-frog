//! Typed Android owner routing for one-shot Age credential operations.

use super::control::android_core_lifecycle_status;
use super::owner_operation::AndroidOwnerOperationContext;
use super::profiles::build_profile_application;
use super::vpn::vpn_configuration_activation_status;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::shared_core;
use crate::host_support::{get_runtime, map_application_failure};
use crate::uniffi_api::owner_operation::{
    AndroidOwnerDataDomain, AndroidOwnerOperationIpcResult, AndroidOwnerOperationIpcState,
    OwnerDataRevisionFenceAcquire, advance_owner_data_revision, android_owner_data_revision,
    owner_data_revision_fence, try_owner_data_revision_fence,
};
use infiltrator_application::vpn_preparation::VpnConfigurationActivationStatus;
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_contract::profile_age_credential::ProfileAgeCredentialReadOutcome;
use infiltrator_contract::profile_age_credential::{
    AgePrivateKey, ProfileAgeCredentialMutation, ProfileAgeCredentialOwnerMetadata,
    ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerRequest,
    ProfileAgeCredentialOwnerSubmission, ProfileAgeCredentialTarget,
    ProfileAgeCredentialWriteRequest,
};
use infiltrator_platform::android_bridge::get_android_bridge;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_contract::identity::CoreScope;
use serde::{Deserialize, Serialize};
use std::mem;
use std::ptr::write_volatile;
use std::sync::atomic::Ordering;
use std::sync::atomic::compiler_fence;

pub(crate) const PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfileAgeCredentialOwnerWireRequest {
    pub(crate) protocol: u8,
    pub(crate) metadata: ProfileAgeCredentialOwnerMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ProfileAgeCredentialOwnerWireOutcome {
    Completed {
        readback: ProfileAgeCredentialOwnerReadback,
    },
    Rejected {
        status: FfiStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfileAgeCredentialOwnerWireReply {
    pub(crate) protocol: u8,
    pub(crate) metadata: ProfileAgeCredentialOwnerMetadata,
    pub(crate) outcome: ProfileAgeCredentialOwnerWireOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfileAgeCredentialDataMutation {
    ReadOnly,
    Mutating,
}

fn data_mutation(metadata: &ProfileAgeCredentialOwnerMetadata) -> ProfileAgeCredentialDataMutation {
    match metadata {
        ProfileAgeCredentialOwnerMetadata::Read { .. } => {
            ProfileAgeCredentialDataMutation::ReadOnly
        }
        ProfileAgeCredentialOwnerMetadata::Set { .. }
        | ProfileAgeCredentialOwnerMetadata::Clear { .. } => {
            ProfileAgeCredentialDataMutation::Mutating
        }
    }
}

fn decode_owner_request(
    metadata: ProfileAgeCredentialOwnerMetadata,
    mut private_key_bytes: Vec<u8>,
) -> Result<ProfileAgeCredentialOwnerRequest, FfiStatus> {
    match metadata {
        ProfileAgeCredentialOwnerMetadata::Read { scope, profile_id } => {
            if private_key_bytes.is_empty() {
                Ok(ProfileAgeCredentialOwnerRequest::Read { scope, profile_id })
            } else {
                scrub_bytes(&mut private_key_bytes);
                Err(FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    "Age credential read request carried an unexpected secret payload",
                ))
            }
        }
        ProfileAgeCredentialOwnerMetadata::Set {
            scope,
            expected,
            expected_selection,
        } => match AgePrivateKey::from_bytes(private_key_bytes) {
            Ok(private_key) => Ok(ProfileAgeCredentialOwnerRequest::Set {
                scope,
                expected,
                expected_selection,
                private_key,
            }),
            Err(error) => Err(FfiStatus::err(
                FfiErrorCode::InvalidInput,
                error.to_string(),
            )),
        },
        ProfileAgeCredentialOwnerMetadata::Clear {
            scope,
            expected,
            expected_selection,
        } => {
            if private_key_bytes.is_empty() {
                Ok(ProfileAgeCredentialOwnerRequest::Clear {
                    scope,
                    expected,
                    expected_selection,
                })
            } else {
                scrub_bytes(&mut private_key_bytes);
                Err(FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    "Age credential clear request carried an unexpected secret payload",
                ))
            }
        }
    }
}

fn request_metadata(
    request: &ProfileAgeCredentialOwnerRequest,
) -> ProfileAgeCredentialOwnerMetadata {
    request.metadata()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestScopeMatch {
    Exact,
    Different,
}

fn request_scope_match(
    metadata: &ProfileAgeCredentialOwnerMetadata,
    observed: &CoreScope,
) -> RequestScopeMatch {
    let requested = match metadata {
        ProfileAgeCredentialOwnerMetadata::Read { scope, .. }
        | ProfileAgeCredentialOwnerMetadata::Set { scope, .. }
        | ProfileAgeCredentialOwnerMetadata::Clear { scope, .. } => scope,
    };
    match requested == observed {
        true => RequestScopeMatch::Exact,
        false => RequestScopeMatch::Different,
    }
}

fn wire_reply(
    metadata: ProfileAgeCredentialOwnerMetadata,
    outcome: ProfileAgeCredentialOwnerWireOutcome,
) -> String {
    serde_json::to_string(&ProfileAgeCredentialOwnerWireReply {
        protocol: PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2,
        metadata,
        outcome,
    })
    .unwrap_or_else(|error| {
        log::error!("Android Age credential reply serialization failed: {error}");
        String::new()
    })
}

fn result(
    state: AndroidOwnerOperationIpcState,
    status: FfiStatus,
    core_generation: u64,
    data_revision: u64,
    outcome_json: String,
) -> AndroidOwnerOperationIpcResult {
    AndroidOwnerOperationIpcResult {
        state,
        status_code: status.code,
        status_message: status.message,
        core_generation,
        data_revision,
        data_domain: Some(AndroidOwnerDataDomain::Profiles),
        snapshot_json: None,
        outcome_json,
    }
}

fn rejected_result(
    state: AndroidOwnerOperationIpcState,
    status: FfiStatus,
    metadata: Option<ProfileAgeCredentialOwnerMetadata>,
    observed_generation: u64,
    observed_revision: u64,
) -> AndroidOwnerOperationIpcResult {
    let outcome_json = metadata.map_or_else(String::new, |metadata| {
        wire_reply(
            metadata,
            ProfileAgeCredentialOwnerWireOutcome::Rejected {
                status: status.clone(),
            },
        )
    });
    result(
        state,
        status,
        observed_generation,
        observed_revision,
        outcome_json,
    )
}

pub(crate) async fn execute_owner_request(
    request: ProfileAgeCredentialOwnerRequest,
) -> Result<ProfileAgeCredentialOwnerReadback, FfiStatus> {
    let metadata = request_metadata(&request);
    let core = shared_core().await?;
    let current_scope = match core.application.scope_observation() {
        CoreScopeObservation::Observed(scope) => scope,
        CoreScopeObservation::Unavailable => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Age credential operations require an active Core owner scope",
            ));
        }
    };
    match request_scope_match(&metadata, &current_scope) {
        RequestScopeMatch::Exact => {}
        RequestScopeMatch::Different => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Age credential request belongs to a stale Core scope",
            ));
        }
    }

    let authorization = core
        .application
        .active_profile_reference_write_authorization()
        .map_err(map_application_failure)?;
    let application = build_profile_application().await?;
    match request {
        ProfileAgeCredentialOwnerRequest::Read { profile_id, .. } => application
            .profile_age_credential(profile_id, authorization)
            .await
            .map(ProfileAgeCredentialOwnerReadback::Read)
            .map_err(map_application_failure),
        ProfileAgeCredentialOwnerRequest::Set {
            expected,
            expected_selection,
            private_key,
            ..
        } => application
            .mutate_profile_age_credential(ProfileAgeCredentialWriteRequest {
                expected,
                expected_selection,
                authorization,
                target: ProfileAgeCredentialTarget::Set(private_key),
            })
            .await
            .map(ProfileAgeCredentialOwnerReadback::Mutation)
            .map_err(map_application_failure),
        ProfileAgeCredentialOwnerRequest::Clear {
            expected,
            expected_selection,
            ..
        } => application
            .mutate_profile_age_credential(ProfileAgeCredentialWriteRequest {
                expected,
                expected_selection,
                authorization,
                target: ProfileAgeCredentialTarget::Clear,
            })
            .await
            .map(ProfileAgeCredentialOwnerReadback::Mutation)
            .map_err(map_application_failure),
    }
}

fn readback_status(readback: &ProfileAgeCredentialOwnerReadback) -> FfiStatus {
    match readback {
        ProfileAgeCredentialOwnerReadback::Read(outcome) => match outcome {
            ProfileAgeCredentialReadOutcome::Observed { .. } => FfiStatus::ok(),
            ProfileAgeCredentialReadOutcome::StaleSelection { .. } => FfiStatus::err(
                FfiErrorCode::NotReady,
                "active profile changed before the Age credential read completed",
            ),
        },
        ProfileAgeCredentialOwnerReadback::Mutation(mutation) => match mutation {
            ProfileAgeCredentialMutation::Updated { .. }
            | ProfileAgeCredentialMutation::Unchanged { .. }
            | ProfileAgeCredentialMutation::Cleared { .. } => FfiStatus::ok(),
            ProfileAgeCredentialMutation::StaleCredential { .. } => FfiStatus::err(
                FfiErrorCode::NotReady,
                "Age credential revision changed before the mutation completed",
            ),
            ProfileAgeCredentialMutation::StaleSelection { .. } => FfiStatus::err(
                FfiErrorCode::NotReady,
                "active profile changed before the Age credential mutation completed",
            ),
        },
    }
}

fn credential_revision_changed(readback: &ProfileAgeCredentialOwnerReadback) -> bool {
    matches!(
        readback,
        ProfileAgeCredentialOwnerReadback::Mutation(
            ProfileAgeCredentialMutation::Updated { .. }
                | ProfileAgeCredentialMutation::Cleared { .. }
        )
    )
}

fn scrub_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: each pointer addresses a valid byte in the owned payload.
        unsafe { write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

struct SensitiveBytes(Vec<u8>);

impl SensitiveBytes {
    fn into_bytes(mut self) -> Vec<u8> {
        mem::take(&mut self.0)
    }
}

impl Drop for SensitiveBytes {
    fn drop(&mut self) {
        scrub_bytes(&mut self.0);
    }
}

async fn dispatch_owner_request(
    context: AndroidOwnerOperationContext,
    metadata: ProfileAgeCredentialOwnerMetadata,
    private_key_bytes: SensitiveBytes,
) -> AndroidOwnerOperationIpcResult {
    let mutation = data_mutation(&metadata);
    let Some(bridge) = get_android_bridge() else {
        return rejected_result(
            AndroidOwnerOperationIpcState::Failed,
            FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable"),
            Some(metadata),
            0,
            android_owner_data_revision(),
        );
    };
    if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
        return rejected_result(
            AndroidOwnerOperationIpcState::Failed,
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Age credential data is owned by the Android :vpn process",
            ),
            Some(metadata),
            0,
            android_owner_data_revision(),
        );
    }
    if context.client_session_id.trim().is_empty()
        || context.owner_session_id.trim().is_empty()
        || context.expected_owner_session_id != context.owner_session_id
    {
        return rejected_result(
            AndroidOwnerOperationIpcState::StaleOwnerData,
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android owner IPC session fence did not match",
            ),
            Some(metadata),
            0,
            android_owner_data_revision(),
        );
    }
    let _owner_operation_guard = match mutation {
        ProfileAgeCredentialDataMutation::ReadOnly => owner_data_revision_fence().await,
        ProfileAgeCredentialDataMutation::Mutating => match try_owner_data_revision_fence() {
            OwnerDataRevisionFenceAcquire::Acquired(guard) => guard,
            OwnerDataRevisionFenceAcquire::Busy => {
                return rejected_result(
                    AndroidOwnerOperationIpcState::VpnConfigurationBusy,
                    FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "VPN configuration currently fences owner-data writes",
                    ),
                    Some(metadata),
                    context.expected_core_generation,
                    android_owner_data_revision(),
                );
            }
        },
    };
    let observed_generation = android_core_lifecycle_status().await.generation;
    let observed_revision = android_owner_data_revision();
    if context.expected_core_generation != observed_generation
        || context.expected_data_revision != observed_revision
    {
        return rejected_result(
            AndroidOwnerOperationIpcState::StaleOwnerData,
            FfiStatus::err(
                FfiErrorCode::NotReady,
                "Age credential owner data request is stale",
            ),
            Some(metadata),
            observed_generation,
            observed_revision,
        );
    }
    match (mutation, vpn_configuration_activation_status()) {
        (ProfileAgeCredentialDataMutation::ReadOnly, _)
        | (
            ProfileAgeCredentialDataMutation::Mutating,
            VpnConfigurationActivationStatus::Available,
        ) => {}
        (
            ProfileAgeCredentialDataMutation::Mutating,
            VpnConfigurationActivationStatus::BuilderAdmitted
            | VpnConfigurationActivationStatus::DataPlaneStarting,
        ) => {
            return rejected_result(
                AndroidOwnerOperationIpcState::VpnConfigurationBusy,
                FfiStatus::err(
                    FfiErrorCode::NotReady,
                    "VPN configuration activation holds the owner-data revision fence",
                ),
                Some(metadata),
                observed_generation,
                observed_revision,
            );
        }
    }
    if mutation == ProfileAgeCredentialDataMutation::Mutating && observed_revision == u64::MAX {
        return rejected_result(
            AndroidOwnerOperationIpcState::Failed,
            FfiStatus::err(
                FfiErrorCode::InvalidState,
                "Android owner data revision is exhausted",
            ),
            Some(metadata),
            observed_generation,
            observed_revision,
        );
    }
    let owner_request = match decode_owner_request(metadata.clone(), private_key_bytes.into_bytes())
    {
        Ok(request) => request,
        Err(status) => {
            let outcome_json = wire_reply(
                metadata,
                ProfileAgeCredentialOwnerWireOutcome::Rejected {
                    status: status.clone(),
                },
            );
            return result(
                AndroidOwnerOperationIpcState::Completed,
                status,
                observed_generation,
                observed_revision,
                outcome_json,
            );
        }
    };
    let (outcome, status, next_revision) = match execute_owner_request(owner_request).await {
        Ok(readback) => {
            let status = readback_status(&readback);
            let revision = if credential_revision_changed(&readback) {
                advance_owner_data_revision().unwrap_or(observed_revision)
            } else {
                observed_revision
            };
            (
                ProfileAgeCredentialOwnerWireOutcome::Completed { readback },
                status,
                revision,
            )
        }
        Err(status) => (
            ProfileAgeCredentialOwnerWireOutcome::Rejected {
                status: status.clone(),
            },
            status,
            observed_revision,
        ),
    };
    result(
        AndroidOwnerOperationIpcState::Completed,
        status,
        observed_generation,
        next_revision,
        wire_reply(metadata, outcome),
    )
}

/// Owner-process Messenger entry for the separate, one-shot Age payload. The
/// metadata stays JSON-serializable; the private key arrives as raw bytes and
/// is immediately converted to its zeroizing contract wrapper.
#[uniffi::export]
pub async fn android_dispatch_profile_age_credential_owner_operation_wire_json(
    request_json: String,
    private_key_bytes: Vec<u8>,
    client_session_id: String,
    expected_owner_session_id: String,
    owner_session_id: String,
    expected_core_generation: u64,
    expected_data_revision: u64,
) -> AndroidOwnerOperationIpcResult {
    let context = AndroidOwnerOperationContext {
        client_session_id,
        expected_owner_session_id,
        owner_session_id,
        expected_core_generation,
        expected_data_revision,
    };
    let private_key_bytes = SensitiveBytes(private_key_bytes);
    get_runtime()
        .spawn(async move {
            let request =
                match serde_json::from_str::<ProfileAgeCredentialOwnerWireRequest>(&request_json) {
                    Ok(request) if request.protocol == PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2 => {
                        request
                    }
                    Ok(_) => {
                        return rejected_result(
                            AndroidOwnerOperationIpcState::Failed,
                            FfiStatus::err(
                                FfiErrorCode::NotSupported,
                                "Age credential owner protocol version is unsupported",
                            ),
                            None,
                            0,
                            android_owner_data_revision(),
                        );
                    }
                    Err(error) => {
                        return rejected_result(
                            AndroidOwnerOperationIpcState::Failed,
                            FfiStatus::err(
                                FfiErrorCode::InvalidInput,
                                format!("Age credential request is invalid: {error}"),
                            ),
                            None,
                            0,
                            android_owner_data_revision(),
                        );
                    }
                };
            dispatch_owner_request(context, request.metadata, private_key_bytes).await
        })
        .await
        .unwrap_or_else(|error| {
            let status = FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("Age credential owner task failed: {error}"),
            );
            rejected_result(
                AndroidOwnerOperationIpcState::Failed,
                status,
                None,
                0,
                android_owner_data_revision(),
            )
        })
}

pub async fn execute_profile_age_credential_owner_submission(
    submission: ProfileAgeCredentialOwnerSubmission,
) -> Result<ProfileAgeCredentialOwnerReadback, FfiStatus> {
    let Some(bridge) = get_android_bridge() else {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android host bridge is unavailable",
        ));
    };
    match bridge.process_role() {
        AndroidHostProcessRole::UiClient => bridge
            .execute_profile_age_credential_owner_operation(submission)
            .await
            .map_err(|error| FfiStatus::err(FfiErrorCode::NotReady, error.to_string())),
        AndroidHostProcessRole::CoreOwner => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Core owner submissions must enter through the fenced owner dispatcher",
        )),
        AndroidHostProcessRole::Unsupported => Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android Age credential operations are unavailable in this process",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2, ProfileAgeCredentialOwnerWireRequest,
        RequestScopeMatch, decode_owner_request, request_scope_match,
    };
    use crate::ffi::FfiErrorCode;
    use infiltrator_contract::active_profile_reference::{
        ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    };
    use infiltrator_contract::profile_age_credential::{
        AgePrivateKey, ProfileAgeCredentialObservation, ProfileAgeCredentialOwnerMetadata,
        ProfileAgeCredentialOwnerRequest, ProfileAgeCredentialRevision, ProfileAgeCredentialState,
    };
    use infiltrator_contract::profile_document_identity::ProfileId;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };

    fn scope(generation: u64) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::mihomo(),
                CoreInstanceId::new("android-vpn").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(generation),
            session_token: None,
        }
    }

    fn selected_profile() -> (ProfileId, ActiveProfileReferenceState) {
        let engine = EngineId::mihomo();
        let profile = ProfileId::new("primary").unwrap();
        let selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            engine,
            profile.clone(),
            ActiveProfileReferenceRevision::new(7),
        ));
        (profile, selection)
    }

    fn set_metadata() -> ProfileAgeCredentialOwnerMetadata {
        let (profile_id, expected_selection) = selected_profile();
        ProfileAgeCredentialOwnerMetadata::Set {
            scope: scope(4),
            expected: ProfileAgeCredentialObservation::new(
                EngineId::mihomo(),
                profile_id,
                ProfileAgeCredentialRevision::new(2),
                ProfileAgeCredentialState::NotConfigured,
            ),
            expected_selection,
        }
    }

    #[test]
    fn one_shot_set_bytes_become_redacted_owner_key_and_never_enter_wire_json() {
        let secret = b"AGE-SECRET-KEY-owner-only".to_vec();
        let metadata = set_metadata();
        let request_json = serde_json::to_string(&ProfileAgeCredentialOwnerWireRequest {
            protocol: PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2,
            metadata: metadata.clone(),
        })
        .unwrap();
        assert!(!request_json.contains("AGE-SECRET-KEY-owner-only"));

        let request = decode_owner_request(metadata.clone(), secret).unwrap();
        assert!(matches!(
            &request,
            ProfileAgeCredentialOwnerRequest::Set { private_key, .. }
                if format!("{private_key:?}") == "AgePrivateKey([REDACTED])"
        ));
        assert!(!format!("{request:?}").contains("AGE-SECRET-KEY-owner-only"));
        assert_eq!(request.metadata(), metadata);
    }

    #[test]
    fn age_owner_rejects_secret_for_read_and_invalid_or_oversized_set_payloads() {
        let (profile_id, _) = selected_profile();
        let read = ProfileAgeCredentialOwnerMetadata::Read {
            scope: scope(4),
            profile_id,
        };
        assert_eq!(
            decode_owner_request(read, b"unexpected".to_vec())
                .unwrap_err()
                .code,
            FfiErrorCode::InvalidInput,
        );
        assert_eq!(
            decode_owner_request(set_metadata(), vec![0xff])
                .unwrap_err()
                .code,
            FfiErrorCode::InvalidInput,
        );
        assert_eq!(
            decode_owner_request(set_metadata(), vec![b'a'; AgePrivateKey::MAX_BYTES + 1],)
                .unwrap_err()
                .code,
            FfiErrorCode::InvalidInput,
        );
    }

    #[test]
    fn age_owner_v2_read_request_has_no_ui_supplied_selection() {
        let (profile_id, _) = selected_profile();
        let metadata = ProfileAgeCredentialOwnerMetadata::Read {
            scope: scope(4),
            profile_id: profile_id.clone(),
        };
        let encoded = serde_json::to_string(&metadata).unwrap();
        assert!(!encoded.contains("expected_selection"));
        assert_eq!(
            serde_json::from_str::<ProfileAgeCredentialOwnerMetadata>(&encoded).unwrap(),
            metadata,
        );
        assert!(matches!(
            decode_owner_request(metadata, Vec::new()).unwrap(),
            ProfileAgeCredentialOwnerRequest::Read {
                profile_id: observed,
                ..
            } if observed == profile_id
        ));
    }

    #[test]
    fn android_owner_request_requires_exact_core_scope() {
        let metadata = set_metadata();
        assert_eq!(
            request_scope_match(&metadata, &scope(4)),
            RequestScopeMatch::Exact,
        );
        assert_eq!(
            request_scope_match(&metadata, &scope(5)),
            RequestScopeMatch::Different,
        );
    }
}
