//! test-intent: behavior
//! Independent product services preserve admission and exact reviewed host effects.
use crate::command_application::CommandApplication;
use crate::core_application::CoreApplication;
use crate::language_choice_fixtures::LanguageCaptureProcess;
use crate::log_stream_test_support::TestRuntime;
use crate::script_application::ScriptApplication;
use crate::script_export_application::ScriptExportApplication;
use crate::script_workbench::ScriptWorkbench;
use crate::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportReceipt, ScriptExportRequest,
};
use infiltrator_contract::script_export_review::ScriptExportDraft;
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::script_sandbox::{ScriptEngineCapabilities, ScriptEngineKind};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_domain::script_engine::{HookStage, ScriptError, ScriptExecutionResult};
use infiltrator_domain::script_export_review::content_sha256;
use infiltrator_ports::error::PortError;
use infiltrator_ports::script_engine::ScriptEnginePort;
use infiltrator_ports::script_export::ScriptExportPort;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FailingEngine(AtomicUsize);
impl ScriptEnginePort for FailingEngine {
    fn kind(&self) -> ScriptEngineKind {
        ScriptEngineKind::DirectiveDsl
    }
    fn capabilities(&self) -> ScriptEngineCapabilities {
        ScriptEngineCapabilities::directive_dsl()
    }
    fn execute(
        &self,
        _: &str,
        _: &str,
        _: HookStage,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ScriptError::Runtime("injected failure".into()))
    }
}
fn run_request(operation: u64) -> ScriptRunRequest {
    ScriptRunRequest {
        operation: ScriptOperationId(operation),
        script_code: "function main(config) { return config; }".into(),
        input_yaml: "mode: rule\n".into(),
        preset: None,
    }
}
fn draft() -> ScriptExportDraft {
    ScriptExportDraft {
        kind: ScriptExportKind::DirectiveDslScript,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: run_request(1).script_code,
        preset: None,
    }
}
#[derive(Default)]
struct Writer {
    calls: Mutex<Vec<ScriptExportRequest>>,
    failure: Mutex<bool>,
    incomplete: Mutex<bool>,
}
impl ScriptExportPort for Writer {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        if *self.failure.lock().unwrap() {
            return Err(PortError::PermissionDenied("write denied".into()));
        }
        self.calls.lock().unwrap().push(request.clone());
        Ok(ScriptExportReceipt {
            path: "/isolated/export.js".into(),
            bytes_written: request.content.len() - usize::from(*self.incomplete.lock().unwrap()),
        })
    }
}
#[test]
fn sandbox_clones_share_breaker_and_report_clear_does_not_reset_admission_or_other_products() {
    let engine = Arc::new(FailingEngine::default());
    let owner = ScriptApplication::with_engine(engine.clone());
    let peer = ScriptApplication::with_engine(engine.clone());
    for operation in 1..=3 {
        owner.clone().run_request(&run_request(operation)).unwrap();
    }
    assert_eq!(engine.0.load(Ordering::SeqCst), 3);
    assert!(
        owner
            .observation()
            .result
            .unwrap()
            .snapshot
            .is_circuit_tripped()
    );
    assert!(peer.observation().result.is_none());
    let cleared = owner.clear(ScriptOperationId(4)).unwrap();
    assert_eq!(cleared.revision, 4);
    owner.run_request(&run_request(5)).unwrap();
    assert_eq!(
        engine.0.load(Ordering::SeqCst),
        3,
        "clear cannot bypass the breaker"
    );
    peer.run_request(&run_request(1)).unwrap();
    assert_eq!(
        engine.0.load(Ordering::SeqCst),
        4,
        "independent product owns its breaker"
    );
    assert_eq!(peer.observation().revision, 1);
    assert_eq!(owner.observation().revision, 5);
}
#[test]
fn prepared_exports_cancel_without_writes_reject_foreign_and_retired_identity_and_write_exact_bytes_once()
 {
    let writer = Arc::new(Writer::default());
    let owner = ScriptExportApplication::new(Some(writer.clone()));
    let peer = ScriptExportApplication::new(Some(writer.clone()));
    let first = owner.prepare(&draft()).unwrap();
    let foreign = peer.prepare(&draft()).unwrap();
    assert_ne!(first.identity, foreign.identity);
    assert_eq!(
        first.identity.sha256,
        content_sha256(&first.snapshot.content)
    );
    assert!(writer.calls.lock().unwrap().is_empty());
    assert_eq!(
        owner.confirm(&foreign.identity).unwrap_err().code,
        ErrorCode::InvalidState
    );
    owner.cancel(&first.identity).unwrap();
    assert!(owner.confirm(&first.identity).is_err());
    assert!(writer.calls.lock().unwrap().is_empty());
    let second = owner.prepare(&draft()).unwrap();
    assert!(owner.confirm(&first.identity).is_err());
    let saved = owner.confirm(&second.identity).unwrap();
    saved.validate(&second.identity).unwrap();
    assert_eq!(owner.confirm(&second.identity).unwrap(), saved);
    let calls = writer.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].content, second.snapshot.content);
    assert_eq!(calls[0].file_name, second.snapshot.file_name);
}
#[test]
fn permission_retry_retains_frozen_bytes_and_invalid_receipt_never_publishes_success() {
    let writer = Arc::new(Writer::default());
    let owner = ScriptExportApplication::new(Some(writer.clone()));
    let review = owner.prepare(&draft()).unwrap();
    *writer.failure.lock().unwrap() = true;
    assert_eq!(
        owner.confirm(&review.identity).unwrap_err().code,
        ErrorCode::Permission
    );
    assert_eq!(owner.snapshot().unwrap(), review.snapshot);
    *writer.failure.lock().unwrap() = false;
    *writer.incomplete.lock().unwrap() = true;
    assert_eq!(
        owner.confirm(&review.identity).unwrap_err().code,
        ErrorCode::InvalidState
    );
    assert!(!owner.snapshot().unwrap().outcome.is_saved());
    *writer.incomplete.lock().unwrap() = false;
    let saved = owner.confirm(&review.identity).unwrap();
    assert_eq!(saved.snapshot.content, review.snapshot.content);
    assert_eq!(
        writer.calls.lock().unwrap().len(),
        2,
        "real failed receipt and retry are separate writes"
    );
}
#[tokio::test]
async fn command_outputs_and_native_workbench_reject_unit_and_late_results_without_replacing_current_report()
 {
    let scripts = ScriptApplication::new();
    let commands = CommandApplication::new().with_scripts(
        scripts.clone(),
        ScriptExportApplication::without_host_port(),
    );
    let mut model = ScriptWorkbench::default();
    model.script_code = run_request(1).script_code;
    model.input_yaml = "mode: rule\n".into();
    let request = model.run().unwrap();
    assert!(model.run().is_none());
    model.edit_yaml("mode: global\n".into());
    assert_eq!(model.input_yaml, "mode: rule\n");
    let result = commands
        .execute_output(request.intent.clone())
        .await
        .unwrap();
    model.finish(request.operation + 1, Ok(result.clone()));
    assert!(model.pending.is_some());
    model.finish(request.operation, Ok(result));
    let observed = model.snapshot.clone();
    let next = model.run().unwrap();
    model.finish(request.operation, Ok(CommandOutput::Unit));
    assert_eq!(model.snapshot, observed);
    model.finish(next.operation, Ok(CommandOutput::Unit));
    assert_eq!(
        model.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    let retry = model.retry().unwrap();
    let output = commands.execute_output(retry.intent).await.unwrap();
    model.finish(retry.operation, Ok(output));
    assert!(model.failure.is_none());
    assert_eq!(scripts.observation().revision, 2);
    assert_eq!(
        CommandApplication::new()
            .execute_output(CommandIntent::RunScriptSandbox {
                request: run_request(1)
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
}

#[tokio::test]
async fn readers_replay_only_their_injected_product_owner_and_clear_is_observed_on_both_surfaces() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        Arc::new(TestRuntime),
    ));
    let owner = ScriptApplication::new();
    let isolated = ScriptApplication::new();
    let exports = ScriptExportApplication::without_host_port();
    let iced =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::IcedDesktop, HostKind::Desktop)
            .with_scripts(owner.clone(), exports.clone());
    let bevy =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_scripts(owner.clone(), exports.clone());
    let other = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_scripts(isolated, ScriptExportApplication::without_host_port());
    let result = owner.run_request(&run_request(1)).unwrap();
    let review = exports.prepare(&draft()).unwrap();
    for reader in [&iced, &bevy] {
        let snapshot = reader.read().await.unwrap();
        assert_eq!(snapshot.script_sandbox, Some(result.snapshot.clone()));
        assert_eq!(snapshot.script_export, Some(review.snapshot.clone()));
    }
    let snapshot = other.read().await.unwrap();
    assert!(snapshot.script_sandbox.is_none());
    assert!(snapshot.script_export.is_none());
    owner.clear(ScriptOperationId(2)).unwrap();
    exports.cancel(&review.identity).unwrap();
    for reader in [&iced, &bevy] {
        let snapshot = reader.read().await.unwrap();
        assert!(snapshot.script_sandbox.is_none());
        assert!(snapshot.script_export.is_none());
    }
}
