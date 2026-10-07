//! DUAL-10-12: the shared export use-case both surfaces call.
//!
//! The artifact is composed by the pure
//! [`infiltrator_domain::script_export`] builders (real bytes, mandatory
//! honesty header, real SHA-256 for packages) and then handed to the host's
//! [`ScriptExportPort`]. The port owns the destination decision: a native save
//! dialog or a host-owned export directory writes a real file, while a host
//! without either answers a typed `Unsupported` outcome — the projection still
//! carries the composed content so nothing is fabricated and nothing is lost.
//!
//! Each product session injects a service shared by its command facade and
//! reader. Preparing a review freezes bytes without saving; only confirmation
//! may call the host. Results cannot be read from another product's memory.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::{
    ScriptExportOutcome, ScriptExportRequest, ScriptExportSnapshot,
};
use infiltrator_domain::script_engine::{ExtensionPackage, ScriptEngine};
mod review;
use infiltrator_contract::script_export_review::{ScriptExportReview, ScriptExportSaved};
use infiltrator_domain::script_export::{
    ExportArtifact, compose_directive_dsl_export, compose_extension_package_export,
    compose_mixin_overlay_export,
};
use infiltrator_domain::script_export_review::draft_package;
use infiltrator_ports::error::PortError;
use infiltrator_ports::script_export::ScriptExportPort;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct ExportState {
    sequence: u64,
    latest: Option<ScriptExportSnapshot>,
    review: Option<ScriptExportReview>,
    saved: Option<ScriptExportSaved>,
    saving: bool,
}

/// The shared export service.
#[derive(Clone)]
pub struct ScriptExportApplication {
    port: Option<Arc<dyn ScriptExportPort>>,
    state: Arc<Mutex<ExportState>>,
    owner: u64,
}

impl Default for ScriptExportApplication {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ScriptExportApplication {
    pub fn new(port: Option<Arc<dyn ScriptExportPort>>) -> Self {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        let owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
        assert!(owner != 0, "Script export owner identity exhausted");
        Self {
            port,
            state: Arc::default(),
            owner,
        }
    }

    /// A host with no save-file adapter: every export reports a typed
    /// unsupported outcome while still carrying the real artifact bytes.
    pub fn without_host_port() -> Self {
        Self::new(None)
    }

    pub fn has_host_port(&self) -> bool {
        self.port.is_some()
    }

    pub fn snapshot(&self) -> Option<ScriptExportSnapshot> {
        self.state
            .lock()
            .expect("script export state")
            .latest
            .clone()
    }

    pub fn clear_history(&self) {
        self.state.lock().expect("script export state").latest = None;
    }

    /// Export the edited Mixin overlay as a real YAML document.
    pub fn export_mixin_overlay(
        &self,
        profile: &str,
        base_yaml: &str,
        mixin_yaml: &str,
    ) -> Result<ScriptExportSnapshot, Failure> {
        let artifact = compose_mixin_overlay_export(profile, base_yaml, mixin_yaml)
            .map_err(|error| export_failure(ErrorCode::Configuration, error))?;
        Ok(self.save(artifact, Some(profile.to_string())))
    }

    /// Export the sandbox script as a `.js`-named directive-DSL file.
    pub fn export_directive_dsl(
        &self,
        requested_stem: Option<&str>,
        script_code: &str,
        preset: Option<&str>,
    ) -> Result<ScriptExportSnapshot, Failure> {
        let artifact = compose_directive_dsl_export(requested_stem, script_code, preset)
            .map_err(|error| export_failure(ErrorCode::Configuration, error))?;
        Ok(self.save(artifact, None))
    }

    /// Export a built-in preset by id through the shared catalogue.
    pub fn export_preset(&self, preset_id: &str) -> Result<ScriptExportSnapshot, Failure> {
        let preset = ScriptEngine::find_preset(preset_id).ok_or_else(|| {
            export_failure(
                ErrorCode::InvalidInput,
                format!("未知的脚本预设: {preset_id}"),
            )
        })?;
        self.export_directive_dsl(Some(preset.id), preset.script_code, Some(preset.id))
    }

    /// Export the full extension package (JSON + real SHA-256 checksum).
    pub fn export_extension_package(
        &self,
        package: &ExtensionPackage,
    ) -> Result<ScriptExportSnapshot, Failure> {
        let artifact = compose_extension_package_export(package)
            .map_err(|error| export_failure(ErrorCode::Internal, error))?;
        Ok(self.save(artifact, None))
    }

    /// Assemble the shareable package from a surface's live drafts (script,
    /// optional overlay, optional preset) and export it. The stage comes from
    /// the shared preset catalogue exactly like a sandbox run.
    pub fn export_draft_package(
        &self,
        name: &str,
        profile: Option<&str>,
        script_code: &str,
        mixin_yaml: Option<&str>,
        preset: Option<&str>,
    ) -> Result<ScriptExportSnapshot, Failure> {
        let package = draft_package(name, profile, script_code, mixin_yaml, preset);
        self.export_extension_package(&package)
    }

    fn save(&self, artifact: ExportArtifact, profile: Option<String>) -> ScriptExportSnapshot {
        let outcome = self.persist(&artifact);
        let snapshot = ScriptExportSnapshot {
            kind: artifact.kind,
            profile,
            file_name: artifact.file_name,
            media_type: artifact.media_type,
            content: artifact.content,
            checksum: artifact.checksum,
            honest_note: artifact.honest_note,
            outcome,
        };
        self.state.lock().expect("script export state").latest = Some(snapshot.clone());
        snapshot
    }

    fn persist(&self, artifact: &ExportArtifact) -> ScriptExportOutcome {
        let Some(port) = self.port.as_ref() else {
            return ScriptExportOutcome::Unsupported {
                reason: "当前宿主没有文件保存对话框或导出目录，导出内容未写入磁盘".to_string(),
            };
        };
        let request = ScriptExportRequest {
            kind: artifact.kind,
            file_name: artifact.file_name.clone(),
            media_type: artifact.media_type.clone(),
            content: artifact.content.clone(),
        };
        match port.save_export(&request) {
            Ok(receipt) => ScriptExportOutcome::Saved {
                path: receipt.path,
                bytes_written: receipt.bytes_written,
            },
            Err(PortError::Unsupported { reason, .. }) => {
                ScriptExportOutcome::Unsupported { reason }
            }
            Err(error) => ScriptExportOutcome::Failed {
                reason: error.to_string(),
            },
        }
    }
}

fn export_failure(code: ErrorCode, message: impl Into<String>) -> Failure {
    Failure::new(code, message, false)
}

#[cfg(test)]
#[path = "script_export_application_test.rs"]
mod script_export_application_test;
