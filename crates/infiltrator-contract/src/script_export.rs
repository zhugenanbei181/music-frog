//! DUAL-10-12: the shared extension-export read model.
//!
//! Group 10's export item is about a file the user can really obtain. This
//! workspace bundles **no JavaScript engine**, so an exported `.js`-named file
//! is a *directive-DSL* script carrying a mandatory honesty header — it is
//! never presented as JavaScript ([`ScriptExportKind::is_javascript`] is
//! always `false`). The three kinds share one read model so both surfaces
//! render the same file name, bytes, checksum and host outcome.

use serde::{Deserialize, Serialize};

/// Mandatory first lines of a `.js`-named directive-DSL export.
///
/// The file is named `.js` because that is the ecosystem's script slot, but its
/// body is the Music Frog directive DSL. The header states that fact in both
/// languages so a recipient cannot mistake it for JavaScript.
pub const DIRECTIVE_DSL_JS_HEADER: &str = "// Music Frog directive-DSL script export — NOT JavaScript.\n// 本文件不是 JavaScript：它是「指令 DSL」覆写脚本，由正则指令识别器解析，不会被 JS 引擎执行。";

/// Mandatory first lines of a Mixin overlay YAML export.
pub const MIXIN_OVERLAY_YAML_HEADER: &str = "# Music Frog Mixin 覆写导出（YAML 文档，非 JavaScript）。\n# 保存/导入时经共享保真引擎合并并校验，注释行不影响解析。";

/// The honest note both surfaces render next to a directive-DSL `.js` export.
pub const DIRECTIVE_DSL_JS_NOTE_ZH: &str = "该 `.js` 文件是指令 DSL 脚本（非 JavaScript），带「不是 JavaScript」文件头，不会被 JS 引擎执行";

/// The honest note both surfaces render next to a Mixin overlay YAML export.
pub const MIXIN_OVERLAY_YAML_NOTE_ZH: &str =
    "该 `.yaml` 文件是真实的 Mixin 覆写文档，可直接作为覆写导入并经共享预检合并";

/// Which shared export a surface is asking for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptExportKind {
    /// The edited Mixin overlay document, as a real `.yaml` file.
    MixinOverlayYaml,
    /// The edited sandbox script, as a `.js`-named directive-DSL text file.
    DirectiveDslScript,
    /// The full extension package (JSON) with its SHA-256 checksum.
    ExtensionPackageJson,
}

impl ScriptExportKind {
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => "yaml",
            Self::DirectiveDslScript => "js",
            Self::ExtensionPackageJson => "json",
        }
    }

    /// The file-name suffix appended after the sanitized stem.
    pub const fn file_suffix(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => ".mixin.yaml",
            Self::DirectiveDslScript => ".js",
            Self::ExtensionPackageJson => ".ext.json",
        }
    }

    pub const fn media_type(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => "application/yaml",
            // The `.js` export is deliberately not `text/javascript`: its body
            // is the directive DSL, and the media type must not claim a JS
            // program the runtime cannot execute.
            Self::DirectiveDslScript => "text/plain",
            Self::ExtensionPackageJson => "application/json",
        }
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => "Mixin 覆写 YAML",
            Self::DirectiveDslScript => "指令 DSL 脚本 (.js)",
            Self::ExtensionPackageJson => "扩展包 (JSON)",
        }
    }

    pub const fn label_key(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => "script_export_kind_mixin_yaml",
            Self::DirectiveDslScript => "script_export_kind_directive_js",
            Self::ExtensionPackageJson => "script_export_kind_package_json",
        }
    }

    /// Never `true`: no bundled engine executes these files.
    pub const fn is_javascript(self) -> bool {
        false
    }

    /// The honest note for this kind, rendered by both surfaces.
    pub const fn honest_note_zh(self) -> &'static str {
        match self {
            Self::MixinOverlayYaml => MIXIN_OVERLAY_YAML_NOTE_ZH,
            Self::DirectiveDslScript => DIRECTIVE_DSL_JS_NOTE_ZH,
            Self::ExtensionPackageJson => {
                "该 JSON 包携带 SHA-256 校验和，可再次导入校验，与 `.js` 执行无关"
            }
        }
    }
}

/// Exactly the bytes a host save port is asked to persist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportRequest {
    pub kind: ScriptExportKind,
    /// Single path component the host may use as the file name.
    pub file_name: String,
    pub media_type: String,
    /// The artifact bytes, including the mandatory honesty header.
    pub content: String,
}

impl ScriptExportRequest {
    pub fn byte_len(&self) -> usize {
        self.content.len()
    }
}

/// What the host really wrote, reported by the adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportReceipt {
    /// Real path the host wrote to (a save dialog result or an owned export
    /// directory). Never a guess.
    pub path: String,
    pub bytes_written: usize,
}

/// The host outcome of one export attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum ScriptExportOutcome {
    /// The artifact was composed but no host save port exists for this host.
    Prepared,
    /// The host persisted the artifact at a real path.
    Saved { path: String, bytes_written: usize },
    /// The host has no file dialog / export location at all. Typed, never a
    /// silent success.
    Unsupported { reason: String },
    /// The host attempted the write and it really failed.
    Failed { reason: String },
}

impl ScriptExportOutcome {
    pub const fn is_saved(&self) -> bool {
        matches!(self, Self::Saved { .. })
    }

    pub const fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }

    pub fn saved_path(&self) -> Option<&str> {
        match self {
            Self::Saved { path, .. } => Some(path.as_str()),
            _ => None,
        }
    }

    pub const fn label_zh(&self) -> &'static str {
        match self {
            Self::Prepared => "已生成（未写入）",
            Self::Saved { .. } => "已导出到磁盘",
            Self::Unsupported { .. } => "宿主不支持文件对话框",
            Self::Failed { .. } => "导出失败",
        }
    }
}

/// The one export fact both surfaces render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptExportSnapshot {
    pub kind: ScriptExportKind,
    /// The profile the artifact belongs to (Mixin overlay), when any.
    pub profile: Option<String>,
    pub file_name: String,
    pub media_type: String,
    /// The real artifact bytes (header included).
    pub content: String,
    /// SHA-256 of the artifact for the JSON package kind.
    pub checksum: Option<String>,
    pub honest_note: String,
    pub outcome: ScriptExportOutcome,
}

impl ScriptExportSnapshot {
    pub fn byte_len(&self) -> usize {
        self.content.len()
    }

    pub fn is_javascript(&self) -> bool {
        self.kind.is_javascript()
    }

    /// A real prefix of the artifact for a bounded surface box, with an
    /// explicit truncation marker (never a fabricated body).
    pub fn content_preview(&self, max_chars: usize) -> String {
        let mut preview: String = self.content.chars().take(max_chars).collect();
        if self.content.chars().count() > max_chars {
            preview.push_str(" …");
        }
        preview
    }

    pub fn outcome_label_zh(&self) -> &'static str {
        self.outcome.label_zh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(kind: ScriptExportKind, outcome: ScriptExportOutcome) -> ScriptExportSnapshot {
        ScriptExportSnapshot {
            kind,
            profile: Some("main".to_string()),
            file_name: format!("main{}", kind.file_suffix()),
            media_type: kind.media_type().to_string(),
            content: "body".to_string(),
            checksum: None,
            honest_note: kind.honest_note_zh().to_string(),
            outcome,
        }
    }

    #[test]
    fn no_export_kind_ever_claims_javascript() {
        for kind in [
            ScriptExportKind::MixinOverlayYaml,
            ScriptExportKind::DirectiveDslScript,
            ScriptExportKind::ExtensionPackageJson,
        ] {
            assert!(!kind.is_javascript());
            assert_ne!(kind.media_type(), "text/javascript");
        }
        assert_eq!(ScriptExportKind::DirectiveDslScript.file_extension(), "js");
        assert!(DIRECTIVE_DSL_JS_HEADER.contains("不是 JavaScript"));
        assert!(DIRECTIVE_DSL_JS_NOTE_ZH.contains("非 JavaScript"));
    }

    #[test]
    fn outcome_vocabulary_is_typed_and_serializable() {
        let saved = ScriptExportOutcome::Saved {
            path: "/tmp/main.mixin.yaml".to_string(),
            bytes_written: 42,
        };
        assert!(saved.is_saved());
        assert_eq!(saved.saved_path(), Some("/tmp/main.mixin.yaml"));
        assert!(
            ScriptExportOutcome::Unsupported {
                reason: "no file dialog".to_string(),
            }
            .is_unsupported()
        );
        let json = serde_json::to_string(&saved).expect("serialize outcome");
        let restored: ScriptExportOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, saved);
    }

    #[test]
    fn snapshot_preview_truncates_honestly_and_round_trips() {
        let mut snapshot = snapshot(
            ScriptExportKind::DirectiveDslScript,
            ScriptExportOutcome::Prepared,
        );
        snapshot.content = "abcdef".to_string();
        assert_eq!(snapshot.content_preview(4), "abcd …");
        assert_eq!(snapshot.content_preview(6), "abcdef");
        assert!(!snapshot.is_javascript());
        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let restored: ScriptExportSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, snapshot);
    }
}
