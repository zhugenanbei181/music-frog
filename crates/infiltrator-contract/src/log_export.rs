//! Source-bound prepared log exports and actual host write receipts.
use crate::error::{ErrorCode, Failure};
use crate::logs::LogSession;
use serde::{Deserialize, Serialize};

pub const MAX_LOG_EXPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogExportIdentity {
    pub session: LogSession,
    pub sequence: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogExportSummary {
    pub identity: LogExportIdentity,
    pub records: usize,
    pub bytes: usize,
    pub first_record: Option<u64>,
    pub last_record: Option<u64>,
}
impl LogExportSummary {
    pub fn validate(&self) -> Result<(), Failure> {
        let range = match (self.records, self.first_record, self.last_record) {
            (0, None, None) => self.bytes == 0,
            (records, Some(first), Some(last)) => {
                records > 0 && first > 0 && first <= last && self.bytes >= records
            }
            _ => false,
        };
        if !range
            || self.identity.sequence == 0
            || !self.identity.session.token.is_valid()
            || self.bytes > MAX_LOG_EXPORT_BYTES
            || self.identity.sha256.len() != 64
            || !self
                .identity
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Log export summary has no valid source or record range",
                false,
            ));
        }
        Ok(())
    }
}
/// Only the shared preparer constructs content; save commands carry identity only.
#[derive(Clone, PartialEq, Eq)]
pub struct LogExportArtifact {
    pub summary: LogExportSummary,
    pub content: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogExportReceipt {
    pub summary: LogExportSummary,
    pub path: String,
    pub bytes_written: usize,
}
impl LogExportReceipt {
    pub fn validate(&self, expected: &LogExportSummary) -> Result<(), Failure> {
        expected.validate()?;
        if self.summary != *expected
            || self.bytes_written != expected.bytes
            || self.path.trim().is_empty()
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Log export returned an incomplete or unrelated host receipt",
                false,
            ));
        }
        Ok(())
    }
}
