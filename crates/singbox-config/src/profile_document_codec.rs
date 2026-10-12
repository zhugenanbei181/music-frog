//! Pure sing-box JSON document inspection.

use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use infiltrator_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};

pub const PROFILE_FORMAT: &str = "sing-box-json";
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct SingBoxProfileDocumentCodec;

impl ProfileDocumentCodecPort for SingBoxProfileDocumentCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        ProfileDocumentCodecScope {
            engine_id: EngineId::sing_box(),
            format_id: ProfileFormatId::new(PROFILE_FORMAT)
                .expect("static sing-box format ID is valid"),
            schema_version: ProfileSchemaVersion::new(PROFILE_SCHEMA_VERSION)
                .expect("sing-box schema version is positive"),
        }
    }

    fn inspect(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, PortError> {
        let value: serde_json::Value = match serde_json::from_slice(raw_bytes) {
            Ok(value) => value,
            Err(error) => {
                return Ok(DocumentValidation::Invalid {
                    diagnostics: vec![DocumentDiagnosticSnapshot {
                        line: Some(error.line()),
                        column: Some(error.column()),
                        message: "invalid sing-box JSON syntax".to_owned(),
                    }],
                });
            }
        };

        if value.is_object() {
            Ok(DocumentValidation::Valid)
        } else {
            Ok(DocumentValidation::Invalid {
                diagnostics: vec![DocumentDiagnosticSnapshot {
                    line: None,
                    column: None,
                    message: "sing-box configuration root must be a JSON object".to_owned(),
                }],
            })
        }
    }
}

#[cfg(test)]
#[path = "profile_document_codec_tests.rs"]
mod tests;
