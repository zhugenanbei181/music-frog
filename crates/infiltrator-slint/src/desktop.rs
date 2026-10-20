use super::age_action::execute_slint_age_action;
use super::*;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::command::CommandResult;
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_desktop::product::DesktopProductSession;
use infiltrator_shared::autostart::{is_autostart_enabled, set_autostart_enabled};
use std::sync::Arc;

struct DesktopProductHost {
    application: Arc<CoreApplication>,
    pump: SurfacePump,
    _session: Arc<DesktopProductSession>,
}

impl SlintProductHost for DesktopProductHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self.pump.current())
    }

    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        self.execute_typed(snapshot, action).map(|_| ())
    }

    fn execute_typed(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<CommandOutput, Failure> {
        match action {
            SlintAction::SetAutostart(enabled) => {
                set_autostart_enabled("MusicFrogInfiltrator", enabled)
                    .map_err(|error| Failure::new(ErrorCode::Internal, error.to_string(), true))?;
                Ok(CommandOutput::Unit)
            }
            SlintAction::Command(intent) => {
                let snapshot = snapshot.ok_or_else(no_snapshot)?;
                self.execute_commands(snapshot.core.scope(), [intent])
            }
            SlintAction::Sequence(intents) => {
                let snapshot = snapshot.ok_or_else(no_snapshot)?;
                self.execute_commands(snapshot.core.scope(), intents)
            }
            action => {
                execute_slint_age_action(self, snapshot, action)?;
                Ok(CommandOutput::Unit)
            }
        }
    }

    fn autostart_enabled(&self) -> Option<bool> {
        Some(is_autostart_enabled("MusicFrogInfiltrator"))
    }
}

impl DesktopProductHost {
    /// Runs the shared commands and reports the typed readback of the last one.
    fn execute_commands(
        &self,
        scope: Option<CoreScope>,
        intents: impl IntoIterator<Item = CommandIntent>,
    ) -> Result<CommandOutput, Failure> {
        let mut scope = scope.ok_or_else(no_snapshot)?;
        let mut output = CommandOutput::Unit;
        for intent in intents {
            let (request_id, receiver) =
                self.application
                    .dispatch_scoped_tracked(CoreCommandRequest {
                        scope: scope.clone(),
                        intent,
                    });
            let outcome = receiver.recv().map_err(|_| {
                Failure::new(
                    ErrorCode::Internal,
                    format!("Core command {request_id:?} ended without a result"),
                    true,
                )
            })?;
            if let Some(next_scope) = outcome.scope {
                scope = next_scope;
            }
            match outcome.result {
                CommandResult::Completed { .. } | CommandResult::Accepted { .. } => {}
                CommandResult::Produced {
                    output: produced, ..
                } => output = *produced,
                CommandResult::Rejected { failure, .. } => return Err(failure),
            }
        }
        Ok(output)
    }
}

fn no_snapshot() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "The shared owner has not published an active Core identity",
        true,
    )
}

pub fn run() -> anyhow::Result<()> {
    let session = Arc::new(DesktopProductSession::open(SurfaceKind::SlintDesktop)?);
    let host = Arc::new(DesktopProductHost {
        application: session.application(),
        pump: session.surface_pump(),
        _session: session,
    });
    run_with_host(host).map_err(|error| anyhow::anyhow!(error.to_string()))
}
