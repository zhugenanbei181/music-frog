//! DUAL-10-12: host port for saving an export artifact.
//!
//! The application composes the artifact (file name + real bytes) and this
//! port owns the host decision of *where* it lands: a native save dialog, or a
//! host-owned export directory. A host that offers neither implements
//! [`UnsupportedScriptExportPort`] and every export answers
//! [`infiltrator_ports::error::PortError::unsupported`] — the surface then
//! shows the composed content plus a typed "this host has no file dialog"
//! outcome instead of pretending a file exists.

use crate::error::PortError;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::script_export::{ScriptExportReceipt, ScriptExportRequest};

/// Persist one composed export artifact and report the real path written.
///
/// Synchronous on purpose: the composition is tiny, the adapter is a plain
/// host file write, and a synchronous seam lets the shared regression matrix
/// exercise the whole export path without an executor.
pub trait ScriptExportPort: Send + Sync {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError>;
}

/// Typed answer for hosts without any file dialog or export directory.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedScriptExportPort;

impl ScriptExportPort for UnsupportedScriptExportPort {
    fn save_export(
        &self,
        _request: &ScriptExportRequest,
    ) -> Result<ScriptExportReceipt, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "当前宿主没有文件保存对话框或导出目录，导出内容未写入磁盘",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::script_export::ScriptExportKind;

    fn request() -> ScriptExportRequest {
        ScriptExportRequest {
            kind: ScriptExportKind::DirectiveDslScript,
            file_name: "main.js".to_string(),
            media_type: ScriptExportKind::DirectiveDslScript
                .media_type()
                .to_string(),
            content: "// not JavaScript\nfunction main(config) { return config; }\n".to_string(),
        }
    }

    #[test]
    fn unsupported_host_answers_with_a_typed_capability_error() {
        let port = UnsupportedScriptExportPort;
        let error = port.save_export(&request()).expect_err("must not save");
        assert_eq!(
            error.error_code(),
            infiltrator_contract::error::ErrorCode::Unsupported
        );
        let failure: infiltrator_contract::error::Failure = error.into();
        assert_eq!(
            failure.code,
            infiltrator_contract::error::ErrorCode::Unsupported
        );
        assert!(!failure.retryable);
        assert!(failure.message.contains("Profiles"));
    }
}
