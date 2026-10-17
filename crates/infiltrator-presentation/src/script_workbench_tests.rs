use crate::script_workbench::{ScriptExportPhase, ScriptWorkbench};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot,
};
use infiltrator_contract::script_export_review::{
    ScriptExportDraft, ScriptExportIdentity, ScriptExportReview, ScriptExportSaved,
    ScriptExportValidationScope,
};
use infiltrator_domain::script_export_review::content_sha256;
use lurker_core_contract::error::{ErrorCode, Failure};

fn draft() -> ScriptExportDraft {
    ScriptExportDraft {
        kind: ScriptExportKind::DirectiveDslScript,
        validation_scope: ScriptExportValidationScope::Unscoped,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: "return;".into(),
        preset: None,
    }
}

fn review(draft: ScriptExportDraft) -> ScriptExportReview {
    let content = "// honest DSL export\nreturn;\n".to_owned();
    let snapshot = ScriptExportSnapshot {
        kind: draft.kind,
        profile: draft.profile.clone(),
        file_name: "sandbox.js".into(),
        media_type: draft.kind.media_type().into(),
        checksum: None,
        honest_note: draft.kind.honest_note_zh().into(),
        content: content.clone(),
        outcome: ScriptExportOutcome::Prepared,
    };
    ScriptExportReview {
        identity: ScriptExportIdentity {
            owner: 1,
            sequence: 1,
            sha256: content_sha256(&content),
        },
        draft,
        snapshot,
    }
}

#[test]
fn export_workbench_uses_typed_review_failure_retry_saved_and_cancel_phases() {
    let draft = draft();
    let review = review(draft.clone());
    let mut workbench = ScriptWorkbench::default();

    let prepare = workbench.prepare_export(draft.clone()).unwrap();
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::Preparing { .. }
    ));
    assert!(workbench.is_exporting());
    workbench.finish(
        prepare.operation + 1,
        Err(Failure::new(ErrorCode::Storage, "foreign completion", true)),
    );
    assert!(workbench.is_exporting());

    workbench.finish(
        prepare.operation,
        Err(Failure::new(
            ErrorCode::Storage,
            "export source unavailable",
            true,
        )),
    );
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::PrepareFailed { .. }
    ));
    assert!(workbench.can_retry());
    let retry_prepare = workbench.retry().unwrap();
    assert!(matches!(
        retry_prepare.intent,
        CommandIntent::PrepareScriptExport { .. }
    ));
    workbench.finish(
        retry_prepare.operation,
        Ok(CommandOutput::ScriptExportPrepared(Box::new(
            review.clone(),
        ))),
    );
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::Review { .. }
    ));
    assert_eq!(workbench.review(), Some(&review));
    assert!(workbench.confirm_export().is_some());

    let save = workbench.pending().unwrap().clone();
    workbench.finish(
        save.operation,
        Err(Failure::new(
            ErrorCode::Permission,
            "write permission denied",
            false,
        )),
    );
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::SaveFailed { .. }
    ));
    assert!(workbench.can_retry());
    let retry_save = workbench.retry().unwrap();
    assert_ne!(retry_save.operation, save.operation);
    assert_eq!(
        retry_save.intent,
        CommandIntent::SaveScriptExport {
            identity: review.identity.clone()
        }
    );
    let mut saved_snapshot = review.snapshot.clone();
    saved_snapshot.outcome = ScriptExportOutcome::Saved {
        path: "/exports/sandbox.js".into(),
        bytes_written: saved_snapshot.content.len(),
    };
    let saved = ScriptExportSaved {
        identity: review.identity.clone(),
        snapshot: saved_snapshot.clone(),
    };
    workbench.finish(
        retry_save.operation,
        Ok(CommandOutput::ScriptExportSaved(Box::new(saved))),
    );
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::Saved { .. }
    ));
    assert_eq!(workbench.export(), Some(&saved_snapshot));

    let cancel = workbench.cancel_export().unwrap();
    assert_eq!(
        cancel.intent,
        CommandIntent::CancelScriptExport {
            identity: review.identity
        }
    );
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::Cancelling { .. }
    ));
    workbench.finish(cancel.operation, Ok(CommandOutput::Unit));
    assert!(matches!(
        workbench.export_phase(),
        ScriptExportPhase::Hidden
    ));
    assert!(workbench.export().is_none());
}
