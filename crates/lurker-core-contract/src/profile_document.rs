//! Engine-neutral document validation vocabulary emitted by document codecs.

use serde::{Deserialize, Serialize};

/// Format-neutral diagnostic emitted by a document codec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnosticSnapshot {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

/// Result of inspecting one document with a format-specific codec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentValidation {
    Valid,
    Invalid {
        diagnostics: Vec<DocumentDiagnosticSnapshot>,
    },
}

#[cfg(test)]
mod tests {
    use super::{DocumentDiagnosticSnapshot, DocumentValidation};

    #[test]
    fn invalid_validation_round_trips_through_serde() {
        let validation = DocumentValidation::Invalid {
            diagnostics: vec![DocumentDiagnosticSnapshot {
                line: Some(3),
                column: Some(7),
                message: "invalid syntax".to_owned(),
            }],
        };
        let encoded = serde_json::to_string(&validation).expect("serialize validation");
        let decoded: DocumentValidation =
            serde_json::from_str(&encoded).expect("deserialize validation");
        assert_eq!(decoded, validation);
    }

    #[test]
    fn valid_and_invalid_are_distinct() {
        assert_ne!(
            DocumentValidation::Valid,
            DocumentValidation::Invalid {
                diagnostics: Vec::new()
            }
        );
    }
}
