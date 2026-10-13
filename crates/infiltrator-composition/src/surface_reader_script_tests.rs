//! Composition-level proof that shared script owners publish consistently across surfaces.
use super::*;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_export_review::{ScriptExportDraft, ScriptExportValidationScope};
use infiltrator_contract::script_run::{ScriptOperationId, ScriptRunRequest};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;

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
        validation_scope: ScriptExportValidationScope::Unscoped,
        profile: None,
        base_yaml: String::new(),
        mixin_yaml: String::new(),
        script_code: run_request(1).script_code,
        preset: None,
    }
}

#[tokio::test]
async fn readers_replay_only_their_injected_product_owner_and_clear_is_observed_on_both_surfaces() {
    let runtime = crate::tokio_application_runtime().expect("composition runtime");
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    ));
    let owner = ScriptApplication::new();
    let isolated = ScriptApplication::new();
    let exports = ScriptExportApplication::without_host_port();
    let iced = ProductSurfaceReader::new(core.clone(), SurfaceKind::IcedDesktop, HostKind::Desktop)
        .with_scripts(owner.clone(), exports.clone());
    let bevy = ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_scripts(owner.clone(), exports.clone());
    let other = ProductSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
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
