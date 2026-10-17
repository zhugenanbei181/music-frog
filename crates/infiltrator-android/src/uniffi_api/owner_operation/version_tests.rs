use super::*;
use infiltrator_contract::version::CoreVersionInstallId;
use lurker_core_contract::error::{ErrorCode, Failure};

#[test]
fn version_operation_maps_to_engine_scoped_application_action() {
    let request = AndroidCoreVersionManagementOperation::Latest {
        engine_id: "sing-box".to_owned(),
        channel: AndroidEngineNeutralReleaseChannel::Alpha,
    }
    .into_application_request()
    .unwrap();

    assert_eq!(request.engine_id.as_str(), "sing-box");
    assert!(matches!(
        request.action,
        CoreVersionManagementAction::Latest {
            channel: EngineNeutralReleaseChannel::Alpha
        }
    ));
}

#[test]
fn version_operation_and_readback_round_trip_as_typed_owner_wire_values() {
    use crate::uniffi_api::owner_operation::{
        AndroidOwnerOperation, AndroidOwnerOperationProtocolVersion, AndroidOwnerOperationReadback,
        AndroidOwnerOperationRequest,
    };

    let request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::CoreVersionManagement {
            operation: AndroidCoreVersionManagementOperation::Install {
                engine_id: "sing-box".to_owned(),
                version: "1.14.3".to_owned(),
            },
        },
    };
    let encoded = serde_json::to_string(&request).unwrap();
    let decoded: AndroidOwnerOperationRequest = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, request);

    let readback = AndroidOwnerOperationReadback::CoreVersionManagement(
        AndroidCoreVersionManagementReadback::InstallObservation {
            observation: AndroidCoreVersionInstallObservation::NotFound { install_id: 15 },
        },
    );
    let encoded = serde_json::to_string(&readback).unwrap();
    let decoded: AndroidOwnerOperationReadback = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, readback);
}

#[test]
fn version_reads_and_mutations_have_distinct_owner_revision_semantics() {
    assert_eq!(
        AndroidCoreVersionManagementOperation::ListInstalled {
            engine_id: "mihomo".to_owned(),
        }
        .data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidCoreVersionManagementOperation::Install {
            engine_id: "mihomo".to_owned(),
            version: "v1.20.3".to_owned(),
        }
        .data_mutation(),
        AndroidOwnerDataMutation::CommitOnSuccess
    );
}

#[test]
fn version_operation_rejects_invalid_engine_identity() {
    let result = AndroidCoreVersionManagementOperation::ListInstalled {
        engine_id: "Invalid Engine".to_owned(),
    }
    .into_application_request();

    assert!(matches!(
        result,
        Err(FfiStatus {
            code: FfiErrorCode::InvalidInput,
            ..
        })
    ));
}

#[test]
fn version_mutation_revision_follows_typed_terminal_outcome() {
    let install_id = CoreVersionInstallId::new(7).unwrap();
    let accepted = AndroidCoreVersionManagementReadback::InstallAccepted {
        install_id: install_id.get(),
        state: AndroidCoreVersionInstallState::Running {
            context: AndroidCoreVersionInstallContext {
                install_id: install_id.get(),
                engine_id: "mihomo".to_owned(),
                version: "v1.20.3".to_owned(),
                revision: 1,
            },
            progress: None,
        },
    };
    assert_eq!(
        accepted.revision_transition(),
        AndroidOwnerRevisionTransition::Advance
    );

    let already_finished = AndroidCoreVersionManagementReadback::InstallCancellation {
        outcome: AndroidCoreVersionInstallCancellation::AlreadyFinished {
            install_id: install_id.get(),
        },
    };
    assert_eq!(
        already_finished.revision_transition(),
        AndroidOwnerRevisionTransition::Unchanged
    );

    let requested = AndroidCoreVersionManagementReadback::InstallCancellation {
        outcome: AndroidCoreVersionInstallCancellation::Requested {
            install_id: install_id.get(),
        },
    };
    assert_eq!(
        requested.revision_transition(),
        AndroidOwnerRevisionTransition::Advance
    );
}

#[test]
fn core_version_state_uses_typed_install_identity_and_progress() {
    let state = CoreVersionInstallState::Running {
        context: CoreVersionInstallContext {
            install_id: CoreVersionInstallId::new(9).unwrap(),
            engine_id: EngineId::new("mihomo").unwrap(),
            version: "v1.20.3".to_owned(),
            revision: 4,
        },
        progress: Some(VersionDownloadProgress {
            downloaded: 1024,
            total: Some(2048),
        }),
    };

    assert!(matches!(
        AndroidCoreVersionInstallState::from(state),
        AndroidCoreVersionInstallState::Running {
            context: AndroidCoreVersionInstallContext {
                install_id: 9,
                revision: 4,
                ..
            },
            progress: Some(AndroidVersionDownloadProgress {
                downloaded: 1024,
                total: Some(2048),
            }),
        }
    ));
}

#[test]
fn version_management_failures_keep_the_owner_error_status() {
    let readback = AndroidCoreVersionManagementReadback::Failed {
        failure: map_application_failure(Failure::new(
            ErrorCode::NotReady,
            "version owner is unavailable",
            true,
        )),
    };

    assert_eq!(readback.status().code, FfiErrorCode::NotReady);
    assert_eq!(
        readback.revision_transition(),
        AndroidOwnerRevisionTransition::Unchanged
    );
}
