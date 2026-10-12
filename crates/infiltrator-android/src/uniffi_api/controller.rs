//! Owner-routed Android controller operations and the unrelated public-IP probe.

use super::product::android_execute_product_command_json;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::shared_core;
use crate::host_support::{get_runtime, map_application_failure, network_application};
use infiltrator_contract::android_control::{
    AndroidHostProcessRole, AndroidProductCommand, AndroidProductCommandOutcome,
    AndroidProductCommandRequest,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_platform::android_bridge::get_android_bridge;

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpCheckResult {
    pub ip: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpResult {
    pub status: FfiStatus,
    pub value: Option<IpCheckResult>,
}

/// Public-IP probing is not a Mihomo controller read and remains an
/// independent network application operation.
#[uniffi::export]
pub async fn ip_check() -> IpResult {
    get_runtime()
        .spawn(async move {
            match fetch_ip_check().await {
                Ok(value) => IpResult {
                    status: FfiStatus::ok(),
                    value: Some(value),
                },
                Err(status) => IpResult {
                    status,
                    value: None,
                },
            }
        })
        .await
        .unwrap_or_else(|error| IpResult {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("runtime join error: {error}"),
            ),
            value: None,
        })
}

/// Close one connection through the owner-authored CoreApplication scope.
#[uniffi::export]
pub async fn connection_close(id: String) -> FfiStatus {
    let id = id.trim().to_owned();
    if id.is_empty() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "connection id is empty");
    }
    execute_owner_application_command(CommandIntent::CloseConnection { id }).await
}

/// Close all connections through the owner-authored CoreApplication scope.
#[uniffi::export]
pub async fn connections_close_all() -> FfiStatus {
    execute_owner_application_command(CommandIntent::CloseAllConnections).await
}

pub(super) async fn execute_owner_application_command(intent: CommandIntent) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let scope = match current_core_scope().await {
                Ok(scope) => scope,
                Err(status) => return status,
            };
            let request = AndroidProductCommandRequest {
                scope,
                command: AndroidProductCommand::Application(Box::new(intent)),
            };
            let request_json = match serde_json::to_string(&request) {
                Ok(value) => value,
                Err(error) => {
                    return FfiStatus::err(
                        FfiErrorCode::InvalidInput,
                        format!("Android application command serialization failed: {error}"),
                    );
                }
            };
            let outcome_json = android_execute_product_command_json(request_json).await;
            map_owner_command_outcome(&outcome_json)
        })
        .await
        .unwrap_or_else(|error| {
            FfiStatus::err(
                FfiErrorCode::Unknown,
                format!("Android owner command task failed: {error}"),
            )
        })
}

async fn current_core_scope() -> Result<CoreScope, FfiStatus> {
    let bridge = get_android_bridge().ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::NotReady, "Android host bridge is unavailable")
    })?;
    let role = match bridge.process_role() {
        AndroidHostProcessRole::CoreOwner => CoreProcessRole::Owner,
        AndroidHostProcessRole::UiClient => CoreProcessRole::Client,
        AndroidHostProcessRole::Unsupported => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android core commands are unavailable in this process",
            ));
        }
    };
    let scope = match role {
        CoreProcessRole::Owner => shared_core().await?.application.snapshot().scope(),
        CoreProcessRole::Client => bridge
            .core_lifecycle_snapshot()
            .await
            .map_err(|error| FfiStatus::err(FfiErrorCode::NotReady, error.to_string()))?
            .scope(),
    };
    scope.ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android core identity is unavailable; command was not sent",
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreProcessRole {
    Owner,
    Client,
}

fn map_owner_command_outcome(outcome_json: &str) -> FfiStatus {
    match serde_json::from_str::<AndroidProductCommandOutcome>(outcome_json) {
        Ok(AndroidProductCommandOutcome::Terminal(outcome)) => {
            let outcome = *outcome;
            match outcome.result {
                CommandResult::Completed { .. } | CommandResult::Produced { .. } => FfiStatus::ok(),
                CommandResult::Rejected { failure, .. } => map_application_failure(failure),
                CommandResult::Accepted { .. } => FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    "Android owner returned a non-terminal application command result",
                ),
            }
        }
        Ok(AndroidProductCommandOutcome::Unavailable { failure, .. }) => {
            map_application_failure(failure)
        }
        Err(error) => FfiStatus::err(
            FfiErrorCode::NotReady,
            format!("Android owner returned an invalid command result: {error}"),
        ),
    }
}

async fn fetch_ip_check() -> Result<IpCheckResult, FfiStatus> {
    let snapshot = network_application()
        .probe_public_ip(None)
        .await
        .map_err(map_application_failure)?;
    Ok(IpCheckResult {
        ip: snapshot.ip,
        country: snapshot.country,
        region: snapshot.region,
        city: snapshot.city,
    })
}

#[cfg(test)]
mod tests {
    use super::map_owner_command_outcome;
    use crate::ffi::FfiErrorCode;
    use infiltrator_contract::android_control::AndroidProductCommandOutcome;
    use infiltrator_contract::command::{CommandResult, RequestId};
    use infiltrator_contract::core_identity::CoreCommandOutcome;
    use infiltrator_contract::error::{ErrorCode, Failure};

    #[test]
    fn stale_owner_command_result_is_not_reported_as_success() {
        let outcome = AndroidProductCommandOutcome::Terminal(Box::new(CoreCommandOutcome {
            scope: None,
            result: CommandResult::Rejected {
                request_id: RequestId(12),
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "core scope changed before command execution",
                    true,
                ),
            },
        }));
        let encoded = serde_json::to_string(&outcome).expect("encode typed owner result");

        let status = map_owner_command_outcome(&encoded);

        assert_eq!(status.code, FfiErrorCode::NotReady);
        assert!(status.message.unwrap().contains("scope changed"));
    }
}
