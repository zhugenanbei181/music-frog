//! Iced command ingress binds every intent to the last accepted shared
//! surface scope. Missing observations fail closed for Core, host, and local
//! intents alike.

use infiltrator_application::core_application::{CoreApplication, ScopedCoreCommandClient};
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreScope};
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document::DocumentInspection;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome;
use infiltrator_ports::profile_document_codec::{
    ProfileMixinStudioProjection, ProfileMixinToggleId,
};

#[derive(Clone)]
pub(crate) struct CoreCommandIngress {
    application: CoreApplication,
    scope: ObservedCoreScope,
}

#[derive(Clone, Default)]
pub(crate) enum ObservedCoreScope {
    Bound(ScopedCoreCommandClient),
    #[default]
    Unavailable,
}

impl CoreCommandIngress {
    pub(crate) fn new(application: CoreApplication, scope: ObservedCoreScope) -> Self {
        Self { application, scope }
    }

    pub(crate) fn snapshot(&self) -> CoreSnapshot {
        self.application.snapshot()
    }

    pub(crate) fn speedtest_snapshot(&self) -> SpeedtestSnapshot {
        self.application.speedtest_snapshot()
    }

    pub(crate) fn observed_scope(&self) -> Option<CoreScope> {
        match &self.scope {
            ObservedCoreScope::Bound(client) => Some(client.observed_scope().clone()),
            ObservedCoreScope::Unavailable => None,
        }
    }

    pub(crate) fn inspect_profile_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.application.inspect_profile_buffer(identity, content)
    }

    pub(crate) fn insert_profile_snippet(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.application
            .insert_profile_snippet(identity, content, snippet_id, line, column)
    }

    pub(crate) fn inspect_mixin_draft(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.application
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }

    pub(crate) fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.application
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    pub(crate) fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.application
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    pub(crate) fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, Failure> {
        self.application
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }

    pub(crate) async fn execute(&self, intent: CommandIntent) -> CommandResult {
        self.execute_outcome(intent).await.result
    }

    pub(crate) async fn execute_outcome(&self, intent: CommandIntent) -> CoreCommandOutcome {
        match &self.scope {
            ObservedCoreScope::Bound(client) => client.execute(intent).await,
            ObservedCoreScope::Unavailable => self.application.reject_missing_scope(intent),
        }
    }
}

#[cfg(test)]
#[path = "../tests/gui/command_ingress_tests.rs"]
mod tests;
