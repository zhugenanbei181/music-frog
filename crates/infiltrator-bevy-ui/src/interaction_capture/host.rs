//! Capture fixtures execute the shared command handler and reject unrelated side effects.
use super::outbound_preferences::OutboundPreferenceCaptureOwner;
use crate::command::{UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::command_application::handler::CommandHandler;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceIntent;
use mem::take;
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub(super) enum CaptureCapability {
    ImportUri,
    GroupExpansion,
    GroupOrder,
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
    outbound_preferences: Option<OutboundPreferenceCaptureOwner>,
}
impl CaptureCommandSink {
    pub fn new(application: CommandApplication, capability: CaptureCapability) -> Self {
        Self {
            application,
            capability,
            next_request: AtomicU64::new(0),
            completed: Arc::new(Mutex::new(Vec::new())),
            outbound_preferences: None,
        }
    }

    pub fn with_outbound_preferences(
        application: CommandApplication,
        capability: CaptureCapability,
        outbound_preferences: OutboundPreferenceCaptureOwner,
    ) -> Self {
        Self {
            application,
            capability,
            next_request: AtomicU64::new(0),
            completed: Arc::new(Mutex::new(Vec::new())),
            outbound_preferences: Some(outbound_preferences),
        }
    }
}
impl UiCommandSink for CaptureCommandSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        let allowed = match (&self.capability, &command) {
            (
                CaptureCapability::GroupExpansion,
                UiCommand::ApplyOutboundUiPreference { request },
            ) => matches!(
                request.intent(),
                OutboundUiPreferenceIntent::SetGroupExpansion { .. }
            ),
            (CaptureCapability::Search, UiCommand::ApplyOutboundUiPreference { request }) => {
                matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::SetSearchQuery(_)
                )
            }
            (CaptureCapability::GroupOrder, UiCommand::ApplyOutboundUiPreference { request }) => {
                matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::ReorderGroups { .. }
                        | OutboundUiPreferenceIntent::ResetGroupOrder { .. }
                )
            }
            (capability, command) => matches!(
                (capability, command),
                (
                    CaptureCapability::SnapshotRestore,
                    UiCommand::SnapshotRestore { .. }
                ) | (
                    CaptureCapability::ImportUri,
                    UiCommand::ImportCustomNodeUri { .. }
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
        };
        assert!(allowed, "capture fixture refuses unrelated side effects");
        let request_id = RequestId(self.next_request.fetch_add(1, Ordering::Relaxed) + 1);
        if let (Some(owner), UiCommand::ApplyOutboundUiPreference { request }) =
            (&self.outbound_preferences, &command)
        {
            let result = owner
                .apply(request)
                .map(CommandOutput::OutboundUiPreferencesApplied);
            self.completed
                .lock()
                .expect("capture feedback")
                .push(CommandExecutedEvent {
                    command,
                    request_id,
                    result,
                });
            return Some(request_id);
        }
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
