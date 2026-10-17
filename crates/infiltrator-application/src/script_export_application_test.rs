//! DUAL-10-12: application-level export tests.
//!
//! The recording port below is a *host double*: it proves the application
//! really hands the composed artifact to the port and reports the receipt.
//! The real filesystem adapter lives in `infiltrator-desktop` and is tested
//! there against a real temporary directory.

use super::*;
use crate::profile_application::ProfileApplication;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportOutcome, ScriptExportReceipt,
};
use infiltrator_contract::script_export_review::ScriptExportDraft;
use infiltrator_contract::script_export_review::ScriptExportValidationScope;
use infiltrator_domain::script_engine::HookStage;
use infiltrator_test_support::filter_capture_store::FilterCaptureStore;
use infiltrator_test_support::profile_document_codec_test_support::TestProfileDocumentCodec;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// Host double: records the exact request it was handed and answers a receipt.
#[derive(Default)]
struct RecordingExportPort {
    saved: Mutex<Vec<ScriptExportRequest>>,
}

#[derive(Clone, Copy)]
enum RetryWriteState {
    FailNext,
    Succeed,
}

struct FailOnceExportPort {
    state: Mutex<RetryWriteState>,
    calls: Mutex<Vec<ScriptExportRequest>>,
}

impl ScriptExportPort for FailOnceExportPort {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        self.calls
            .lock()
            .expect("retry port requests")
            .push(request.clone());
        let mut state = self.state.lock().expect("retry port state");
        match *state {
            RetryWriteState::FailNext => {
                *state = RetryWriteState::Succeed;
                Err(PortError::Network("injected write failure".to_owned()))
            }
            RetryWriteState::Succeed => Ok(ScriptExportReceipt {
                path: format!("/fake/exports/{}", request.file_name),
                bytes_written: request.byte_len(),
            }),
        }
    }
}

struct ReentrantExportPort {
    callback: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl ScriptExportPort for ReentrantExportPort {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        if let Some(callback) = self.callback.lock().expect("reentrant callback").as_ref() {
            callback();
        }
        Ok(ScriptExportReceipt {
            path: format!("/fake/exports/{}", request.file_name),
            bytes_written: request.byte_len(),
        })
    }
}

fn valid_review_draft() -> ScriptExportDraft {
    ScriptExportDraft {
        kind: ScriptExportKind::DirectiveDslScript,
        validation_scope: ScriptExportValidationScope::Unscoped,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: "function main(config) { return config; }".to_owned(),
        preset: None,
    }
}

fn overlay_review_draft(
    identity: ProfileDocumentIdentity,
    base_yaml: String,
    mixin_yaml: &str,
) -> ScriptExportDraft {
    ScriptExportDraft {
        kind: ScriptExportKind::MixinOverlayYaml,
        validation_scope: ScriptExportValidationScope::ProfileDocument(identity),
        profile: Some("filter-source".into()),
        base_yaml,
        mixin_yaml: mixin_yaml.into(),
        script_code: String::new(),
        preset: None,
    }
}

impl RecordingExportPort {
    fn saved_requests(&self) -> Vec<ScriptExportRequest> {
        self.saved.lock().expect("recording lock").clone()
    }
}

impl ScriptExportPort for RecordingExportPort {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        let mut saved = self.saved.lock().expect("recording lock");
        saved.push(request.clone());
        Ok(ScriptExportReceipt {
            path: format!("/fake/exports/{}", request.file_name),
            bytes_written: request.byte_len(),
        })
    }
}

fn package() -> ExtensionPackage {
    ExtensionPackage {
        name: "auto-country".to_string(),
        version: "1.0.0".to_string(),
        author: "tester".to_string(),
        description: "export test".to_string(),
        stage: HookStage::PreMerge,
        script_code: "function main(config) { auto_country_groups(config); return config; }"
            .to_string(),
        mixin_yaml: Some("ipv6: true\n".to_string()),
        tags: vec!["export".to_string()],
    }
}

#[test]
fn a_host_with_a_save_port_persists_the_real_artifact_and_reports_the_path() {
    let port = Arc::new(RecordingExportPort::default());
    let application = ScriptExportApplication::new(Some(port.clone()));
    assert!(application.has_host_port());

    let snapshot = application
        .export_directive_dsl(
            Some("auto-country-groups"),
            "function main(config) {\n  auto_country_groups(config);\n  return config;\n}",
            Some("auto-country-groups"),
        )
        .expect("export");
    assert_eq!(snapshot.kind, ScriptExportKind::DirectiveDslScript);
    assert_eq!(snapshot.file_name, "auto-country-groups.js");
    assert!(!snapshot.is_javascript());
    assert!(snapshot.content.contains("不是 JavaScript"));
    assert!(snapshot.content.contains("auto_country_groups(config)"));
    match &snapshot.outcome {
        ScriptExportOutcome::Saved {
            path,
            bytes_written,
        } => {
            assert_eq!(path, "/fake/exports/auto-country-groups.js");
            assert_eq!(*bytes_written, snapshot.byte_len());
        }
        other => panic!("expected a saved outcome, got {other:?}"),
    }

    // The port really received the artifact bytes; nothing was fabricated.
    let saved = port.saved_requests();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].content, snapshot.content);
    assert_eq!(saved[0].media_type, "text/plain");
    // The shared projection is the same fact the Bevy surface reads.
    assert_eq!(application.snapshot().as_ref(), Some(&snapshot));

    // The package export carries the real checksum and re-imports.
    let package = application
        .export_extension_package(&package())
        .expect("package export");
    let checksum = package.checksum.expect("checksum");
    assert_eq!(checksum.len(), 64);
    let restored: ExtensionPackage =
        serde_json::from_str(&package.content).expect("json package is real");
    assert_eq!(restored.name, "auto-country");
    assert_eq!(port.saved_requests().len(), 2);
}

#[test]
fn a_host_without_a_save_port_reports_typed_unsupported_without_losing_content() {
    let application = ScriptExportApplication::without_host_port();
    assert!(!application.has_host_port());
    let snapshot = application
        .export_directive_dsl(None, "function main(config) { return config; }", None)
        .expect("compose still succeeds");
    assert!(snapshot.outcome.is_unsupported());
    match &snapshot.outcome {
        ScriptExportOutcome::Unsupported { reason } => {
            assert!(reason.contains("文件保存对话框"), "{reason}");
        }
        other => panic!("expected unsupported, got {other:?}"),
    }
    // The composed bytes survive the unsupported host outcome.
    assert!(snapshot.content.contains("function main(config)"));
    assert_eq!(snapshot.byte_len(), snapshot.content.len());
    assert_eq!(application.snapshot().as_ref(), Some(&snapshot));
}

#[test]
fn invalid_drafts_are_refused_before_any_port_call() {
    let port = Arc::new(RecordingExportPort::default());
    let application = ScriptExportApplication::new(Some(port.clone()));
    assert!(application.export_directive_dsl(None, "", None).is_err());
    assert!(application.export_preset("does-not-exist").is_err());
    assert!(port.saved_requests().is_empty());
}

#[test]
fn mixin_overlay_review_requires_a_scoped_codec_preflight_and_never_writes_while_preparing() {
    let store = Arc::new(FilterCaptureStore::default());
    let workspace = store.observed();
    let identity = workspace.document.identity.clone();
    let base_yaml = workspace.document.utf8_text().unwrap().to_owned();
    let codec = Arc::new(TestProfileDocumentCodec::mihomo());
    let options = ProfileOptionsApplication::new(
        ProfileApplication::new(store.clone()).with_document_codec(codec.clone()),
    );
    let writer = Arc::new(RecordingExportPort::default());
    let exports = ScriptExportApplication::new(Some(writer.clone()));

    let valid = overlay_review_draft(identity.clone(), base_yaml.clone(), "ipv6: true\n");
    let calls_before_unscoped_entry = codec.options_call_count();
    assert_eq!(
        exports
            .prepare(&valid)
            .expect_err("overlay export cannot bypass the profile codec")
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(codec.options_call_count(), calls_before_unscoped_entry);
    let review = exports
        .prepare_mixin_overlay(&valid, &options)
        .expect("scoped codec accepts the overlay");
    assert_eq!(review.draft.validation_scope, valid.validation_scope);
    assert!(review.snapshot.content.contains("ipv6: true"));
    assert!(matches!(
        review.snapshot.outcome,
        ScriptExportOutcome::Prepared
    ));
    assert!(writer.saved_requests().is_empty(), "prepare is review only");
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.observed(), workspace);
    assert!(codec.options_call_count() > 0);

    exports.cancel(&review.identity).expect("cancel review");
    let invalid = overlay_review_draft(identity.clone(), base_yaml.clone(), "mode: [bad\n");
    let calls_before_invalid = codec.options_call_count();
    let failure = exports
        .prepare_mixin_overlay(&invalid, &options)
        .expect_err("invalid overlay is refused by the selected codec");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(codec.options_call_count(), calls_before_invalid + 1);
    assert!(exports.snapshot().is_none());
    assert!(writer.saved_requests().is_empty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.observed(), workspace);

    let mut foreign_identity = identity;
    foreign_identity.engine_id = EngineId::new("sing-box").expect("typed foreign engine");
    let foreign = overlay_review_draft(foreign_identity, base_yaml, "ipv6: true\n");
    let calls_before_foreign = codec.options_call_count();
    let failure = exports
        .prepare_mixin_overlay(&foreign, &options)
        .expect_err("foreign codec scope fails before parsing");
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(codec.options_call_count(), calls_before_foreign);
    assert!(writer.saved_requests().is_empty());
}

#[test]
fn export_projection_clears_and_republishes() {
    let application = ScriptExportApplication::without_host_port();
    let first = application
        .export_directive_dsl(
            Some("first"),
            "function main(config) { return config; }",
            None,
        )
        .expect("export");
    assert_eq!(application.snapshot().as_ref(), Some(&first));
    application.clear_history();
    assert!(application.snapshot().is_none());
    let second = application
        .export_preset("remove-ads")
        .expect("preset export");
    assert_eq!(second.file_name, "remove-ads.js");
    assert_eq!(application.snapshot().as_ref(), Some(&second));
    assert_ne!(first, second);
}

#[test]
fn failed_review_write_returns_to_prepared_and_retry_is_idempotent() {
    let port = Arc::new(FailOnceExportPort {
        state: Mutex::new(RetryWriteState::FailNext),
        calls: Mutex::new(Vec::new()),
    });
    let application = ScriptExportApplication::new(Some(port.clone()));
    let review = application.prepare(&valid_review_draft()).expect("prepare");

    let failure = application
        .confirm(&review.identity)
        .expect_err("first host write fails");
    assert_eq!(failure.code, ErrorCode::Network);

    let saved = application.confirm(&review.identity).expect("retry saves");
    let repeated = application
        .confirm(&review.identity)
        .expect("saved result is idempotent");
    assert_eq!(repeated, saved);
    assert_eq!(port.calls.lock().expect("retry port requests").len(), 2);
}

#[test]
fn cancel_expires_prepared_review_and_dismisses_saved_receipt_without_undoing_host_write() {
    let port = Arc::new(RecordingExportPort::default());
    let application = ScriptExportApplication::new(Some(port.clone()));
    let review = application.prepare(&valid_review_draft()).expect("prepare");

    application
        .cancel(&review.identity)
        .expect("cancel prepared review");
    assert!(application.snapshot().is_none());
    assert_eq!(
        application
            .confirm(&review.identity)
            .expect_err("canceled review is expired")
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(
        application
            .cancel(&review.identity)
            .expect_err("canceled review cannot be canceled twice")
            .code,
        ErrorCode::InvalidState
    );

    let saved_review = application
        .prepare(&valid_review_draft())
        .expect("prepare again");
    let saved = application
        .confirm(&saved_review.identity)
        .expect("persist reviewed bytes");
    application
        .cancel(&saved_review.identity)
        .expect("dismiss saved review without undoing the host write");
    assert!(application.snapshot().is_none());
    assert_eq!(
        application
            .confirm(&saved_review.identity)
            .expect_err("dismissed review is no longer active")
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(port.saved_requests().len(), 1);
    let ScriptExportOutcome::Saved {
        path,
        bytes_written,
    } = &saved.snapshot.outcome
    else {
        panic!("saved review must retain its original host receipt");
    };
    assert!(path.ends_with("/script.js"));
    assert_eq!(*bytes_written, saved.snapshot.byte_len());
}

#[test]
fn saving_review_rejects_reentrant_prepare_cancel_and_confirm() {
    let port = Arc::new(ReentrantExportPort {
        callback: Mutex::new(None),
    });
    let application = ScriptExportApplication::new(Some(port.clone()));
    let review = application.prepare(&valid_review_draft()).expect("prepare");
    let reentrant_results = Arc::new(Mutex::new(Vec::new()));
    let callback_application = application.clone();
    let callback_identity = review.identity.clone();
    let callback_results = reentrant_results.clone();
    *port.callback.lock().expect("reentrant callback") = Some(Box::new(move || {
        let outcomes = [
            callback_application
                .prepare(&valid_review_draft())
                .map(|_| ()),
            callback_application.cancel(&callback_identity),
            callback_application.confirm(&callback_identity).map(|_| ()),
        ];
        callback_results
            .lock()
            .expect("reentrant outcomes")
            .extend(outcomes.into_iter().map(|outcome| {
                outcome
                    .expect_err("review mutation is rejected while saving")
                    .code
            }));
    }));

    application
        .confirm(&review.identity)
        .expect("outer write succeeds");

    assert_eq!(
        *reentrant_results.lock().expect("reentrant outcomes"),
        [
            ErrorCode::InvalidState,
            ErrorCode::InvalidState,
            ErrorCode::InvalidState
        ]
    );
}
