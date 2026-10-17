use crate::log_export_actions::{LogExportActions, LogExportPhase};
use crate::log_export_projection::project_log_export;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::log_export::{LogExportReceipt, LogExportSummary};
use infiltrator_contract::logs::LogSession;
use infiltrator_domain::log_export::prepare_log_export;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::session::SessionToken;

fn summary() -> LogExportSummary {
    prepare_log_export(
        LogSession::new(7, SessionToken::new(52)),
        1,
        &[(13, "INFO[1] healthy".into())],
        &[],
    )
    .unwrap()
    .summary
}

fn prepared() -> LogExportActions {
    let mut model = LogExportActions::default();
    let request = model.show().unwrap();
    assert!(model.finish(
        request.operation,
        Ok(CommandOutput::LogExportPrepared(summary()))
    ));
    model
}

#[test]
fn explicit_prepare_review_cancel_and_closed_phases_reject_invalid_actions() {
    let mut model = LogExportActions::default();
    assert!(matches!(model.phase(), LogExportPhase::Closed));
    let request = model.show().unwrap();
    assert!(matches!(model.phase(), LogExportPhase::Preparing { .. }));
    assert!(model.cancel().is_none());
    assert!(model.confirm().is_none());
    assert!(!model.finish(request.operation + 1, Ok(CommandOutput::Unit)));
    assert!(model.is_pending());
    assert!(model.finish(request.operation, Ok(CommandOutput::Unit)));
    assert!(matches!(
        model.phase(),
        LogExportPhase::PrepareFailed { .. }
    ));
    assert_eq!(model.failure().unwrap().code, ErrorCode::InvalidState);
    assert!(model.cancel().is_none());
    assert!(matches!(model.phase(), LogExportPhase::Closed));

    let mut model = prepared();
    assert!(matches!(model.phase(), LogExportPhase::Review { .. }));
    assert!(project_log_export(&model, "en-US").confirm);
    let cancelled = model.cancel().unwrap();
    assert_eq!(
        cancelled.intent,
        CommandIntent::CancelLogExport {
            identity: summary().identity
        }
    );
    assert!(matches!(model.phase(), LogExportPhase::Cancelling { .. }));
    assert!(model.finish(cancelled.operation, Ok(CommandOutput::Unit)));
    assert!(matches!(model.phase(), LogExportPhase::Closed));
}

#[test]
fn save_failure_retry_preserves_identity_and_only_a_matching_receipt_succeeds() {
    let mut model = prepared();
    let request = model.confirm().unwrap();
    assert_eq!(
        request.intent,
        CommandIntent::SaveLogExport {
            identity: summary().identity.clone()
        }
    );
    assert!(model.finish(
        request.operation,
        Err(Failure::new(
            ErrorCode::Storage,
            "directory unavailable",
            true
        ))
    ));
    assert!(matches!(model.phase(), LogExportPhase::SaveFailed { .. }));
    assert!(model.can_retry());
    let retry = model.retry().unwrap();
    assert_ne!(retry.operation, request.operation);
    assert_eq!(retry.intent, request.intent);
    let receipt = LogExportReceipt {
        summary: summary(),
        path: "/host/verified.log".into(),
        bytes_written: summary().bytes,
    };
    assert!(model.finish(
        retry.operation,
        Ok(CommandOutput::LogExportSaved(receipt.clone()))
    ));
    assert_eq!(model.receipt(), Some(&receipt));
    let view = project_log_export(&model, "en-US");
    assert_eq!(view.path, "/host/verified.log");
    assert!(!view.error && !view.confirm && !view.retry && view.close);
}

#[test]
fn invalid_receipt_is_typed_failure_and_cancel_retires_saved_owner_state() {
    let mut model = prepared();
    let save = model.confirm().unwrap();
    let mut receipt = LogExportReceipt {
        summary: summary(),
        path: "/host/other.log".into(),
        bytes_written: summary().bytes + 1,
    };
    receipt.summary.identity.sequence += 1;
    assert!(model.finish(save.operation, Ok(CommandOutput::LogExportSaved(receipt))));
    assert!(matches!(model.phase(), LogExportPhase::SaveFailed { .. }));
    assert_eq!(model.failure().unwrap().code, ErrorCode::InvalidState);
    assert!(!model.can_retry());
    assert!(model.receipt().is_none());
    let cancel = model.cancel().unwrap();
    assert!(matches!(
        cancel.intent,
        CommandIntent::CancelLogExport { .. }
    ));
    assert!(model.finish(cancel.operation, Ok(CommandOutput::Unit)));
    assert!(matches!(model.phase(), LogExportPhase::Closed));
}
