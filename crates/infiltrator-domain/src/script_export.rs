//! DUAL-10-12: pure composition of the exported artifacts.
//!
//! Given the very bytes the surfaces already edit, these functions produce the
//! file a user could really obtain. Runtime truth: **no JavaScript engine is
//! bundled**, so the `.js`-named export is the directive DSL with a mandatory
//! honesty header from
//! [`infiltrator_contract::script_export::DIRECTIVE_DSL_JS_HEADER`]; nothing
//! here fabricates a JS program or claims execution.
//!
//! The functions are pure (no I/O, no executor): the application use-case
//! composes the artifact and the host port decides where it is written.

use crate::mixin_studio;
use crate::script_engine::{ExtensionPackage, ScriptEngine};
use infiltrator_contract::script_export::{
    DIRECTIVE_DSL_JS_HEADER, DIRECTIVE_DSL_JS_NOTE_ZH, MIXIN_OVERLAY_YAML_HEADER,
    MIXIN_OVERLAY_YAML_NOTE_ZH, ScriptExportKind,
};

/// A composed export artifact: exactly what the host is asked to persist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportArtifact {
    pub kind: ScriptExportKind,
    pub file_name: String,
    pub media_type: String,
    pub content: String,
    /// SHA-256 for kinds whose content carries an integrity check.
    pub checksum: Option<String>,
    pub honest_note: String,
}

impl ExportArtifact {
    fn new(
        kind: ScriptExportKind,
        file_name: String,
        content: String,
        checksum: Option<String>,
        honest_note: &str,
    ) -> Self {
        Self {
            kind,
            file_name,
            media_type: kind.media_type().to_string(),
            content,
            checksum,
            honest_note: honest_note.to_string(),
        }
    }

    pub fn byte_len(&self) -> usize {
        self.content.len()
    }
}

/// Turn a user/host-supplied stem into a safe single path component.
///
/// Only ASCII alphanumerics, `-`, `_` and `.` survive; everything else becomes
/// `-`. An empty or all-separator input falls back to `export`, and the stem is
/// bounded so an export can never overflow the file-name limit.
pub fn sanitize_file_stem(raw: &str) -> String {
    let mut stem: String = raw
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect();
    while stem.starts_with('.') || stem.starts_with('-') {
        stem.remove(0);
    }
    while stem.ends_with('.') || stem.ends_with('-') {
        stem.pop();
    }
    stem.truncate(64);
    if stem.is_empty() {
        "export".to_string()
    } else {
        stem
    }
}

/// Compose the `.yaml` export of the Mixin overlay draft.
///
/// The draft must pass the shared preflight (parse + fidelity merge + output
/// validation) against the open base document: exporting an overlay the kernel
/// could never load would be a fabricated artifact.
pub fn compose_mixin_overlay_export(
    profile: &str,
    base_yaml: &str,
    mixin_yaml: &str,
) -> Result<ExportArtifact, String> {
    if mixin_yaml.trim().is_empty() {
        return Err("Mixin 覆写为空，没有可导出的内容".to_string());
    }
    let report = mixin_studio::preflight_mixin(base_yaml, mixin_yaml);
    if report.is_blocking() {
        return Err(report
            .error
            .unwrap_or_else(|| "Mixin 覆写未通过共享预检".to_string()));
    }
    let stem = sanitize_file_stem(profile);
    let file_name = format!("{stem}{}", ScriptExportKind::MixinOverlayYaml.file_suffix());
    // The header is YAML comments, so the file still parses to the same
    // overlay document through the shared codec.
    let content = format!("{MIXIN_OVERLAY_YAML_HEADER}\n{mixin_yaml}");
    Ok(ExportArtifact::new(
        ScriptExportKind::MixinOverlayYaml,
        file_name,
        content,
        None,
        MIXIN_OVERLAY_YAML_NOTE_ZH,
    ))
}

/// Compose the `.js`-named directive-DSL export of the sandbox script.
///
/// The script must pass the shared validator (entry point present, no
/// `while(true)`-style infinite loop) so the exported file is runnable by the
/// directive DSL. The content always opens with
/// [`DIRECTIVE_DSL_JS_HEADER`], which states it is not JavaScript.
pub fn compose_directive_dsl_export(
    requested_stem: Option<&str>,
    script_code: &str,
    preset: Option<&str>,
) -> Result<ExportArtifact, String> {
    if script_code.trim().is_empty() {
        return Err("脚本为空，没有可导出的内容".to_string());
    }
    let validation = ScriptEngine::validate_script(script_code);
    if !validation.valid {
        return Err(validation
            .error
            .unwrap_or_else(|| "脚本未通过共享校验".to_string()));
    }
    let stem = requested_stem
        .or(preset)
        .map(sanitize_file_stem)
        .unwrap_or_else(|| "script".to_string());
    let file_name = format!(
        "{stem}{}",
        ScriptExportKind::DirectiveDslScript.file_suffix()
    );
    let content = format!("{DIRECTIVE_DSL_JS_HEADER}\n{script_code}\n");
    Ok(ExportArtifact::new(
        ScriptExportKind::DirectiveDslScript,
        file_name,
        content,
        None,
        DIRECTIVE_DSL_JS_NOTE_ZH,
    ))
}

/// Compose the JSON extension-package export with its real SHA-256 checksum.
pub fn compose_extension_package_export(
    package: &ExtensionPackage,
) -> Result<ExportArtifact, String> {
    let json =
        ScriptEngine::export_extension_package(package).map_err(|error| error.to_string())?;
    let checksum = package.calculate_checksum();
    if !package.verify_checksum(&checksum) {
        return Err("扩展包校验和自校验失败".to_string());
    }
    let file_name = format!(
        "{}{}",
        sanitize_file_stem(&package.name),
        ScriptExportKind::ExtensionPackageJson.file_suffix()
    );
    Ok(ExportArtifact::new(
        ScriptExportKind::ExtensionPackageJson,
        file_name,
        json,
        Some(checksum),
        ScriptExportKind::ExtensionPackageJson.honest_note_zh(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::script_engine::HookStage;

    use super::*;

    const BASE: &str = "mode: rule\nport: 7890\n";

    #[test]
    fn overlay_export_is_a_real_yaml_document_that_parses_back() {
        let artifact =
            compose_mixin_overlay_export("main", BASE, "ipv6: true\n").expect("compose overlay");
        assert_eq!(artifact.kind, ScriptExportKind::MixinOverlayYaml);
        assert_eq!(artifact.file_name, "main.mixin.yaml");
        assert!(artifact.content.starts_with("# Music Frog Mixin"));
        // The exported bytes re-parse through the shared gate: a real document.
        let round_trip = mixin_studio::preflight_mixin(BASE, &artifact.content);
        assert!(round_trip.valid, "{:?}", round_trip.error);
        assert!(
            round_trip
                .merged_preview
                .expect("preview")
                .contains("ipv6: true")
        );
    }

    #[test]
    fn overlay_export_refuses_an_unmergeable_or_empty_draft() {
        assert!(compose_mixin_overlay_export("main", BASE, "   ").is_err());
        assert!(compose_mixin_overlay_export("main", BASE, "mode: [bad\n").is_err());
    }

    #[test]
    fn js_export_states_it_is_not_javascript() {
        let script =
            "function main(config, profile) {\n  auto_country_groups(config);\n  return config;\n}";
        let artifact =
            compose_directive_dsl_export(Some("HK-Group"), script, None).expect("compose js");
        assert_eq!(artifact.kind, ScriptExportKind::DirectiveDslScript);
        assert_eq!(artifact.file_name, "HK-Group.js");
        assert!(!artifact.kind.is_javascript());
        assert!(artifact.content.contains("不是 JavaScript"));
        assert!(artifact.content.contains("NOT JavaScript"));
        assert!(artifact.content.contains("auto_country_groups(config)"));
    }

    #[test]
    fn js_export_refuses_script_text_the_shared_validator_rejects() {
        assert!(compose_directive_dsl_export(None, "", None).is_err());
        assert!(compose_directive_dsl_export(None, "let x = 42;", None).is_err());
        assert!(
            compose_directive_dsl_export(None, "function main(c) { while(true) {} }", None)
                .is_err()
        );
    }

    #[test]
    fn package_export_carries_the_real_checksum() {
        let package = ExtensionPackage {
            name: "国家地区自动分组".to_string(),
            version: "1.0.0".to_string(),
            author: "tester".to_string(),
            description: "test".to_string(),
            stage: HookStage::PreMerge,
            script_code: "function main(config) { return config; }".to_string(),
            mixin_yaml: Some("ipv6: true\n".to_string()),
            tags: vec!["export".to_string()],
        };
        let artifact = compose_extension_package_export(&package).expect("compose package");
        let checksum = artifact.checksum.clone().expect("checksum");
        assert_eq!(checksum.len(), 64);
        assert_eq!(checksum, package.calculate_checksum());
        let restored: ExtensionPackage =
            serde_json::from_str(&artifact.content).expect("real json package");
        assert_eq!(restored, package);
        // A fully non-ASCII package name sanitizes to the `export` fallback.
        assert_eq!(artifact.file_name, "export.ext.json");
    }

    #[test]
    fn file_stems_are_sanitized_to_a_single_component() {
        assert_eq!(sanitize_file_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_file_stem("香港 main"), "main");
        assert_eq!(sanitize_file_stem("   "), "export");
        assert_eq!(sanitize_file_stem("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_file_stem(&"x".repeat(200)).len(), 64);
    }
}
