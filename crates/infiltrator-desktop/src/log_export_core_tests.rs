//! Actual Core commands, lifecycle fencing and host files in an isolated directory.
use super::DesktopLogExportPort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::log_application::{
    LogBindingIntent, LogBindingOutcome, LogIngestOutcome,
};
use infiltrator_application::log_capture_fixtures::{
    LogCaptureProcess, append_follow_records, populate_logs,
};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreLifecycleOperation, CoreOperation, EngineCapabilityDecision,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

async fn execute_at_observed_scope(core: &CoreApplication, intent: CommandIntent) -> CommandResult {
    let scope = core
        .snapshot()
        .scope()
        .expect("desktop test observes the Core owner scope");
    core.execute_scoped(CoreCommandRequest { scope, intent })
        .await
        .result
}

struct LifecycleTestCapabilities;

impl CoreCapabilityProvider for LifecycleTestCapabilities {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = [
            CoreLifecycleOperation::Start,
            CoreLifecycleOperation::Stop,
            CoreLifecycleOperation::Restart,
        ]
        .into_iter()
        .map(|operation| {
            CoreCapabilityEntry::new(
                CoreOperation::Lifecycle(operation),
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        })
        .collect();
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

#[tokio::test]
async fn actual_typed_commands_cancel_without_files_save_frozen_bytes_and_retire_old_confirmations_on_restart()
 {
    let directory = tempdir().unwrap();
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().unwrap(),
    );
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        LifecycleTestCapabilities,
    )))
    .expect("explicit lifecycle test capabilities install before use");
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_logs(core.log_application())
            .with_log_export(LogExportApplication::new(
                core.log_application(),
                Some(Arc::new(DesktopLogExportPort::new(directory.path()))),
                vec![],
            )),
    ));
    populate_logs(&core).await.unwrap();
    execute_at_observed_scope(
        &core,
        CommandIntent::SetLogLevelFilter {
            level: Some("WARN".into()),
        },
    )
    .await
    .into_unit()
    .unwrap();
    let cancelled = execute_at_observed_scope(&core, CommandIntent::PrepareLogExport)
        .await
        .into_output()
        .unwrap()
        .into_log_export_summary()
        .unwrap();
    assert_eq!(
        cancelled.records, 3,
        "viewer filtering cannot change export scope"
    );
    assert!(!directory.path().join("exports").exists());
    execute_at_observed_scope(
        &core,
        CommandIntent::CancelLogExport {
            identity: cancelled.identity.clone(),
        },
    )
    .await
    .into_unit()
    .unwrap();
    assert_eq!(
        execute_at_observed_scope(
            &core,
            CommandIntent::SaveLogExport {
                identity: cancelled.identity
            }
        )
        .await
        .into_output()
        .unwrap_err()
        .code,
        ErrorCode::InvalidState
    );
    assert!(!directory.path().join("exports").exists());
    let summary = execute_at_observed_scope(&core, CommandIntent::PrepareLogExport)
        .await
        .into_output()
        .unwrap()
        .into_log_export_summary()
        .unwrap();
    append_follow_records(&core).unwrap();
    let receipt = execute_at_observed_scope(
        &core,
        CommandIntent::SaveLogExport {
            identity: summary.identity.clone(),
        },
    )
    .await
    .into_output()
    .unwrap()
    .into_log_export_receipt()
    .unwrap();
    assert_eq!(receipt.summary, summary);
    let content = fs::read_to_string(&receipt.path).unwrap();
    assert_eq!(content.lines().count(), 3);
    assert!(content.contains("API.EXAMPLE dial TIMEOUT"));
    assert!(!content.contains("continued controller record"));
    assert_eq!(receipt.bytes_written, content.len());
    execute_at_observed_scope(&core, CommandIntent::RestartCore)
        .await
        .into_unit()
        .unwrap();
    assert_eq!(
        core.log_application()
            .bind(LogBindingIntent::Bind(summary.identity.session.clone())),
        LogBindingOutcome::RejectedByCoreOwner
    );
    assert_eq!(
        core.log_application().ingest(
            summary.identity.session.clone(),
            RuntimeStreamEvent::Item("ERROR[old] rejected stale worker".into())
        ),
        LogIngestOutcome::RejectedForeignSession
    );
    assert!(
        execute_at_observed_scope(
            &core,
            CommandIntent::SaveLogExport {
                identity: summary.identity
            }
        )
        .await
        .into_output()
        .is_err()
    );
    assert_eq!(fs::read_to_string(&receipt.path).unwrap(), content);
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
    core.close().await.unwrap();
}
