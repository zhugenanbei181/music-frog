//! Desktop crash-report adapter.

use mihomo_platform::crash_reporter::CrashReporter;
use std::path::Path;

/// Best-effort local structured report writer. Platform serialization and
/// sanitization stay in the desktop host rather than leaking into the UI.
pub fn write_sanitized_report(
    log_dir: &Path,
    panic_message: &str,
    client_version: &str,
    backtrace_summary: &str,
) {
    let mut report = CrashReporter::new_report(
        panic_message,
        client_version,
        Some(backtrace_summary),
    );
    CrashReporter::sanitize_report(&mut report);
    if let Ok(json) = CrashReporter::serialize_report(&report) {
        let _ = std::fs::write(log_dir.join("infiltrator_crash_report.json"), json);
    }
}
