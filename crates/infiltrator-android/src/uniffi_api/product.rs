use crate::ffi::{FfiErrorCode, FfiStringResult};
use crate::host_session::shared_core;
use crate::host_support::get_runtime;
use crate::product::owner_surface_snapshot_json;
use infiltrator_contract::android_control::{
    AndroidHostProcessRole, AndroidProductCommand, AndroidProductCommandOutcome,
    AndroidProductCommandRequest,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_platform::android_bridge::get_android_bridge;
use lurker_core_contract::error::{ErrorCode, Failure};

/// Execute a Serde-tagged Rust product command in the `:vpn` owner and return
/// its typed terminal outcome. The UI process forwards the unchanged payload.
#[uniffi::export]
pub async fn android_execute_product_command_json(command_json: String) -> String {
    get_runtime()
        .spawn(async move {
            let Some(bridge) = get_android_bridge() else {
                return encode_outcome(AndroidProductCommandOutcome::Unavailable {
                    scope: None,
                    failure: Failure::new(
                        ErrorCode::NotReady,
                        "Android host bridge is unavailable",
                        true,
                    ),
                });
            };
            match bridge.process_role() {
                AndroidHostProcessRole::UiClient => {
                    return match bridge.execute_product_command_json(&command_json).await {
                        Ok(result) => result,
                        Err(error) => encode_outcome(AndroidProductCommandOutcome::Unavailable {
                            scope: None,
                            failure: Failure::new(ErrorCode::NotReady, error.to_string(), true),
                        }),
                    };
                }
                AndroidHostProcessRole::Unsupported => {
                    return encode_outcome(AndroidProductCommandOutcome::Unavailable {
                        scope: None,
                        failure: Failure::new(
                            ErrorCode::NotReady,
                            "Android product commands are unavailable in this process",
                            true,
                        ),
                    });
                }
                AndroidHostProcessRole::CoreOwner => {}
            }

            let request = match serde_json::from_str::<AndroidProductCommandRequest>(&command_json)
            {
                Ok(command) => command,
                Err(error) => {
                    return encode_outcome(AndroidProductCommandOutcome::Unavailable {
                        scope: None,
                        failure: Failure::new(
                            ErrorCode::InvalidInput,
                            format!("Android product command payload is invalid: {error}"),
                            false,
                        ),
                    });
                }
            };
            let core = match shared_core().await {
                Ok(core) => core,
                Err(status) => {
                    return encode_outcome(AndroidProductCommandOutcome::Unavailable {
                        scope: None,
                        failure: Failure::new(
                            ErrorCode::NotReady,
                            status.message.unwrap_or_else(|| {
                                "Android CoreApplication is unavailable".to_owned()
                            }),
                            true,
                        ),
                    });
                }
            };
            let intent = match request.command {
                AndroidProductCommand::Control(intent) => core.control.command_intent(intent),
                AndroidProductCommand::Application(intent) => *intent,
            };
            let outcome = core
                .application
                .execute_scoped(CoreCommandRequest {
                    scope: request.scope,
                    intent,
                })
                .await;
            encode_outcome(AndroidProductCommandOutcome::Terminal(Box::new(outcome)))
        })
        .await
        .unwrap_or_else(|error| {
            encode_outcome(AndroidProductCommandOutcome::Unavailable {
                scope: None,
                failure: Failure::new(
                    ErrorCode::Internal,
                    format!("Android product command task failed: {error}"),
                    true,
                ),
            })
        })
}

/// Return one serialized, owner-authored shared UI snapshot.
#[uniffi::export]
pub async fn android_surface_snapshot_json() -> FfiStringResult {
    get_runtime()
        .spawn(async move {
            let Some(bridge) = get_android_bridge() else {
                return FfiStringResult::err(
                    FfiErrorCode::NotReady,
                    "Android host bridge is unavailable",
                );
            };
            let snapshot = match bridge.process_role() {
                AndroidHostProcessRole::CoreOwner => owner_surface_snapshot_json().await,
                AndroidHostProcessRole::UiClient => bridge
                    .surface_snapshot_json()
                    .await
                    .map_err(|error| error.to_string()),
                AndroidHostProcessRole::Unsupported => {
                    Err("Android surface snapshots are unavailable in this process".to_owned())
                }
            };
            match snapshot {
                Ok(snapshot) => FfiStringResult::ok(Some(snapshot)),
                Err(error) => FfiStringResult::err(FfiErrorCode::NotReady, error),
            }
        })
        .await
        .unwrap_or_else(|error| {
            FfiStringResult::err(
                FfiErrorCode::Unknown,
                format!("Android surface request failed: {error}"),
            )
        })
}

fn encode_outcome(outcome: AndroidProductCommandOutcome) -> String {
    serde_json::to_string(&outcome).unwrap_or_else(|error| {
        let fallback = AndroidProductCommandOutcome::Unavailable {
            scope: None,
            failure: Failure::new(
                ErrorCode::Internal,
                format!("Android command result serialization failed: {error}"),
                true,
            ),
        };
        serde_json::to_string(&fallback).unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::android_surface_snapshot_json;
    use super::encode_outcome;
    use crate::ffi::FfiErrorCode;
    use infiltrator_contract::android_control::AndroidProductCommandOutcome;
    use infiltrator_contract::command::{CommandResult, RequestId};
    use infiltrator_contract::core_identity::CoreCommandOutcome;
    use infiltrator_platform::TEST_LOCK;
    use infiltrator_platform::android_bridge::{
        clear_android_bridge, get_android_bridge, set_android_bridge,
    };

    #[test]
    fn terminal_application_result_is_encoded_as_typed_json() {
        let json = encode_outcome(AndroidProductCommandOutcome::Terminal(Box::new(
            CoreCommandOutcome {
                scope: None,
                result: CommandResult::Completed {
                    request_id: RequestId(9),
                },
            },
        )));
        let Ok(AndroidProductCommandOutcome::Terminal(outcome)) =
            serde_json::from_str::<AndroidProductCommandOutcome>(&json)
        else {
            panic!("terminal result should decode");
        };
        let outcome = *outcome;
        assert!(matches!(
            outcome.result,
            CommandResult::Completed {
                request_id: RequestId(9)
            }
        ));
    }

    #[tokio::test]
    async fn missing_owner_snapshot_is_not_returned_as_an_empty_success() {
        let _guard = TEST_LOCK.lock().await;
        let previous_bridge = get_android_bridge();
        clear_android_bridge();

        let result = android_surface_snapshot_json().await;

        if let Some(bridge) = previous_bridge {
            set_android_bridge(bridge);
        }
        assert_eq!(result.status.code, FfiErrorCode::NotReady);
        assert!(result.value.is_none());
    }
}
