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
//! The last projection is cached process-wide and republished through
//! `SurfaceSnapshot.script_export`, so the Bevy console renders exactly what
//! the Iced console exported.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::{
    ScriptExportOutcome, ScriptExportRequest, ScriptExportSnapshot,
};
use infiltrator_domain::script_engine::ExtensionPackage;
use infiltrator_ports::error::PortError;
use infiltrator_ports::script_export::ScriptExportPort;
use std::sync::{Arc, Mutex, OnceLock};

/// Process-wide cache of the last export projection (one shared fact).
fn export_cache() -> &'static Mutex<Option<ScriptExportSnapshot>> {
    static EXPORT: OnceLock<Mutex<Option<ScriptExportSnapshot>>> = OnceLock::new();
    EXPORT.get_or_init(|| Mutex::new(None))
}

/// The last export projection computed in this process, if any.
pub fn last_script_export() -> Option<ScriptExportSnapshot> {
    export_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Replace the process-wide export projection.
pub fn publish_script_export(snapshot: ScriptExportSnapshot) {
    if let Ok(mut cache) = export_cache().lock() {
        *cache = Some(snapshot);
    }
}

/// Drop the cached projection.
pub fn clear_script_export() {
    if let Ok(mut cache) = export_cache().lock() {
        *cache = None;
    }
}

/// The shared export service.
#[derive(Clone, Default)]
pub struct ScriptExportApplication {
    port: Option<Arc<dyn ScriptExportPort>>,
}

impl ScriptExportApplication {
    pub fn new(port: Option<Arc<dyn ScriptExportPort>>) -> Self {
        Self { port }
    }

    /// A host with no save-file adapter: every export reports a typed
    /// unsupported outcome while still carrying the real artifact bytes.
    pub fn without_host_port() -> Self {
        Self { port: None }
    }

    pub fn has_host_port(&self) -> bool {
        self.port.is_some()
    }

    /// Export the edited Mixin overlay as a real YAML document.
    pub fn export_mixin_overlay(
        &self,
        profile: &str,
        base_yaml: &str,
        mixin_yaml: &str,
    ) -> Result<ScriptExportSnapshot, Failure> {
        let artifact = infiltrator_domain::script_export::compose_mixin_overlay_export(
            profile, base_yaml, mixin_yaml,
        )
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
        let artifact = infiltrator_domain::script_export::compose_directive_dsl_export(
            requested_stem,
            script_code,
            preset,
        )
        .map_err(|error| export_failure(ErrorCode::Configuration, error))?;
        Ok(self.save(artifact, None))
    }

    /// Export a built-in preset by id through the shared catalogue.
    pub fn export_preset(&self, preset_id: &str) -> Result<ScriptExportSnapshot, Failure> {
        let preset = infiltrator_domain::script_engine::ScriptEngine::find_preset(preset_id)
            .ok_or_else(|| {
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
        let artifact = infiltrator_domain::script_export::compose_extension_package_export(package)
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
        let stage = preset
            .and_then(infiltrator_domain::script_engine::ScriptEngine::find_preset)
            .map(|preset| preset.stage)
            .unwrap_or_default();
        let package = ExtensionPackage {
            name: name.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "Music Frog 指令 DSL 工作台".to_string(),
            description: match profile {
                Some(profile) => format!("从配置 {profile} 导出的指令 DSL 覆写包"),
                None => "从脚本沙箱控制台导出的指令 DSL 覆写包".to_string(),
            },
            stage,
            script_code: script_code.to_string(),
            mixin_yaml: mixin_yaml
                .filter(|text| !text.trim().is_empty())
                .map(str::to_string),
            tags: vec!["music-frog".to_string(), "directive-dsl".to_string()],
        };
        self.export_extension_package(&package)
    }

    fn save(
        &self,
        artifact: infiltrator_domain::script_export::ExportArtifact,
        profile: Option<String>,
    ) -> ScriptExportSnapshot {
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
        publish_script_export(snapshot.clone());
        snapshot
    }

    fn persist(
        &self,
        artifact: &infiltrator_domain::script_export::ExportArtifact,
    ) -> ScriptExportOutcome {
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
