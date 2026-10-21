//! Capture fixtures execute the shared command handler and reject unrelated side effects.
use super::capability::capture_scoped_sink;
use super::outbound_preferences::OutboundPreferenceCaptureOwner;
use crate::command::{UiCommand, UiCommandSink};
use crate::command_events::CommandExecutedEvent;
use crate::command_execution::ApplicationCommandSink;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::command_application::handler::CommandHandler;
use infiltrator_application::core_application::CoreApplication;
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
    DoctorBootstrap,
    DnsLeak,
    DnsQuery,
    Hosts,
    RuleTrace,
    RuleList,
    SnapshotRestore,
    AppRouting,
    Script,
}

/// The route one capture command travels.
enum CaptureRoute {
    /// The unscoped shared command bus: a local-only or host-only command.
    Unscoped(CommandApplication),
    /// The Core owner's scoped route: a command that needs a validated scope.
    Scoped(ApplicationCommandSink),
}

pub(super) struct CaptureCommandSink {
    route: CaptureRoute,
    capability: CaptureCapability,
    next_request: AtomicU64,
    completed: Arc<Mutex<Vec<CommandExecutedEvent>>>,
    outbound_preferences: Option<OutboundPreferenceCaptureOwner>,
}
impl CaptureCommandSink {
    pub fn new(application: CommandApplication, capability: CaptureCapability) -> Self {
        Self {
            route: CaptureRoute::Unscoped(application),
            capability,
            next_request: AtomicU64::new(0),
            completed: Arc::new(Mutex::new(Vec::new())),
            outbound_preferences: None,
        }
    }

    /// The scoped capture route: the surface's shared typed commands travel the
    /// Core owner's validated-scope route, exactly like the product host's
    /// commands do. A command the shared bus only admits with a scope (a Core
    /// backend command) needs this route; the unscoped route is denied.
    pub fn scoped(
        core: Arc<CoreApplication>,
        handler: CommandApplication,
        capability: CaptureCapability,
    ) -> Self {
        Self {
            route: CaptureRoute::Scoped(capture_scoped_sink(core, handler)),
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
            route: CaptureRoute::Unscoped(application),
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
                    | (
                        CaptureCapability::DoctorBootstrap,
                        UiCommand::BootstrapDoctor
                    )
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
                    | (CaptureCapability::AppRouting, UiCommand::SetAppRule { .. })
                    | (
                        CaptureCapability::Script,
                        UiCommand::RunScriptSandbox { .. }
                            | UiCommand::ClearScriptSandbox { .. }
                            | UiCommand::PrepareScriptExport { .. }
                            | UiCommand::SaveScriptExport { .. }
                            | UiCommand::CancelScriptExport { .. }
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
        match &self.route {
            CaptureRoute::Unscoped(application) => {
                let intent = command.to_intent().expect("typed capture intent");
                let application = application.clone();
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
            CaptureRoute::Scoped(sink) => sink.submit_tracked(command),
        }
    }
    fn drain_results(&self) -> Vec<CommandExecutedEvent> {
        let mut results = take(&mut *self.completed.lock().expect("capture feedback"));
        if let CaptureRoute::Scoped(sink) = &self.route {
            results.extend(sink.drain_results());
        }
        results
    }
}
