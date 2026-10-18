//! Standard unidirectional UI command pipeline sink port (PUB-004.2).
//!
//! Charter: All UI surfaces (Iced, Bevy, Slint, Android) interact with the
//! core exclusively through typed read observations ([`crate::surface::SurfaceReader`])
//! and typed intent dispatch ([`CommandSink`]). Direct side-effects or
//! core-level mutations from UI surfaces are strictly forbidden.

use async_trait::async_trait;
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use lurker_core_ports::error::PortError;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Standard abstract sink consuming typed UI command intents.
///
/// Implementations dispatch user actions to the core actor while preserving
/// monotonic generation/revision sequencing and returning terminal readbacks.
#[async_trait]
pub trait CommandSink: Send + Sync {
    /// Submit a command intent for asynchronous execution.
    async fn submit(&self, intent: CommandIntent) -> Result<CommandResult, PortError>;

    /// Submit a tracked command intent, returning an exact RequestId for UI correlation.
    async fn submit_tracked(
        &self,
        intent: CommandIntent,
    ) -> Result<(RequestId, CommandResult), PortError>;
}

/// An in-memory, thread-safe recording command sink for headless tests and verification.
#[derive(Default)]
pub struct RecordingCommandSink {
    next_id: AtomicU64,
    history: Mutex<Vec<CommandIntent>>,
    responses: Mutex<VecDeque<Result<CommandResult, PortError>>>,
}

impl RecordingCommandSink {
    /// Create a fresh recording sink.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            history: Mutex::new(Vec::new()),
            responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Pre-enqueue a response to be returned by subsequent calls to submit / submit_tracked.
    pub fn enqueue_response(&self, response: Result<CommandResult, PortError>) {
        self.responses
            .lock()
            .expect("recording sink poisoned")
            .push_back(response);
    }

    /// Retrieve all recorded command intents submitted to this sink.
    pub fn recorded_intents(&self) -> Vec<CommandIntent> {
        self.history
            .lock()
            .expect("recording sink poisoned")
            .clone()
    }
}

#[async_trait]
impl CommandSink for RecordingCommandSink {
    async fn submit(&self, intent: CommandIntent) -> Result<CommandResult, PortError> {
        let id = RequestId(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.history
            .lock()
            .expect("recording sink poisoned")
            .push(intent);
        if let Some(resp) = self
            .responses
            .lock()
            .expect("recording sink poisoned")
            .pop_front()
        {
            resp
        } else {
            Ok(CommandResult::Completed { request_id: id })
        }
    }

    async fn submit_tracked(
        &self,
        intent: CommandIntent,
    ) -> Result<(RequestId, CommandResult), PortError> {
        let id = RequestId(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.history
            .lock()
            .expect("recording sink poisoned")
            .push(intent);
        let res = if let Some(resp) = self
            .responses
            .lock()
            .expect("recording sink poisoned")
            .pop_front()
        {
            resp?
        } else {
            CommandResult::Completed { request_id: id }
        };
        Ok((id, res))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recording_command_sink_records_and_dispatches_in_order() {
        let sink = RecordingCommandSink::new();
        sink.submit(CommandIntent::StartCore).await.unwrap();
        sink.submit(CommandIntent::StopCore).await.unwrap();

        let (req_id, res) = sink
            .submit_tracked(CommandIntent::RestartCore)
            .await
            .unwrap();
        assert_eq!(req_id, RequestId(3));
        assert_eq!(
            res,
            CommandResult::Completed {
                request_id: RequestId(3)
            }
        );

        let intents = sink.recorded_intents();
        assert_eq!(intents.len(), 3);
        assert_eq!(intents[0], CommandIntent::StartCore);
        assert_eq!(intents[1], CommandIntent::StopCore);
        assert_eq!(intents[2], CommandIntent::RestartCore);
    }

    #[tokio::test]
    async fn recording_command_sink_returns_prequeued_errors() {
        let sink = RecordingCommandSink::new();
        sink.enqueue_response(Err(PortError::Failed("core service unavailable".into())));

        let err = sink.submit(CommandIntent::StartCore).await.unwrap_err();
        assert!(matches!(err, PortError::Failed(_)));
    }
}
