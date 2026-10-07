//! One redacted byte projection for every log export surface and host.
use crate::profile_source::hash_document_bytes;
use crate::redact::redact_line;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{
    LogExportArtifact, LogExportIdentity, LogExportSummary, MAX_LOG_EXPORT_BYTES,
};
use infiltrator_contract::logs::LogSession;

pub fn prepare_log_export(
    session: LogSession,
    sequence: u64,
    records: &[(u64, String)],
    secrets: &[String],
) -> Result<LogExportArtifact, Failure> {
    let mut content = String::new();
    for (_, raw) in records {
        // Explicit credentials must be removed even when short. Exporting
        // diagnostics may obscure extra text, but must never retain a known key.
        let mut line = redact_line(raw, secrets);
        for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
            line = line.replace(secret, "***");
        }
        if content.len().saturating_add(line.len()).saturating_add(1) > MAX_LOG_EXPORT_BYTES {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Prepared log export exceeds its byte limit",
                false,
            ));
        }
        content.push_str(&line);
        content.push('\n');
    }
    let artifact = LogExportArtifact {
        summary: LogExportSummary {
            identity: LogExportIdentity {
                session,
                sequence,
                sha256: hash_document_bytes(&content),
            },
            records: records.len(),
            bytes: content.len(),
            first_record: records.first().map(|(id, _)| *id),
            last_record: records.last().map(|(id, _)| *id),
        },
        content,
    };
    artifact.summary.validate()?;
    Ok(artifact)
}

#[cfg(test)]
#[path = "log_export_tests.rs"]
mod tests;
