//! DUAL-10-12: desktop save-file adapter for script/Mixin exports.
//!
//! This workspace's desktop product ships no native file-dialog crate, so the
//! adapter writes the composed artifact into a host-owned `exports/`
//! directory next to the configs and reports the *real* path and byte count.
//! A host that cannot resolve that directory omits the port and every export
//! answers a typed unsupported outcome instead of a fabricated path.
//!
//! The file name arrives from the application as a sanitized single path
//! component; the adapter re-checks that contract and refuses anything that is
//! not a plain name, so an export can never escape the exports directory.

use infiltrator_contract::capability::Capability;
use infiltrator_contract::script_export::{ScriptExportReceipt, ScriptExportRequest};
use infiltrator_ports::error::PortError;
use infiltrator_ports::script_export::ScriptExportPort;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use tempfile::Builder;

/// Upper bound for a single export artifact (8 MiB): an overlay or script
/// larger than this is a bug, not a document.
pub const MAX_EXPORT_BYTES: usize = 8 * 1024 * 1024;

/// Desktop writer over `<config dir>/exports/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopScriptExportPort {
    exports_dir: PathBuf,
}

impl DesktopScriptExportPort {
    /// The adapter writes under `<config_dir>/exports/`.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            exports_dir: config_dir.into().join("exports"),
        }
    }

    pub fn exports_dir(&self) -> &Path {
        &self.exports_dir
    }

    fn safe_file_name(request: &ScriptExportRequest) -> Result<&str, PortError> {
        let file_name = request.file_name.trim();
        if file_name.is_empty() {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "导出文件名称为空",
            ));
        }
        let mut components = Path::new(file_name).components();
        let plain =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !plain {
            return Err(PortError::unsupported(
                Capability::Profiles,
                "导出文件名称必须是单一文件名（不允许目录跳转）",
            ));
        }
        Ok(file_name)
    }
}

impl ScriptExportPort for DesktopScriptExportPort {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        let file_name = Self::safe_file_name(request)?;
        if request.byte_len() > MAX_EXPORT_BYTES {
            return Err(PortError::unsupported(
                Capability::Profiles,
                format!("导出内容超过 {MAX_EXPORT_BYTES} 字节上限"),
            ));
        }
        fs::create_dir_all(&self.exports_dir).map_err(storage)?;
        let path = self.exports_dir.join(file_name);
        let mut temporary = Builder::new()
            .prefix(".script-export-")
            .tempfile_in(&self.exports_dir)
            .map_err(storage)?;
        temporary
            .write_all(request.content.as_bytes())
            .map_err(storage)?;
        temporary.as_file().sync_all().map_err(storage)?;
        let published = temporary
            .persist_noclobber(&path)
            .map_err(|error| storage(error.error))?;
        let metadata = published.metadata().map_err(storage)?;
        Ok(ScriptExportReceipt {
            path: path.to_string_lossy().to_string(),
            bytes_written: metadata.len() as usize,
        })
    }
}

fn storage(error: io::Error) -> PortError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use infiltrator_contract::error::ErrorCode;
    #[cfg(test)]
    use infiltrator_contract::script_export::DIRECTIVE_DSL_JS_HEADER;
    use infiltrator_contract::script_export::ScriptExportKind;
    #[cfg(test)]
    use std::env;
    #[cfg(test)]
    use std::process::id;
    #[cfg(test)]
    use std::time::SystemTime;
    #[cfg(test)]
    use std::time::UNIX_EPOCH;

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = format!(
            "mf-script-export-{tag}-{}-{}",
            id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        );
        env::temp_dir().join(unique)
    }

    fn request(file_name: &str) -> ScriptExportRequest {
        let content = format!(
            "{}\nfunction main(config) {{ return config; }}\n",
            DIRECTIVE_DSL_JS_HEADER
        );
        ScriptExportRequest {
            kind: ScriptExportKind::DirectiveDslScript,
            file_name: file_name.to_string(),
            media_type: ScriptExportKind::DirectiveDslScript
                .media_type()
                .to_string(),
            content,
        }
    }

    #[test]
    fn a_real_file_lands_in_the_exports_directory() {
        let dir = temp_dir("ok");
        let port = DesktopScriptExportPort::new(&dir);
        let receipt = port.save_export(&request("main.js")).expect("write");
        assert_eq!(receipt.bytes_written, request("main.js").byte_len());
        let path = PathBuf::from(&receipt.path);
        assert!(path.starts_with(port.exports_dir()));
        let written = fs::read_to_string(&path).expect("real file");
        assert!(written.contains("不是 JavaScript"));
        assert!(written.contains("function main(config)"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_export_and_symlink_targets_are_never_overwritten() {
        let dir = temp_dir("collision");
        let port = DesktopScriptExportPort::new(&dir);
        fs::create_dir_all(port.exports_dir()).unwrap();
        let existing = port.exports_dir().join("main.js");
        fs::write(&existing, "user-owned bytes").unwrap();
        assert!(port.save_export(&request("main.js")).is_err());
        assert_eq!(fs::read_to_string(&existing).unwrap(), "user-owned bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target = dir.join("external.txt");
            fs::write(&target, "external bytes").unwrap();
            symlink(&target, port.exports_dir().join("linked.js")).unwrap();
            assert!(port.save_export(&request("linked.js")).is_err());
            assert_eq!(fs::read_to_string(target).unwrap(), "external bytes");
        }
        assert_eq!(
            fs::read_dir(port.exports_dir())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".script-export-"))
                .count(),
            0
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_traversal_name_is_refused_before_any_write() {
        let dir = temp_dir("traversal");
        let port = DesktopScriptExportPort::new(&dir);
        for name in ["../escape.js", "nested/escape.js", "/etc/passwd", "..", ""] {
            let error = port.save_export(&request(name)).expect_err("refused");
            assert_eq!(error.error_code(), ErrorCode::Unsupported, "name {name:?}");
        }
        assert!(
            !dir.exists(),
            "no directory may be created for a refused name"
        );
    }

    #[test]
    fn an_oversized_artifact_is_refused_with_a_typed_reason() {
        let dir = temp_dir("oversize");
        let port = DesktopScriptExportPort::new(&dir);
        let mut oversized = request("big.js");
        oversized.content = "x".repeat(MAX_EXPORT_BYTES + 1);
        assert!(port.save_export(&oversized).is_err());
        assert!(!dir.exists());
    }
}
