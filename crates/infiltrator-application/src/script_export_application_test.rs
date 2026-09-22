//! DUAL-10-12: application-level export tests.
//!
//! The recording port below is a *host double*: it proves the application
//! really hands the composed artifact to the port and reports the receipt.
//! The real filesystem adapter lives in `infiltrator-desktop` and is tested
//! there against a real temporary directory.

use super::*;
use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
use infiltrator_domain::script_engine::HookStage;
use std::sync::Mutex;

/// Host double: records the exact request it was handed and answers a receipt.
#[derive(Default)]
struct RecordingExportPort {
    saved: Mutex<Vec<ScriptExportRequest>>,
}

impl RecordingExportPort {
    fn saved_requests(&self) -> Vec<ScriptExportRequest> {
        self.saved.lock().expect("recording lock").clone()
    }
}

impl ScriptExportPort for RecordingExportPort {
    fn save_export(
        &self,
        request: &ScriptExportRequest,
    ) -> Result<infiltrator_contract::script_export::ScriptExportReceipt, PortError> {
        let mut saved = self.saved.lock().expect("recording lock");
        saved.push(request.clone());
        Ok(infiltrator_contract::script_export::ScriptExportReceipt {
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
    let port = std::sync::Arc::new(RecordingExportPort::default());
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
    assert_eq!(last_script_export().as_ref(), Some(&snapshot));

    // The overlay export goes through the same port with its real YAML bytes.
    let overlay = application
        .export_mixin_overlay("main", "mode: rule\nport: 7890\n", "ipv6: true\n")
        .expect("overlay export");
    assert_eq!(overlay.file_name, "main.mixin.yaml");
    assert!(overlay.content.contains("ipv6: true"));
    assert_eq!(port.saved_requests().len(), 2);

    // The package export carries the real checksum and re-imports.
    let package = application
        .export_extension_package(&package())
        .expect("package export");
    let checksum = package.checksum.expect("checksum");
    assert_eq!(checksum.len(), 64);
    let restored: ExtensionPackage =
        serde_json::from_str(&package.content).expect("json package is real");
    assert_eq!(restored.name, "auto-country");
    assert_eq!(port.saved_requests().len(), 3);
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
    assert_eq!(last_script_export().as_ref(), Some(&snapshot));
}

#[test]
fn invalid_drafts_are_refused_before_any_port_call() {
    let port = std::sync::Arc::new(RecordingExportPort::default());
    let application = ScriptExportApplication::new(Some(port.clone()));
    assert!(
        application
            .export_mixin_overlay("main", "mode: rule\n", "mode: [bad\n")
            .is_err()
    );
    assert!(application.export_directive_dsl(None, "", None).is_err());
    assert!(application.export_preset("does-not-exist").is_err());
    assert!(port.saved_requests().is_empty());
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
    assert_eq!(last_script_export().as_ref(), Some(&first));
    clear_script_export();
    assert!(last_script_export().is_none());
    let second = application
        .export_preset("remove-ads")
        .expect("preset export");
    assert_eq!(second.file_name, "remove-ads.js");
    assert_eq!(last_script_export().as_ref(), Some(&second));
    assert_ne!(first, second);
}
