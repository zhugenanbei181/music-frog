use crate::host_support::get_runtime;
use crate::owner_command::execute_owner_commands;
use crate::uniffi_api::profile_age_credential_owner::execute_owner_request;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::profile_age_credential::{
    ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerRequest,
};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::age_action::execute_slint_age_action;
use infiltrator_slint::{SlintAction, SlintProductHost};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;

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
        self.execute_typed(snapshot, action).map(|_| ())
    }

    /// Runs one owner action and reports the owner's typed readback.
    ///
    /// The Android owner answers a product command with the typed output it
    /// produced, so the surface learns the owner's identity for a reviewed
    /// export, a restoration review or an applied subscription filter instead
    /// of learning only that a command was dispatched.
    fn execute_typed(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<CommandOutput, Failure> {
        match action {
            SlintAction::SetAutostart(_) => Err(Failure::unsupported(
                "Android autostart is controlled by the Android host, not the shared Slint surface",
            )),
            SlintAction::Command(intent) => {
                let scope = android_scope(snapshot)?;
                self.execute_commands(scope, vec![intent])
            }
            SlintAction::Sequence(intents) => {
                let scope = android_scope(snapshot)?;
                self.execute_commands(scope, intents)
            }
            action => {
                execute_slint_age_action(self, snapshot, action)?;
                Ok(CommandOutput::Unit)
            }
        }
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
    /// Runs the shared commands in the Android owner and reports the typed
    /// readback of the last one.
    fn execute_commands(
        &self,
        scope: CoreScope,
        intents: Vec<CommandIntent>,
    ) -> Result<CommandOutput, Failure> {
        let transport = |request_json: &str| -> Result<String, Failure> {
            Ok(
                get_runtime().block_on(crate::android_execute_product_command_json(
                    request_json.to_owned(),
                )),
            )
        };
        execute_owner_commands(scope, intents, &transport)
    }
}

/// The Core scope the Android owner snapshot carries.
fn android_scope(snapshot: Option<&SurfaceSnapshot>) -> Result<CoreScope, Failure> {
    snapshot
        .and_then(|snapshot| snapshot.core.scope())
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "The Android owner snapshot has no active Core identity",
                true,
            )
        })
}
