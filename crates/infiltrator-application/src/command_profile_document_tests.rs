//! Source-bound document commands and snippet validity.
//! test-intent: behavior
use super::*;
use crate::command_application::handler::ValidatedCoreCommandScope;
use crate::core_capability_test_support::{core_scope, with_test_capabilities};
use crate::failure_projection::failure_message;
use crate::profile_document_application::ProfileDocumentApplication;
use crate::profile_document_codec_test_support::TestProfileDocumentCodec;
use infiltrator_contract::profile_document::{
    DocumentDiagnosticSnapshot, DocumentInspection, DocumentInspectionUnavailableReason,
    DocumentInspectionUnsupportedReason, DocumentValidation,
};
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome;
use std::str::from_utf8;

fn invalid_document_validation() -> DocumentValidation {
    DocumentValidation::Invalid {
        diagnostics: vec![DocumentDiagnosticSnapshot {
            line: Some(1),
            column: Some(0),
            message: "fixture syntax error".into(),
        }],
    }
}

#[tokio::test]
async fn load_and_save_profile_document_round_trips_through_the_shared_guard() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let application =
        with_test_capabilities(CommandApplication::new().with_profile(profiles.clone()));

    application
        .handle_output_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::LoadProfileDocument { profile: None },
        )
        .await
        .expect("load");
    let document = profiles
        .editor_observations()
        .0
        .expect("the shared document is published for the surfaces");
    assert_eq!(
        document.workspace_identity.document.profile_id.as_str(),
        "main"
    );
    assert_eq!(document.content, THREE_RULES);
    assert_eq!(
        document.inspection,
        DocumentInspection::Inspected(DocumentValidation::Valid),
        "a valid stored document has an engine-scoped codec verdict"
    );
    assert!(!document.write_protection.is_protected());
    assert_eq!(document.line_count, THREE_RULES.lines().count());

    // Invalid bytes are rejected by the configured codec before store reads
    // or writes and are not projected as a YAML-specific command failure.
    let reads_before_invalid = store.workspace_reads.load(Ordering::SeqCst);
    codec.queue_inspection(invalid_document_validation());
    let error = application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::SaveProfileDocument {
                workspace_identity: document.workspace_identity.clone(),
                content: "rules: [\n".to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await
        .expect_err("invalid yaml is rejected");
    assert_eq!(error.code, ErrorCode::Configuration);
    assert!(
        error.reason.is_none(),
        "generic document failure: {error:?}"
    );
    assert!(failure_message(&error, "en-US").contains("line"));
    assert_eq!(
        store.workspace_reads.load(Ordering::SeqCst),
        reads_before_invalid,
        "invalid input is rejected before a store read"
    );
    assert_eq!(store.content(), THREE_RULES, "the store is untouched");

    // A valid buffer commits against the observed workspace and publishes the actual receipt.
    let saved = "rules:\n  - MATCH,DIRECT\n";
    application
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::SaveProfileDocument {
                workspace_identity: document.workspace_identity.clone(),
                content: saved.to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await
        .expect("save");
    assert_eq!(store.content(), saved);
    assert_eq!(
        profiles.editor_observations().0.expect("published").content,
        saved
    );
}

#[tokio::test]
async fn missing_or_mismatched_codec_never_claims_valid_or_reaches_store_on_write() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let plain_profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let plain_documents = ProfileDocumentApplication::new(plain_profiles.clone());
    let unavailable = plain_documents
        .load(None)
        .await
        .expect("raw text is readable");
    assert_eq!(
        unavailable.inspection,
        DocumentInspection::Unavailable(DocumentInspectionUnavailableReason::CodecMissing)
    );
    let reads_before_write = store.workspace_reads.load(Ordering::SeqCst);
    let error = plain_documents
        .save::<dyn ManagedRuntime>(
            None,
            &unavailable.workspace_identity,
            "mode: rule\n",
            ProfileWriteAuthorization::RequireUnlock,
        )
        .await
        .expect_err("a missing codec fails closed");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(
        store.workspace_reads.load(Ordering::SeqCst),
        reads_before_write
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);

    let workspace = plain_profiles.load_workspace("main").await.unwrap();
    for (engine, format, schema) in [
        ("sing-box", "mihomo-yaml", 1),
        ("mihomo", "sing-box-json", 1),
        ("mihomo", "mihomo-yaml", 2),
    ] {
        let mismatch = Arc::new(TestProfileDocumentCodec::for_scope(engine, format, schema));
        let mismatch_profiles = plain_profiles.clone().with_document_codec(mismatch.clone());
        let inspection = mismatch_profiles.inspect_document(&workspace.document);
        match (engine, format, inspection) {
            (
                "sing-box",
                _,
                DocumentInspection::Unsupported(
                    DocumentInspectionUnsupportedReason::EngineMismatch { .. },
                ),
            ) => {}
            (
                _,
                "sing-box-json",
                DocumentInspection::Unsupported(
                    DocumentInspectionUnsupportedReason::FormatMismatch { .. },
                ),
            ) => {}
            (
                _,
                _,
                DocumentInspection::Unsupported(
                    DocumentInspectionUnsupportedReason::SchemaMismatch { .. },
                ),
            ) => {}
            (_, _, actual) => panic!("unexpected scoped inspection: {actual:?}"),
        }
        let before = store.workspace_reads.load(Ordering::SeqCst);
        let command =
            with_test_capabilities(CommandApplication::new().with_profile(mismatch_profiles));
        let error = command
            .handle_scoped(
                ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
                CommandIntent::SaveProfileDocument {
                    workspace_identity: workspace.identity(),
                    content: "mode: rule\n".to_owned(),
                    authorization: ProfileWriteAuthorization::RequireUnlock,
                },
            )
            .await
            .expect_err("a codec scope mismatch fails closed");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(store.workspace_reads.load(Ordering::SeqCst), before);
        assert_eq!(mismatch.inspection_count(), 0);
        assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn malformed_stored_document_loads_with_typed_diagnostics_without_changing_raw_bytes() {
    let malformed = b"rules: [\n";
    let store = Arc::new(FakeStore::with_profile(
        from_utf8(malformed).expect("fixture is UTF-8"),
    ));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    codec.queue_inspection(invalid_document_validation());
    let snapshot = ProfileDocumentApplication::new(profiles)
        .load(None)
        .await
        .expect("malformed text remains available to repair");
    match snapshot.inspection {
        DocumentInspection::Inspected(DocumentValidation::Invalid { diagnostics }) => {
            assert!(!diagnostics.is_empty());
            assert!(diagnostics[0].line.is_some());
            assert!(diagnostics[0].column.is_some());
        }
        actual => panic!("stored malformed document must retain diagnostics: {actual:?}"),
    }
    assert_eq!(store.content(), from_utf8(malformed).unwrap());
}

// ---- DUAL-09-04: codec-scoped snippet and live preflight -------------------

#[tokio::test]
async fn live_preflight_and_snippet_insertion_use_neutral_codec_diagnostics() {
    let document = "proxies:\n  - name: keep\n    type: ss\nrules:\n  - MATCH,DIRECT\n";
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let workspace = profiles
        .load_workspace("main")
        .await
        .expect("workspace identity");
    let application = ProfileDocumentApplication::new(profiles);
    let identity = &workspace.document.identity;
    let reads_before = store.workspace_reads.load(Ordering::SeqCst);
    let writes_before = store.workspace_writes.load(Ordering::SeqCst);

    assert_eq!(
        application.inspect_buffer(identity, document),
        DocumentInspection::Inspected(DocumentValidation::Valid)
    );
    match application.insert_snippet_for_buffer(identity, document, "ss", 3, 12) {
        Ok(ProfileDocumentSnippetOutcome::Inserted(insertion)) => {
            assert_eq!(insertion.validation, DocumentValidation::Valid);
            assert!(
                insertion
                    .content
                    .starts_with("proxies:\n  - name: keep\n    type: ss")
            );
            assert!(insertion.content.contains("- name: SS-Node"));
        }
        other => panic!("valid snippet must be applied: {other:?}"),
    }

    let malformed = "mode: rule\n";
    codec.queue_inspection(DocumentValidation::Valid);
    codec.queue_inspection(invalid_document_validation());
    match application.insert_snippet_for_buffer(identity, malformed, "select", 1, 6) {
        Ok(ProfileDocumentSnippetOutcome::Rejected {
            validation: DocumentValidation::Invalid { diagnostics },
        }) => {
            assert!(!diagnostics.is_empty());
            assert!(diagnostics[0].line.is_some());
        }
        other => panic!("a snippet that breaks valid YAML must be rejected: {other:?}"),
    }

    let already_invalid = "rules: [\n";
    codec.queue_inspection(invalid_document_validation());
    assert!(matches!(
        application.inspect_buffer(identity, already_invalid),
        DocumentInspection::Inspected(DocumentValidation::Invalid { .. })
    ));
    codec.queue_inspection(invalid_document_validation());
    codec.queue_inspection(invalid_document_validation());
    assert!(
        matches!(
            application.insert_snippet_for_buffer(identity, already_invalid, "rule-geoip", 1, 0),
            Ok(ProfileDocumentSnippetOutcome::Inserted(_))
        ),
        "editing an already-invalid document remains possible"
    );
    assert_eq!(
        store.workspace_reads.load(Ordering::SeqCst),
        reads_before,
        "preflight and insertion do not read profile storage"
    );
    assert_eq!(
        store.workspace_writes.load(Ordering::SeqCst),
        writes_before,
        "preflight and insertion do not write profile storage"
    );
}

#[tokio::test]
async fn foreign_document_identity_is_rejected_before_codec_or_store_side_effects() {
    let store = Arc::new(FakeStore::with_profile(THREE_RULES));
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .with_document_codec(codec.clone());
    let workspace = profiles
        .load_workspace("main")
        .await
        .expect("workspace identity");
    let mut foreign: ProfileDocumentIdentity = workspace.document.identity.clone();
    foreign.engine_id = infiltrator_contract::core_identity::EngineId::new("sing-box")
        .expect("valid foreign engine");
    let application = ProfileDocumentApplication::new(profiles);
    let reads_before = store.workspace_reads.load(Ordering::SeqCst);
    let writes_before = store.workspace_writes.load(Ordering::SeqCst);
    let inspections_before = codec.inspection_count();

    assert!(matches!(
        application.inspect_buffer(&foreign, "mode: rule\n"),
        DocumentInspection::Unsupported(DocumentInspectionUnsupportedReason::EngineMismatch { .. })
    ));
    assert_eq!(
        application
            .insert_snippet_for_buffer(&foreign, "mode: rule\n", "ss", 1, 0)
            .expect_err("foreign engine must fail closed")
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(codec.inspection_count(), inspections_before);
    assert_eq!(codec.insertion_count(), 0);
    assert_eq!(store.workspace_reads.load(Ordering::SeqCst), reads_before);
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), writes_before);
}
