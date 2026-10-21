//! The Android host hands the owner's typed readback to the Slint surface.
//!
//! The `:vpn` owner answers a product command with the typed output it
//! produced. These tests prove that the Android owner channel keeps that
//! output and that the shared surface reducer settles the dispatched business
//! on the owner's identity instead of on an acknowledgement.

use super::{
    OwnerAnswer, decode_owner_terminal, encode_application_command, execute_owner_command,
    execute_owner_commands,
};
use infiltrator_contract::android_control::{
    AndroidProductCommand, AndroidProductCommandOutcome, AndroidProductCommandRequest,
};
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreCommandOutcome;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportSnapshot,
};
use infiltrator_contract::script_export_review::{
    ScriptExportDraft, ScriptExportIdentity, ScriptExportReview, ScriptExportValidationScope,
};
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_presentation::script_workbench::ScriptExportPhase;
use infiltrator_presentation::subscription_filter_editor::SubscriptionFilterEditor;
use infiltrator_slint::surface::workbenches::{PendingBusiness, Workbenches};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use lurker_core_contract::session::SessionToken;

/// The Core scope the Android owner holds.
fn core_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("android-owner").expect("valid instance"),
            None,
        ),
        generation: CoreGeneration::new(3),
        session_token: Some(SessionToken::new(11)),
    }
}

/// The workspace identity of one profile document.
fn workspace_identity(profile: &str) -> ProfileWorkspaceIdentity {
    let document = ProfileDocumentIdentity::new(
        ProfileId::new(profile).expect("valid profile"),
        EngineId::mihomo(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new(format!("local-{profile}")).expect("valid source"),
        ),
        ProfileDocumentRevision::new(format!("revision-{profile}")).expect("valid revision"),
        ProfileFormatId::new("mihomo-yaml").expect("valid format"),
        ProfileSchemaVersion::new(1).expect("valid schema"),
    );
    ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent)
}

/// The export draft the script workbench dispatches.
fn export_draft() -> ScriptExportDraft {
    ScriptExportDraft {
        kind: ScriptExportKind::DirectiveDslScript,
        validation_scope: ScriptExportValidationScope::Unscoped,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: "print('hello')".to_owned(),
        preset: None,
    }
}

/// The review the owner publishes for the export draft.
fn export_review(draft: &ScriptExportDraft) -> ScriptExportReview {
    ScriptExportReview {
        identity: ScriptExportIdentity {
            owner: 4,
            sequence: 9,
            sha256: "a".repeat(64),
        },
        draft: draft.clone(),
        snapshot: ScriptExportSnapshot {
            kind: draft.kind,
            profile: draft.profile.clone(),
            file_name: "script.js".to_owned(),
            media_type: draft.kind.media_type().to_owned(),
            content: draft.script_code.clone(),
            checksum: None,
            honest_note: draft.kind.honest_note_zh().to_owned(),
            outcome: ScriptExportOutcome::Prepared,
        },
    }
}

/// The terminal the `:vpn` owner answers with.
fn owner_terminal(scope: &CoreScope, result: CommandResult) -> String {
    serde_json::to_string(&AndroidProductCommandOutcome::Terminal(Box::new(
        CoreCommandOutcome {
            scope: Some(scope.clone()),
            result,
        },
    )))
    .expect("the owner terminal serializes")
}

#[test]
fn the_android_owner_hands_the_script_export_review_to_the_surface() {
    let scope = core_scope();
    let draft = export_draft();
    let review = export_review(&draft);

    // The UI process encodes the command for the owner.
    let request_json = encode_application_command(
        &scope,
        CommandIntent::PrepareScriptExport {
            draft: draft.clone(),
        },
    )
    .expect("the command encodes");
    let request: AndroidProductCommandRequest =
        serde_json::from_str(&request_json).expect("the owner decodes the command");
    assert_eq!(request.scope, scope);
    let AndroidProductCommand::Application(intent) = request.command else {
        panic!("the Slint surface sends an application command");
    };
    assert_eq!(
        *intent,
        CommandIntent::PrepareScriptExport {
            draft: draft.clone()
        }
    );

    // The owner answers with the typed output it produced.
    let terminal = owner_terminal(
        &scope,
        CommandResult::Produced {
            request_id: RequestId::new(7),
            output: Box::new(CommandOutput::ScriptExportPrepared(Box::new(
                review.clone(),
            ))),
        },
    );

    // The Android host keeps that output instead of an acknowledgement.
    let transport = |_request_json: &str| -> Result<String, Failure> { Ok(terminal.clone()) };
    let owner = execute_owner_command(
        &scope,
        CommandIntent::PrepareScriptExport {
            draft: draft.clone(),
        },
        &transport,
    )
    .expect("the owner terminal decodes");
    assert_eq!(owner.scope, Some(scope.clone()));
    let OwnerAnswer::Produced(output) = owner.answer else {
        panic!("the owner produced a typed output");
    };
    assert_eq!(
        output,
        CommandOutput::ScriptExportPrepared(Box::new(review.clone()))
    );

    // The surface business that was dispatched settles on the owner's identity.
    let workbenches = Workbenches::default();
    {
        let mut script = workbenches.script.lock().expect("script workbench lock");
        script
            .prepare_export(draft.clone())
            .expect("the export draft starts a business");
    }
    workbenches.record(PendingBusiness::Script);
    assert!(matches!(
        workbenches
            .script
            .lock()
            .expect("script workbench lock")
            .export_phase(),
        ScriptExportPhase::Preparing { .. }
    ));
    assert!(workbenches.settle(Ok(output)));
    let script = workbenches.script.lock().expect("script workbench lock");
    let ScriptExportPhase::Review { review: observed } = script.export_phase() else {
        panic!("the export business observed the owner's review");
    };
    assert_eq!(observed.identity, review.identity);
    assert_eq!(observed.draft, draft);
}

#[test]
fn the_android_owner_hands_the_applied_subscription_filter_to_the_surface() {
    let scope = core_scope();
    let identity = workspace_identity("main");
    let draft = SubscriptionFilterDraft::default();
    let applied = SubscriptionFilterApplied {
        workspace_identity: identity.clone(),
        report: FilterReport {
            total_input: 12,
            passed: 9,
            renamed: 1,
            ..FilterReport::default()
        },
    };
    let terminal = owner_terminal(
        &scope,
        CommandResult::Produced {
            request_id: RequestId::new(8),
            output: Box::new(CommandOutput::SubscriptionFilterApplied(applied.clone())),
        },
    );

    let transport = |_request_json: &str| -> Result<String, Failure> { Ok(terminal.clone()) };
    let output = execute_owner_commands(
        scope.clone(),
        vec![CommandIntent::SaveSubscriptionFilter {
            workspace_identity: identity.clone(),
            filter: draft.clone(),
        }],
        &transport,
    )
    .expect("the owner terminal decodes");
    assert_eq!(
        output,
        CommandOutput::SubscriptionFilterApplied(applied.clone())
    );

    let workbenches = Workbenches::default();
    {
        let mut filter = workbenches.filter.lock().expect("filter editor lock");
        *filter = SubscriptionFilterEditor::new(Some(FilterObservation {
            workspace_identity: identity.clone(),
            filter: draft.clone(),
        }));
        filter.begin().expect("the filter form starts a write");
    }
    workbenches.record(PendingBusiness::Filter);
    assert!(workbenches.settle(Ok(output)));
    let filter = workbenches.filter.lock().expect("filter editor lock");
    assert!(filter.applied());
    assert_eq!(filter.report(), Some(&applied.report));
}

#[test]
fn the_android_owner_refusal_reaches_the_surface_as_the_owners_failure() {
    let scope = core_scope();
    let draft = export_draft();
    let failure = Failure::new(ErrorCode::Storage, "the owner refused the export", true);
    let terminal = owner_terminal(
        &scope,
        CommandResult::Rejected {
            request_id: RequestId::new(9),
            failure: failure.clone(),
        },
    );

    let transport = |_request_json: &str| -> Result<String, Failure> { Ok(terminal.clone()) };
    let error = execute_owner_commands(
        scope.clone(),
        vec![CommandIntent::PrepareScriptExport {
            draft: draft.clone(),
        }],
        &transport,
    )
    .expect_err("the owner refused the command");
    assert_eq!(error, failure);

    let workbenches = Workbenches::default();
    {
        let mut script = workbenches.script.lock().expect("script workbench lock");
        script
            .prepare_export(draft.clone())
            .expect("the export draft starts a business");
    }
    workbenches.record(PendingBusiness::Script);
    assert!(workbenches.settle(Err(error)));
    let script = workbenches.script.lock().expect("script workbench lock");
    assert!(matches!(
        script.export_phase(),
        ScriptExportPhase::PrepareFailed { .. }
    ));
    assert_eq!(script.failure(), Some(&failure));
}

#[test]
fn a_command_without_a_typed_output_leaves_the_business_dispatched() {
    let scope = core_scope();
    let identity = workspace_identity("main");
    let draft = SubscriptionFilterDraft::default();
    let terminal = owner_terminal(
        &scope,
        CommandResult::Completed {
            request_id: RequestId::new(10),
        },
    );

    let transport = |_request_json: &str| -> Result<String, Failure> { Ok(terminal.clone()) };
    let output = execute_owner_commands(
        scope.clone(),
        vec![CommandIntent::SaveSubscriptionFilter {
            workspace_identity: identity.clone(),
            filter: draft.clone(),
        }],
        &transport,
    )
    .expect("the owner terminal decodes");
    assert_eq!(output, CommandOutput::Unit);

    let workbenches = Workbenches::default();
    {
        let mut filter = workbenches.filter.lock().expect("filter editor lock");
        *filter = SubscriptionFilterEditor::new(Some(FilterObservation {
            workspace_identity: identity.clone(),
            filter: draft.clone(),
        }));
        filter.begin().expect("the filter form starts a write");
    }
    workbenches.record(PendingBusiness::Filter);
    assert!(!workbenches.settle(Ok(output)));
    let filter = workbenches.filter.lock().expect("filter editor lock");
    assert!(!filter.applied());
    assert!(filter.pending().is_some());
}

#[test]
fn the_owner_terminal_keeps_its_typed_refusal_and_its_scope() {
    let scope = core_scope();
    let failure = Failure::new(ErrorCode::Permission, "the owner is not allowed", false);
    let raw = serde_json::to_string(&AndroidProductCommandOutcome::Unavailable {
        scope: Some(scope.clone()),
        failure: failure.clone(),
    })
    .expect("the owner refusal serializes");

    let error = decode_owner_terminal(&raw).expect_err("the refusal is not a terminal");
    assert_eq!(error, failure);
}
