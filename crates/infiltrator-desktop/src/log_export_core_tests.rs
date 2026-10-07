//! Actual Core commands, lifecycle fencing and host files in an isolated directory.
use super::DesktopLogExportPort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{
    LogCaptureProcess, append_follow_records, populate_logs,
};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::ErrorCode;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

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
    core.execute(CommandIntent::SetLogLevelFilter {
        level: Some("WARN".into()),
    })
    .await
    .into_unit()
    .unwrap();
    let cancelled = core
        .execute(CommandIntent::PrepareLogExport)
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
    core.execute(CommandIntent::CancelLogExport {
        identity: cancelled.identity.clone(),
    })
    .await
    .into_unit()
    .unwrap();
    assert_eq!(
        core.execute(CommandIntent::SaveLogExport {
            identity: cancelled.identity
        })
        .await
        .into_output()
        .unwrap_err()
        .code,
        ErrorCode::InvalidState
    );
    assert!(!directory.path().join("exports").exists());
    let summary = core
        .execute(CommandIntent::PrepareLogExport)
        .await
        .into_output()
        .unwrap()
        .into_log_export_summary()
        .unwrap();
    append_follow_records(&core).unwrap();
    let receipt = core
        .execute(CommandIntent::SaveLogExport {
            identity: summary.identity.clone(),
        })
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
    core.execute(CommandIntent::RestartCore)
        .await
        .into_unit()
        .unwrap();
    assert!(!core.log_application().bind(Some(summary.identity.session)));
    assert!(!core.log_application().ingest(
        summary.identity.session,
        RuntimeStreamEvent::Item("ERROR[old] rejected stale worker".into())
    ));
    assert!(
        core.execute(CommandIntent::SaveLogExport {
            identity: summary.identity
        })
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
