//! Engine-neutral document validation vocabulary emitted by document codecs.

use serde::{Deserialize, Serialize};

/// Format-neutral diagnostic emitted by a document codec.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDiagnosticSnapshot {
    /// Zero-based line the diagnostic points at, when the codec reports one.
    pub line: Option<usize>,
    /// Zero-based column the diagnostic points at, when the codec reports one.
    pub column: Option<usize>,
    /// Human-readable description of the problem.
    pub message: String,
}

/// Result of inspecting one document with a format-specific codec.
///
/// The result is a closed vocabulary: a codec never returns "probably fine",
/// it either reports [`DocumentValidation::Valid`] or lists the diagnostics
/// that made the document invalid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentValidation {
    /// The document parses and satisfies every codec rule.
    Valid,
    /// The document is invalid; the diagnostics explain why.
    Invalid {
        /// Every diagnostic the codec produced, in codec order.
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
