//! Typed output routing validates actual results rather than unit acknowledgements.
use super::{
    CommandApplication, CommandFuture, CommandHandler, CommandOutputFuture,
    CoreLifecycleChildPermit, ValidatedCoreCommandScope,
};
use crate::command_capability::{CommandCapabilityRoute, classify_command};
use crate::core_capability_application::CoreCapabilityGateFailure;
use crate::routing_intent_application::RoutingIntentApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::ProfileDocumentSaved;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOutboundFacts, RuleTraceOwnerBinding, ScopedRuleTraceRequest,
};

impl CommandApplication {
    pub async fn execute_output(&self, intent: CommandIntent) -> Result<CommandOutput, Failure> {
        self.execute_output_with_scope(None, intent).await
    }

    async fn execute_output_with_scope(
        &self,
        scope: Option<&CoreScope>,
        intent: CommandIntent,
    ) -> Result<CommandOutput, Failure> {
        match classify_command(&intent) {
            CommandCapabilityRoute::CoreBackend(requirement) => {
                self.core_capabilities
                    .authorize(scope, requirement)
                    .map_err(CoreCapabilityGateFailure::into_failure)?;
            }
            CommandCapabilityRoute::LocalOnly => {}
            CommandCapabilityRoute::HostOnly(capability) => {
                self.authorize_host_capability(capability)?;
            }
            CommandCapabilityRoute::LifecycleOwned(_) | CommandCapabilityRoute::LifecycleChild => {
                return Err(Failure::unsupported(
                    "lifecycle command must be routed through CoreApplication",
                ));
            }
        }
        self.execute_output_dispatch(scope, intent).await
    }

    async fn execute_lifecycle_child_output(
        &self,
        intent: CommandIntent,
    ) -> Result<CommandOutput, Failure> {
        if classify_command(&intent) != CommandCapabilityRoute::LifecycleChild {
            return Err(Failure::unsupported(
                "only a CoreApplication lifecycle child can use this command route",
            ));
        }
        self.execute_output_dispatch(None, intent).await
    }

    async fn execute_output_dispatch(
        &self,
        scope: Option<&CoreScope>,
        intent: CommandIntent,
    ) -> Result<CommandOutput, Failure> {
        match intent {
            CommandIntent::SetRoutingIntent { intent } => {
                let scope = required_routing_scope(scope)?;
                let port = self.routing_intent_port.as_ref().ok_or_else(|| {
                    Failure::unsupported("routing intent adapter is not composed")
                })?;
                RoutingIntentApplication::new(port.clone(), self.core_capabilities.clone())
                    .apply(Some(scope), intent)
                    .await
                    .map(|observed| CommandOutput::RoutingIntentApplied { observed })
            }
            CommandIntent::GetRoutingIntent => {
                let scope = required_routing_scope(scope)?;
                let port = self.routing_intent_port.as_ref().ok_or_else(|| {
                    Failure::unsupported("routing intent adapter is not composed")
                })?;
                let snapshot =
                    RoutingIntentApplication::new(port.clone(), self.core_capabilities.clone())
                        .observe(Some(scope))
                        .await;
                Ok(CommandOutput::RoutingIntentObserved { snapshot })
            }
            CommandIntent::ReadOutboundInventory => {
                let scope = required_core_scope(scope)?;
                self.outbound()?
                    .inventory(scope.clone())
                    .await
                    .map(CommandOutput::OutboundInventory)
            }
            CommandIntent::SelectOutbound { request } => {
                let scope = required_core_scope(scope)?;
                self.outbound()?
                    .select_request(scope, &request)
                    .await
                    .map(CommandOutput::OutboundInventory)
            }
            CommandIntent::ProbeOutbound { request } => {
                let scope = required_core_scope(scope)?;
                self.outbound()?
                    .probe_request(scope, &request)
                    .await
                    .map(CommandOutput::OutboundProbe)
            }
            CommandIntent::ProbeOutboundBatch { request } => {
                let scope = required_core_scope(scope)?;
                self.outbound()?
                    .probe_batch_request(scope, &request)
                    .await
                    .map(CommandOutput::OutboundProbeBatch)
            }
            CommandIntent::ApplyOutboundUiPreference { request } => {
                let scope = required_core_scope(scope)?;
                let observation = self.outbound()?.observation(scope.clone()).await?;
                self.outbound_ui_preferences()?
                    .apply(scope, &request, &observation)
                    .map(CommandOutput::OutboundUiPreferencesApplied)
            }
            CommandIntent::SimulateRuleTrace { operation, request } => {
                let scope = required_core_scope(scope)?;
                let profile_workspace = self.profile()?.load_active_workspace().await?;
                let workspace_identity = profile_workspace.identity();
                if workspace_identity.document.engine_id != scope.identity.engine {
                    return Err(Failure::unsupported(
                        "active profile workspace belongs to another engine",
                    ));
                }
                let owner = RuleTraceOwnerBinding::new(scope.clone(), workspace_identity.clone())?;
                let scoped_request = ScopedRuleTraceRequest::new(owner, request)?;
                let outbound_facts = match scope.session_token.as_ref() {
                    Some(_) => RuleTraceOutboundFacts::Observed(
                        self.outbound()?.observation(scope.clone()).await?,
                    ),
                    None => RuleTraceOutboundFacts::Unavailable(Failure::new(
                        ErrorCode::NotReady,
                        "Core session is unavailable; this simulation uses only local profile AST facts",
                        true,
                    )),
                };
                self.rule_tracer()?
                    .simulate_scoped(
                        &scope,
                        &workspace_identity,
                        operation,
                        scoped_request,
                        &outbound_facts,
                    )
                    .await
                    .map(|_| CommandOutput::Unit)
            }
            CommandIntent::ClearDnsCache { operation } => {
                let scope = required_core_scope(scope)?;
                let report = self
                    .dns_cache
                    .as_ref()
                    .ok_or_else(|| Failure::unsupported("DNS cache application is unavailable"))?
                    .flush_with_id(scope.clone(), operation)
                    .await?;
                if report.fake_ip.is_flushed() || report.os_cache.is_flushed() {
                    Ok(CommandOutput::Unit)
                } else {
                    Err(Failure::unsupported(
                        "neither DNS cache target has a drivable adapter",
                    ))
                }
            }
            intent @ (CommandIntent::CreateBackupSnapshot { .. }
            | CommandIntent::LoadSnapshotHistory { .. }
            | CommandIntent::LoadSnapshotDiff { .. }
            | CommandIntent::PruneSnapshots { .. }
            | CommandIntent::PrepareSnapshotRestore { .. }
            | CommandIntent::ConfirmSnapshotRestore { .. }
            | CommandIntent::CancelSnapshotRestore { .. }) => {
                self.execute_snapshot_output(intent).await
            }
            CommandIntent::RunScriptSandbox { request } => self
                .scripts
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Script sandbox is not composed"))?
                .run_request(&request)
                .map(|result| CommandOutput::ScriptSandboxRun(Box::new(result))),
            CommandIntent::ClearScriptSandbox { operation } => self
                .scripts
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Script sandbox is not composed"))?
                .clear(operation)
                .map(CommandOutput::ScriptSandboxCleared),
            CommandIntent::PrepareScriptExport { draft } => self
                .script_exports
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Script export is not composed"))
                .and_then(|exports| match draft.kind {
                    infiltrator_contract::script_export::ScriptExportKind::MixinOverlayYaml => {
                        let options = self.profile_options()?;
                        exports.prepare_mixin_overlay(&draft, &options)
                    }
                    infiltrator_contract::script_export::ScriptExportKind::DirectiveDslScript
                    | infiltrator_contract::script_export::ScriptExportKind::ExtensionPackageJson => {
                        exports.prepare(&draft)
                    }
                })
                .map(|review| CommandOutput::ScriptExportPrepared(Box::new(review))),
            CommandIntent::SaveScriptExport { identity } => self
                .script_exports
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Script export is not composed"))?
                .confirm(&identity)
                .map(|saved| CommandOutput::ScriptExportSaved(Box::new(saved))),
            CommandIntent::CancelScriptExport { identity } => self
                .script_exports
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Script export is not composed"))?
                .cancel(&identity)
                .map(|()| CommandOutput::Unit),
            CommandIntent::ResetRuleHitCounters { expected_source } => self
                .rule_tracer()?
                .clear_hits(&expected_source)
                .await
                .map(CommandOutput::RuleStatisticsReset),
            CommandIntent::PrepareLogExport => self
                .log_export
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Log export is not composed"))?
                .prepare()
                .await
                .map(CommandOutput::LogExportPrepared),
            CommandIntent::SaveLogExport { identity } => self
                .log_export
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Log export is not composed"))?
                .save(&identity)
                .await
                .map(CommandOutput::LogExportSaved),
            CommandIntent::CancelLogExport { identity } => self
                .log_export
                .as_ref()
                .ok_or_else(|| Failure::unsupported("Log export is not composed"))?
                .cancel(&identity)
                .map(|()| CommandOutput::Unit),
            CommandIntent::SaveSubscriptionFilter {
                workspace_identity,
                filter,
            } => self
                .profile_options()?
                .save_filter(self.managed_runtime.clone(), &workspace_identity, &filter)
                .await
                .map(CommandOutput::SubscriptionFilterApplied),
            CommandIntent::LoadProfileDocument { profile } => self
                .profile_document()?
                .load(profile.as_deref())
                .await
                .map(|document| CommandOutput::ProfileDocumentLoaded(Box::new(document))),
            CommandIntent::SaveProfileDocument {
                workspace_identity,
                content,
                authorization,
            } => {
                let document = self
                    .profile_document()?
                    .save(
                        self.managed_runtime.clone(),
                        &workspace_identity,
                        &content,
                        authorization,
                    )
                    .await?;
                Ok(CommandOutput::ProfileDocumentSaved(Box::new(
                    ProfileDocumentSaved {
                        previous: workspace_identity,
                        document,
                    },
                )))
            }
            CommandIntent::SaveMixinOverlay {
                workspace_identity,
                mixin_yaml,
            } => self
                .profile_options()?
                .save_mixin(
                    self.managed_runtime.clone(),
                    &workspace_identity,
                    &mixin_yaml,
                )
                .await
                .map(|saved| CommandOutput::ProfileMixinSaved(Box::new(saved))),
            CommandIntent::LoadProfileOptions { profile } => self
                .profile_options()?
                .load(profile.as_deref())
                .await
                .map(CommandOutput::ProfileOptionsLoaded),
            intent => self
                .execute_dispatch(scope, intent)
                .await
                .map(|()| CommandOutput::Unit),
        }
    }
}

fn required_core_scope(scope: Option<&CoreScope>) -> Result<&CoreScope, Failure> {
    scope.ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "Core command has no validated Core scope",
            true,
        )
    })
}

fn required_routing_scope(scope: Option<&CoreScope>) -> Result<&CoreScope, Failure> {
    scope.ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "routing intent command has no validated Core scope",
            false,
        )
    })
}

impl CommandHandler for CommandApplication {
    fn inspect_profile_buffer(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        content: &str,
    ) -> Result<infiltrator_contract::profile_document::DocumentInspection, Failure> {
        Ok(self.profile_document()?.inspect_buffer(identity, content))
    }

    fn insert_profile_snippet(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome, Failure>
    {
        self.profile_document()?
            .insert_snippet_for_buffer(identity, content, snippet_id, line, column)
    }

    fn inspect_mixin_draft(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.profile_options()?
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }

    fn inspect_mixin_toggle(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: infiltrator_ports::profile_document_codec::ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.profile_options()?
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    fn set_mixin_toggle(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: infiltrator_ports::profile_document_codec::ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.profile_options()?
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    fn inspect_mixin_studio(
        &self,
        identity: &infiltrator_contract::profile_document_identity::ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<infiltrator_ports::profile_document_codec::ProfileMixinStudioProjection, Failure>
    {
        self.profile_options()?
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }

    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let application = self.clone();
        Box::pin(async move { application.execute(intent).await })
    }
    fn handle_output(&self, intent: CommandIntent) -> CommandOutputFuture {
        let application = self.clone();
        Box::pin(async move { application.execute_output(intent).await })
    }

    fn handle_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandFuture {
        let application = self.clone();
        Box::pin(async move {
            application
                .execute_output_with_scope(scope.as_ref(), intent)
                .await
                .map(|_| ())
        })
    }

    fn handle_output_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        let application = self.clone();
        Box::pin(async move {
            application
                .execute_output_with_scope(scope.as_ref(), intent)
                .await
        })
    }

    fn handle_lifecycle_child_output(
        &self,
        _permit: CoreLifecycleChildPermit,
        _scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        let application = self.clone();
        Box::pin(async move { application.execute_lifecycle_child_output(intent).await })
    }
}
