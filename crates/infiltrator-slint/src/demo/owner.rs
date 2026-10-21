//! The demo owner's answers to the commands the capture's gestures send.
//!
//! The capture drives the real product callbacks, so the demo host has to
//! answer the shared commands the business sends. The answers are typed
//! failures of the shared contract: the surface then shows the owner's
//! refusal in the business's own panel and in the owner-visible notice, so
//! the receipt proves that the business really ran and never that a route
//! changed.

use crate::{CommandIntent, CommandOutput, SlintAction};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};

/// Answers one owner action the way the capture's business needs it.
pub fn respond(
    _snapshot: &mut SurfaceSnapshot,
    action: SlintAction,
) -> Result<CommandOutput, Failure> {
    match action {
        SlintAction::Command(intent) => Err(refusal(&intent)),
        SlintAction::Sequence(intents) => match intents.first() {
            Some(intent) => Err(refusal(intent)),
            None => Ok(CommandOutput::Unit),
        },
        _ => Ok(CommandOutput::Unit),
    }
}

/// The typed refusal the demo owner answers a business command with.
///
/// The message names the business that refused, so the receipt shows which
/// command ran.
fn refusal(intent: &CommandIntent) -> Failure {
    let message = match intent {
        CommandIntent::TestDnsLeak => "the leak probe could not reach the configured resolvers",
        CommandIntent::QueryDns { .. } => "the upstream resolver refused the query",
        CommandIntent::ClearDnsCache { .. } => "the core refused to flush the Fake-IP cache",
        CommandIntent::ApplyDnsSettings { .. } => "the core refused the Hosts draft",
        CommandIntent::RunScriptSandbox { .. } => "the sandbox refused the script",
        CommandIntent::PrepareScriptExport { .. } => {
            "the export artifact could not be composed for the review"
        }
        CommandIntent::LoadSnapshotDiff { .. } => "the stored difference could not be read",
        CommandIntent::PrepareSnapshotRestore { .. } => {
            "the stored snapshot could not be reviewed for the restoration"
        }
        CommandIntent::SaveSubscriptionFilter { .. } => {
            "the subscription filter could not be saved"
        }
        CommandIntent::PreviewProfileAggregation { .. } => {
            "the aggregation preview could not be computed"
        }
        CommandIntent::CreateAggregatedProfile { .. } => {
            "the aggregated profile could not be created"
        }
        CommandIntent::RunDoctorDiagnostics => "the diagnostic run did not reach the core",
        CommandIntent::BootstrapDoctor => "the engine could not be initialised again",
        CommandIntent::ResolveConflictKeepLocal | CommandIntent::ResolveConflictTakeRemote => {
            "the synchronisation conflict could not be resolved"
        }
        CommandIntent::SetAppRule { .. } => "the routing rule could not be applied",
        _ => "the owner refused the command",
    };
    Failure::new(ErrorCode::Internal, message, true)
}
