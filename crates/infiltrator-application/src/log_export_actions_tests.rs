//! Shared modal transitions consume exact typed preparation and save terminals.
use super::LogExportActions;
use crate::log_export_projection::project_log_export;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{LogExportReceipt, LogExportSummary};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::session::SessionToken;
use infiltrator_domain::log_export::prepare_log_export;

fn summary() -> LogExportSummary {
    prepare_log_export(
        LogSession {
            generation: 7,
            token: SessionToken::new(52),
        },
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
fn prepare_requires_typed_summary_and_cancel_cannot_submit_a_save_or_close_a_pending_request() {
    let mut model = LogExportActions::default();
    let request = model.show().unwrap();
    assert_eq!(request.intent, CommandIntent::PrepareLogExport);
    assert!(model.cancel().is_none());
    assert!(model.confirm().is_none());
    assert!(!model.finish(request.operation + 1, Ok(CommandOutput::Unit)));
    assert!(model.pending.is_some());
    assert!(model.finish(request.operation, Ok(CommandOutput::Unit)));
    assert!(model.open);
    assert_eq!(
        model.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(!project_log_export(&model, "en-US").confirm);
    assert!(model.cancel().is_none());
    assert!(!model.open);
    let mut model = prepared();
    let cancelled = model.cancel().unwrap();
    assert_eq!(
        cancelled.intent,
        CommandIntent::CancelLogExport {
            identity: summary().identity
        }
    );
    assert!(model.open);
    assert!(model.finish(cancelled.operation, Ok(CommandOutput::Unit)));
    assert!(!model.open);
    assert!(model.receipt.is_none());
}
#[test]
fn save_failure_remains_visible_retry_reuses_identity_and_only_a_matching_real_receipt_completes() {
    let mut model = prepared();
    let view = project_log_export(&model, "en-US");
    assert!(view.confirm && view.close && !view.retry);
    assert!(view.details.contains("1 buffered records"));
    let request = model.confirm().unwrap();
    assert_eq!(
        request.intent,
        CommandIntent::SaveLogExport {
            identity: summary().identity.clone()
        }
    );
    assert!(model.cancel().is_none());
    assert!(model.confirm().is_none());
    let failure = Failure::new(ErrorCode::Storage, "directory unavailable", true);
    assert!(model.finish(request.operation, Err(failure.clone())));
    assert_eq!(model.failure, Some(failure));
    let view = project_log_export(&model, "en-US");
    assert!(view.error && view.retry && view.close && !view.confirm);
    assert!(view.status.contains("directory unavailable"));
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
    assert_eq!(model.receipt, Some(receipt));
    let view = project_log_export(&model, "en-US");
    assert_eq!(view.path, "/host/verified.log");
    assert!(!view.error && !view.confirm && !view.retry && view.close);
}
#[test]
fn wrong_bytes_and_unrelated_source_receipts_remain_failures_with_no_saved_path() {
    for mismatched_source in [false, true] {
        let mut model = prepared();
        let request = model.confirm().unwrap();
        let mut receipt = LogExportReceipt {
            summary: summary(),
            path: "/host/other.log".into(),
            bytes_written: summary().bytes,
        };
        if mismatched_source {
            receipt.summary.identity.sequence += 1;
        } else {
            receipt.bytes_written += 1;
        }
        assert!(model.finish(
            request.operation,
            Ok(CommandOutput::LogExportSaved(receipt))
        ));
        assert_eq!(
            model.failure.as_ref().unwrap().code,
            ErrorCode::InvalidState
        );
        assert!(model.receipt.is_none());
        assert!(project_log_export(&model, "en-US").path.is_empty());
    }
}
