use crate::host_support::get_runtime;
use crate::uniffi_api::profile_age_credential_owner::execute_owner_request;
use infiltrator_contract::android_control::{
    AndroidProductCommand, AndroidProductCommandOutcome, AndroidProductCommandRequest,
};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreScope};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_age_credential::{
    ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerRequest,
};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::age_action::execute_slint_age_action;
use infiltrator_slint::{SlintAction, SlintProductHost};

pub(super) struct AndroidSlintHost;

impl SlintProductHost for AndroidSlintHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        let result = get_runtime().block_on(crate::android_surface_snapshot_json());
        if result.status.code != crate::FfiErrorCode::Ok {
            return Err(Failure::new(
                ErrorCode::NotReady,
                result.status.message.unwrap_or_else(|| {
                    "Android owner has not published a surface snapshot".to_owned()
                }),
                true,
            ));
        }
        let json = result.value.ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Android owner returned no surface snapshot",
                true,
            )
        })?;
        serde_json::from_str(&json).map_err(|error| {
            Failure::new(
                ErrorCode::Internal,
                format!("Android owner snapshot could not be decoded: {error}"),
                true,
            )
        })
    }

    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        let intents = match action {
            SlintAction::SetAutostart(_) => {
                return Err(Failure::unsupported(
                    "Android autostart is controlled by the Android host, not the shared Slint surface",
                ));
            }
            SlintAction::Command(intent) => vec![intent],
            SlintAction::Sequence(intents) => intents,
            action @ (SlintAction::ImportEncryptedSubscription { .. }
            | SlintAction::SetProfileAgeCredential { .. }
            | SlintAction::ClearProfileAgeCredential { .. }
            | SlintAction::SaveProfileSettings { .. }
            | SlintAction::ImportProfileFile { .. }
            | SlintAction::ImportOverrideFile { .. }
            | SlintAction::ExportProfile { .. }
            | SlintAction::ExportOverride { .. }
            | SlintAction::ExportLogs { .. }
            | SlintAction::StartRuntime
            | SlintAction::ScanQrCode
            | SlintAction::OpenAutostartSettings
            | SlintAction::RequestBatteryOptimizationExemption
            | SlintAction::SetHideBackgroundCard(_)
            | SlintAction::SetReleaseUiWhenBackground(_)
            | SlintAction::SetShowTrafficNotification(_)
            | SlintAction::CacheBackPreview
            | SlintAction::CommitBackPreview) => {
                return execute_slint_age_action(self, snapshot, action);
            }
        };
        let mut scope = snapshot
            .and_then(|snapshot| snapshot.core.scope())
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "The Android owner snapshot has no active Core identity",
                    true,
                )
            })?;
        let command_count = intents.len();
        for (index, intent) in intents.into_iter().enumerate() {
            match self.execute_one(scope.clone(), intent)? {
                Some(next_scope) => scope = next_scope,
                None if index + 1 < command_count => {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "The Android owner retired the Core scope during a command sequence",
                        true,
                    ));
                }
                None => {}
            }
        }
        Ok(())
    }

    fn autostart_enabled(&self) -> Option<bool> {
        None
    }

    fn execute_age_credential_request(
        &self,
        request: ProfileAgeCredentialOwnerRequest,
    ) -> Result<ProfileAgeCredentialOwnerReadback, Failure> {
        get_runtime()
            .block_on(execute_owner_request(request))
            .map_err(|status| {
                Failure::new(
                    ErrorCode::Internal,
                    status
                        .message
                        .unwrap_or_else(|| "Age credential operation failed".to_owned()),
                    true,
                )
            })
    }
}

impl AndroidSlintHost {
    fn execute_one(
        &self,
        scope: CoreScope,
        intent: CommandIntent,
    ) -> Result<Option<CoreScope>, Failure> {
        let request = AndroidProductCommandRequest {
            scope,
            command: AndroidProductCommand::Application(Box::new(intent)),
        };
        let command_json = serde_json::to_string(&request).map_err(|error| {
            Failure::new(
                ErrorCode::Internal,
                format!("Android command could not be encoded: {error}"),
                false,
            )
        })?;
        let result =
            get_runtime().block_on(crate::android_execute_product_command_json(command_json));
        match serde_json::from_str::<AndroidProductCommandOutcome>(&result).map_err(|error| {
            Failure::new(
                ErrorCode::Internal,
                format!("Android owner command result could not be decoded: {error}"),
                true,
            )
        })? {
            AndroidProductCommandOutcome::Unavailable { failure, .. } => Err(failure),
            AndroidProductCommandOutcome::Terminal(outcome) => {
                let CoreCommandOutcome { scope, result } = *outcome;
                match result {
                    CommandResult::Completed { .. }
                    | CommandResult::Produced { .. }
                    | CommandResult::Accepted { .. } => Ok(scope),
                    CommandResult::Rejected { failure, .. } => Err(failure),
                }
            }
        }
    }
}
