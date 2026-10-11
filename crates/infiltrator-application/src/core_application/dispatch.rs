//! Exact terminal acknowledgments survive the bounded lifecycle event ring.
use super::execution::closed_failure;
use super::{CoreApplication, DispatchedCommand, DispatchedMessage};
use infiltrator_contract::command::{CommandResult, RequestId};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreCommandRequest};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreEvent;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, TrySendError, channel};

impl CoreApplication {
    /// Execute only when the observed engine instance and activation are
    /// still current. A mismatch is a typed rejection with no side effects.
    pub async fn execute_scoped(&self, request: CoreCommandRequest) -> CoreCommandOutcome {
        let request_id = self.allocate_request_id();
        let result = self.execute_scoped_with_id(request_id, request).await;
        CoreCommandOutcome {
            scope: self.snapshot().scope(),
            result,
        }
    }

    /// Dispatch a command fenced to the Core scope most recently observed by
    /// the requesting surface. The terminal scope is retained in the reply,
    /// including when a lifecycle command advances the Core generation.
    pub fn dispatch_scoped_tracked(
        &self,
        request: CoreCommandRequest,
    ) -> (RequestId, Receiver<CoreCommandOutcome>) {
        let (sender, receiver) = channel();
        let id = self.enqueue(request, sender);
        (id, receiver)
    }

    pub(super) fn enqueue(
        &self,
        request: CoreCommandRequest,
        reply: Sender<CoreCommandOutcome>,
    ) -> RequestId {
        let request_id = self.allocate_request_id();
        let kind = request.intent.kind();
        let rejection_reply = reply.clone();
        let failure = if self.inner.closed.load(Ordering::Acquire) {
            closed_failure()
        } else {
            match self
                .inner
                .dispatch_tx
                .try_send(DispatchedMessage::Command(DispatchedCommand {
                    request_id,
                    request,
                    reply,
                })) {
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
            scope: self.snapshot().scope(),
            request_id,
            kind,
            failure: failure.clone(),
        });
        let _ = rejection_reply.send(CoreCommandOutcome {
            scope: self.snapshot().scope(),
            result: CommandResult::Rejected {
                request_id,
                failure,
            },
        });
        request_id
    }
}
