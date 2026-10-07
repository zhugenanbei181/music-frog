//! Typed output routing validates actual results rather than unit acknowledgements.
use super::{CommandApplication, CommandFuture, CommandHandler, CommandOutputFuture};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document::ProfileDocumentSaved;

impl CommandApplication {
    pub async fn execute_output(&self, intent: CommandIntent) -> Result<CommandOutput, Failure> {
        match intent {
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
                .ok_or_else(|| Failure::unsupported("Script export is not composed"))?
                .prepare(&draft)
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
            CommandIntent::SaveSubscriptionFilter { source, filter } => self
                .profile_options()?
                .save_filter(self.managed_runtime.clone(), &source, &filter)
                .await
                .map(CommandOutput::SubscriptionFilterApplied),
            CommandIntent::LoadProfileDocument { profile } => self
                .profile_document()?
                .load(profile.as_deref())
                .await
                .map(|document| CommandOutput::ProfileDocumentLoaded(Box::new(document))),
            CommandIntent::SaveProfileDocument {
                source,
                content,
                allow_protected,
            } => {
                let document = self
                    .profile_document()?
                    .save(
                        self.managed_runtime.clone(),
                        &source,
                        &content,
                        allow_protected,
                    )
                    .await?;
                Ok(CommandOutput::ProfileDocumentSaved(Box::new(
                    ProfileDocumentSaved {
                        previous: source,
                        document,
                    },
                )))
            }
            CommandIntent::SaveMixinOverlay { source, mixin_yaml } => self
                .profile_options()?
                .save_mixin(self.managed_runtime.clone(), &source, &mixin_yaml)
                .await
                .map(|saved| CommandOutput::ProfileMixinSaved(Box::new(saved))),
            CommandIntent::LoadProfileOptions { profile } => self
                .profile_options()?
                .load(profile.as_deref())
                .await
                .map(CommandOutput::ProfileOptionsLoaded),
            intent => self
                .execute_dispatch(intent)
                .await
                .map(|()| CommandOutput::Unit),
        }
    }
}

impl CommandHandler for CommandApplication {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let application = self.clone();
        Box::pin(async move { application.execute(intent).await })
    }
    fn handle_output(&self, intent: CommandIntent) -> CommandOutputFuture {
        let application = self.clone();
        Box::pin(async move { application.execute_output(intent).await })
    }
}
