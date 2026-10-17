//! Scoped command contract shared by application entry points and host surfaces.

use crate::command_capability::{CommandCapabilityRoute, classify_command};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::profile_document::DocumentInspection;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::profile_mixin_studio::{
    ProfileMixinStudioObservation, ProfileMixinToggleId,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use std::future::Future;
use std::pin::Pin;

pub type CommandFuture = Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'static>>;
pub type CommandOutputFuture =
    Pin<Box<dyn Future<Output = Result<CommandOutput, Failure>> + Send + 'static>>;

/// Scope token issued only by CoreApplication after it has checked the
/// request against the owner-authored lifecycle snapshot.
pub struct ValidatedCoreCommandScope(Option<CoreScope>);

impl ValidatedCoreCommandScope {
    pub(crate) fn from_core_application(scope: Option<CoreScope>) -> Self {
        Self(scope)
    }

    pub(crate) fn as_ref(&self) -> Option<&CoreScope> {
        self.0.as_ref()
    }
}

/// Token for the narrow child-command path used inside a CoreApplication
/// lifecycle transaction (VPN authorization/start/stop).
pub struct CoreLifecycleChildPermit(());

impl CoreLifecycleChildPermit {
    pub(crate) fn from_core_application() -> Self {
        Self(())
    }
}

/// Extension point consumed by CoreApplication for commands beyond lifecycle
/// and routing intent.
pub trait CommandHandler: Send + Sync {
    fn handle(&self, intent: CommandIntent) -> CommandFuture;

    /// Pure editor codec calls bypass the asynchronous Core command queue.
    /// Their document identity is the engine/format/schema fence, and the
    /// implementations must not touch profile storage or a running runtime.
    fn inspect_profile_buffer(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
    ) -> Result<DocumentInspection, Failure> {
        Err(Failure::unsupported(
            "profile document inspection is not composed",
        ))
    }
    fn insert_profile_snippet(
        &self,
        _identity: &ProfileDocumentIdentity,
        _content: &str,
        _snippet_id: &str,
        _line: usize,
        _column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        Err(Failure::unsupported(
            "profile document snippet insertion is not composed",
        ))
    }
    fn inspect_mixin_draft(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<(), Failure> {
        Err(Failure::unsupported(
            "profile Mixin draft inspection is not composed",
        ))
    }
    fn inspect_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        Err(Failure::unsupported(
            "Mixin toggle inspection is not composed",
        ))
    }
    fn set_mixin_toggle(
        &self,
        _identity: &ProfileDocumentIdentity,
        _mixin_yaml: &str,
        _toggle_id: ProfileMixinToggleId,
        _enabled: bool,
    ) -> Result<String, Failure> {
        Err(Failure::unsupported("Mixin toggle editing is not composed"))
    }
    fn inspect_mixin_studio(
        &self,
        _identity: &ProfileDocumentIdentity,
        _document_content: &str,
        _mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioObservation, Failure> {
        Err(Failure::unsupported(
            "Mixin studio observation is not composed",
        ))
    }

    fn handle_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandFuture {
        match classify_command(&intent) {
            CommandCapabilityRoute::LocalOnly => self.handle(intent),
            CommandCapabilityRoute::CoreBackend(_) => Box::pin(async {
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "command handler has no engine-scoped capability gate",
                    true,
                ))
            }),
            CommandCapabilityRoute::HostOnly(_) => Box::pin(async {
                Err(Failure::unsupported(
                    "command handler has no host capability gate",
                ))
            }),
            CommandCapabilityRoute::LifecycleOwned(_) | CommandCapabilityRoute::LifecycleChild => {
                Box::pin(async {
                    Err(Failure::unsupported(
                        "lifecycle commands must use a CoreApplication lifecycle route",
                    ))
                })
            }
        }
    }
    fn handle_output(&self, intent: CommandIntent) -> CommandOutputFuture {
        if classify_command(&intent) != CommandCapabilityRoute::LocalOnly {
            return Box::pin(async {
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "unscoped command handler output is denied without an explicit gate",
                    true,
                ))
            });
        }
        if matches!(
            intent,
            CommandIntent::SaveSubscriptionFilter { .. }
                | CommandIntent::RunScriptSandbox { .. }
                | CommandIntent::ClearScriptSandbox { .. }
                | CommandIntent::PrepareScriptExport { .. }
                | CommandIntent::SaveScriptExport { .. }
                | CommandIntent::PrepareSnapshotRestore { .. }
                | CommandIntent::ConfirmSnapshotRestore { .. }
                | CommandIntent::CancelSnapshotRestore { .. }
                | CommandIntent::CreateBackupSnapshot { .. }
                | CommandIntent::LoadSnapshotHistory { .. }
                | CommandIntent::LoadSnapshotDiff { .. }
                | CommandIntent::PruneSnapshots { .. }
                | CommandIntent::LoadProfileOptions { .. }
                | CommandIntent::LoadProfileDocument { .. }
                | CommandIntent::SaveProfileDocument { .. }
                | CommandIntent::SaveMixinOverlay { .. }
                | CommandIntent::PrepareLogExport
                | CommandIntent::SaveLogExport { .. }
                | CommandIntent::ResetRuleHitCounters { .. }
        ) {
            return Box::pin(async {
                Err(Failure::unsupported(
                    "Requested typed output is unsupported by this command handler",
                ))
            });
        }
        let future = self.handle(intent);
        Box::pin(async move { future.await.map(|()| CommandOutput::Unit) })
    }
    fn handle_output_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        match classify_command(&intent) {
            CommandCapabilityRoute::LocalOnly => self.handle_output(intent),
            CommandCapabilityRoute::CoreBackend(_) => Box::pin(async {
                Err(Failure::new(
                    ErrorCode::NotReady,
                    "command handler has no engine-scoped capability gate",
                    true,
                ))
            }),
            CommandCapabilityRoute::HostOnly(_) => Box::pin(async {
                Err(Failure::unsupported(
                    "command handler has no host capability gate",
                ))
            }),
            CommandCapabilityRoute::LifecycleOwned(_) | CommandCapabilityRoute::LifecycleChild => {
                Box::pin(async {
                    Err(Failure::unsupported(
                        "lifecycle commands must use a CoreApplication lifecycle route",
                    ))
                })
            }
        }
    }
    fn handle_lifecycle_child_output(
        &self,
        _permit: CoreLifecycleChildPermit,
        _scope: ValidatedCoreCommandScope,
        _intent: CommandIntent,
    ) -> CommandOutputFuture {
        Box::pin(async {
            Err(Failure::unsupported(
                "command handler has no explicit lifecycle-child route",
            ))
        })
    }
}
