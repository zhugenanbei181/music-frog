//! Product-level Android control orchestration shared by Compose and Bevy.

use crate::core_application::CoreApplication;
use infiltrator_contract::android_control::AndroidProductControlIntent;
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreCommandRequest};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use std::sync::mpsc::Receiver;

#[derive(Clone)]
pub struct AndroidControlApplication {
    core: CoreApplication,
}

impl AndroidControlApplication {
    pub fn new(core: CoreApplication) -> Self {
        Self { core }
    }

    pub fn command_intent(&self, intent: AndroidProductControlIntent) -> CommandIntent {
        command_for(intent)
    }

    /// Execute an Android product intent against the exact owner scope
    /// published to the requesting surface.
    pub async fn execute(
        &self,
        intent: AndroidProductControlIntent,
        scope: CoreScope,
    ) -> Result<CoreLifecycleSnapshot, Failure> {
        match self
            .core
            .execute_scoped(CoreCommandRequest {
                scope,
                intent: self.command_intent(intent),
            })
            .await
            .result
        {
            CommandResult::Completed { .. } | CommandResult::Produced { .. } => {
                Ok(self.core.snapshot().lifecycle_snapshot())
            }
            CommandResult::Rejected { failure, .. } => Err(failure),
            CommandResult::Accepted { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "Android control command did not reach a terminal result",
                true,
            )),
        }
    }

    /// Dispatch an Android product request carrying the requesting
    /// surface's observed Core scope.
    pub fn dispatch_scoped_tracked(
        &self,
        intent: AndroidProductControlIntent,
        scope: CoreScope,
    ) -> (CoreCommandRequest, RequestId, Receiver<CoreCommandOutcome>) {
        let request = CoreCommandRequest {
            scope,
            intent: self.command_intent(intent),
        };
        let (request_id, reply) = self.core.dispatch_scoped_tracked(request.clone());
        (request, request_id, reply)
    }

    /// Return a terminal, side-effect-free rejection when no accepted surface
    /// snapshot supplied a Core scope.
    pub fn reject_missing_scope(&self, intent: AndroidProductControlIntent) -> CoreCommandOutcome {
        self.core.reject_missing_scope(self.command_intent(intent))
    }
}

fn command_for(intent: AndroidProductControlIntent) -> CommandIntent {
    match intent {
        AndroidProductControlIntent::StartProxy => CommandIntent::StartProxy,
        AndroidProductControlIntent::ResumeKernel => CommandIntent::ResumeKernel,
        AndroidProductControlIntent::PauseKernel => CommandIntent::PauseKernel,
        AndroidProductControlIntent::RestartKernel => CommandIntent::RestartKernel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_intents_map_to_explicit_rust_commands() {
        assert_eq!(
            command_for(AndroidProductControlIntent::StartProxy),
            CommandIntent::StartProxy
        );
        assert_eq!(
            command_for(AndroidProductControlIntent::ResumeKernel),
            CommandIntent::ResumeKernel
        );
        assert_eq!(
            command_for(AndroidProductControlIntent::PauseKernel),
            CommandIntent::PauseKernel
        );
        assert_eq!(
            command_for(AndroidProductControlIntent::RestartKernel),
            CommandIntent::RestartKernel
        );
    }
}
