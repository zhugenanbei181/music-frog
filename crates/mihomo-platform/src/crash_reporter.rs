use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    pub timestamp_secs: u64,
    pub os_info: String,
    pub panic_reason: String,
    pub backtrace_summary: Option<String>,
    pub client_version: String,
}

pub struct CrashReporter;

impl CrashReporter {
    pub fn new_report(
        panic_reason: &str,
        client_version: &str,
        backtrace: Option<&str>,
    ) -> CrashReport {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let os_info = std::env::consts::OS.to_string();

        CrashReport {
            timestamp_secs,
            os_info,
            panic_reason: panic_reason.to_string(),
            backtrace_summary: backtrace.map(|s| s.to_string()),
            client_version: client_version.to_string(),
        }
    }

    pub fn sanitize_report(report: &mut CrashReport) {
        let mut text_to_sanitize = vec![&mut report.panic_reason];
        if let Some(bt) = &mut report.backtrace_summary {
            text_to_sanitize.push(bt);
        }

        let home_dir = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        // Match "Bearer " followed by token characters
        let token_re = Regex::new(r"(?i)(bearer\s+)[a-z0-9_\-]+").unwrap();
        let unix_home_re = Regex::new(r"/home/[^/]+").unwrap();
        let win_home_re = Regex::new(r"(?i)C:\\Users\\[^\\]+").unwrap();

        for text in text_to_sanitize {
            if let Some(ref home) = home_dir {
                *text = text.replace(home, "<REDACTED_HOME>");
            }
            
            *text = unix_home_re.replace_all(text, "<REDACTED_HOME>").to_string();
            *text = win_home_re.replace_all(text, "<REDACTED_HOME>").to_string();
            *text = token_re.replace_all(text, "${1}<REDACTED_TOKEN>").to_string();
        }
    }

    pub fn serialize_report(report: &CrashReport) -> anyhow::Result<String> {
        let json = serde_json::to_string(report)?;
        Ok(json)
    }

    pub fn parse_report(json_str: &str) -> anyhow::Result<CrashReport> {
        let report = serde_json::from_str(json_str)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_report() {
        let report = CrashReporter::new_report(
            "Test panic",
            "v1.0.0",
            Some("Stack trace line 1\nStack trace line 2"),
        );
        assert!(report.timestamp_secs > 0);
        assert_eq!(report.os_info, std::env::consts::OS);
        assert_eq!(report.panic_reason, "Test panic");
        assert_eq!(report.client_version, "v1.0.0");
        assert_eq!(
            report.backtrace_summary.unwrap(),
            "Stack trace line 1\nStack trace line 2"
        );
    }

    #[test]
    fn test_sanitize_report() {
        let mut report = CrashReporter::new_report(
            "Failed with token Bearer abcdef123456 and bearer xyz_789.",
            "v1.0.0",
            None,
        );
        CrashReporter::sanitize_report(&mut report);
        assert_eq!(
            report.panic_reason,
            "Failed with token Bearer <REDACTED_TOKEN> and bearer <REDACTED_TOKEN>."
        );
    }

    #[test]
    fn test_sanitize_paths() {
        let mut report = CrashReporter::new_report(
            "File not found: /home/username/.config/mihomo and C:\\Users\\Admin\\AppData",
            "v1.0.0",
            None,
        );
        CrashReporter::sanitize_report(&mut report);
        assert_eq!(
            report.panic_reason,
            "File not found: <REDACTED_HOME>/.config/mihomo and <REDACTED_HOME>\\AppData"
        );
    }

    #[test]
    fn test_serialization() {
        let report = CrashReporter::new_report("Test panic", "v1.0.0", None);
        let serialized = CrashReporter::serialize_report(&report).unwrap();
        let deserialized = CrashReporter::parse_report(&serialized).unwrap();
        assert_eq!(report, deserialized);
    }
}
