//! Exact terminal acknowledgments survive the bounded lifecycle event ring.
use super::execution::closed_failure;
use super::{CoreApplication, DispatchedCommand};
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreEvent;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, TrySendError, channel};

impl CoreApplication {
    /// Dispatch with a private acknowledgment channel; no UI or runtime types enter the API.
    pub fn dispatch_tracked(&self, intent: CommandIntent) -> (RequestId, Receiver<CommandResult>) {
        let (sender, receiver) = channel();
        let id = self.enqueue(intent, Some(sender));
        (id, receiver)
    }
    pub(super) fn enqueue(
        &self,
        intent: CommandIntent,
        reply: Option<Sender<CommandResult>>,
    ) -> RequestId {
        let request_id = self.allocate_request_id();
        let kind = intent.kind();
        let failure = if self.inner.closed.load(Ordering::Acquire) {
            closed_failure()
        } else {
            match self.inner.dispatch_tx.try_send(DispatchedCommand {
                request_id,
                intent,
                reply: reply.clone(),
            }) {
                Ok(()) => return request_id,
                Err(TrySendError::Full(_)) => Failure::new(
                    ErrorCode::Internal,
                    "application command queue is full",
                    true,
                ),
                Err(TrySendError::Disconnected(_)) => Failure::new(
                    ErrorCode::Internal,
                    "application worker is no longer available",
                    true,
                ),
            }
        };
        self.push_event(CoreEvent::CommandFailed {
            request_id,
            kind,
            failure: failure.clone(),
        });
        if let Some(reply) = reply {
            let _ = reply.send(CommandResult::Rejected {
                request_id,
                failure,
            });
        }
        request_id
    }
}
