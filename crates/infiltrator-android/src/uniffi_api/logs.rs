//! Log mutations routed through the Android Core owner.

use super::controller::execute_owner_application_command;
use crate::ffi::FfiStatus;
use infiltrator_contract::command::CommandIntent;

/// Clear the Core-owned log projection through the same typed command path as
/// desktop and Bevy clients. Reads and stream state come from the owner-authored
/// `SurfaceSnapshot`; this module keeps no process-local log buffer.
#[uniffi::export]
pub async fn logs_clear() -> FfiStatus {
    execute_owner_application_command(CommandIntent::ClearLogs).await
}
