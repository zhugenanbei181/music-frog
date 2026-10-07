//! Capture fixtures execute the shared command handler and reject unrelated side effects.
use crate::command::{UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use infiltrator_application::command_application::{CommandApplication, CommandHandler};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::RequestId;
use std::mem::take;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub(super) enum CaptureCapability {
    ImportUri,
    GroupExpansion,
    Search,
    Language,
    Doctor,
    DnsLeak,
    DnsQuery,
    Hosts,
    RuleTrace,
    RuleList,
    SnapshotRestore,
}

pub(super) struct CaptureCommandSink {
    application: CommandApplication,
    capability: CaptureCapability,
    next_request: AtomicU64,
    completed: Arc<Mutex<Vec<CommandExecutedEvent>>>,
}
impl CaptureCommandSink {
    pub fn new(application: CommandApplication, capability: CaptureCapability) -> Self {
        Self {
            application,
            capability,
            next_request: AtomicU64::new(0),
            completed: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
impl UiCommandSink for CaptureCommandSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        assert!(
            matches!(
                (&self.capability, &command),
                (
                    CaptureCapability::SnapshotRestore,
                    UiCommand::SnapshotRestore { .. }
                ) | (
                    CaptureCapability::ImportUri,
                    UiCommand::ImportCustomNodeUri { .. }
                ) | (
                    CaptureCapability::GroupExpansion,
                    UiCommand::ToggleProxyGroupExpand { .. }
                ) | (
                    CaptureCapability::Search,
                    UiCommand::SetProxySearchQuery { .. }
                ) | (CaptureCapability::Language, UiCommand::SetLanguage { .. })
                    | (CaptureCapability::Doctor, UiCommand::RunDoctorDiagnostics)
                    | (CaptureCapability::DnsLeak, UiCommand::TestDnsLeak)
                    | (CaptureCapability::DnsQuery, UiCommand::QueryDns { .. })
                    | (CaptureCapability::Hosts, UiCommand::ApplyDnsSettings { .. })
                    | (
                        CaptureCapability::RuleTrace,
                        UiCommand::SimulateRuleTrace { .. }
                    )
                    | (
                        CaptureCapability::RuleList,
                        UiCommand::CommitRuleList { .. }
                    )
            ),
            "capture fixture refuses unrelated side effects"
        );
        let request_id = RequestId(self.next_request.fetch_add(1, Ordering::Relaxed) + 1);
        let intent = command.to_intent().expect("typed capture intent");
        let application = self.application.clone();
        let completed = self.completed.clone();
        tokio_application_runtime()
            .expect("capture runtime")
            .block_on(Box::pin(async move {
                let result = application.handle_output(intent).await;
                completed
                    .lock()
                    .expect("capture feedback")
                    .push(CommandExecutedEvent {
                        command,
                        request_id,
                        result,
                    });
            }));
        Some(request_id)
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        take(&mut *self.completed.lock().expect("capture feedback"))
    }
}
