use crate::*;

#[test]
fn backtrace_summary_keeps_innermost_head() {
    let lines: Vec<String> = (0..100).map(|i| format!("frame {i}")).collect();
    let summary = backtrace_summary(&lines.join("\n"));
    assert_eq!(summary.lines().count(), BACKTRACE_SUMMARY_LINES);
    assert!(summary.starts_with("frame 0\n"));
    assert!(!summary.contains("frame 99"));
}

#[test]
fn backtrace_summary_handles_short_and_empty_input() {
    assert_eq!(backtrace_summary(""), "");
    assert_eq!(backtrace_summary("only frame"), "only frame");
}

#[test]
fn sanitized_report_is_collectible_without_network_or_panic() {
    let home = std::env::var("HOME").unwrap_or_default();
    let message = format!("boom at {home}/secrets with Bearer abcTOKEN123");
    let mut report = CrashReporter::new_report(&message, env!("CARGO_PKG_VERSION"), None);
    CrashReporter::sanitize_report(&mut report);
    assert!(!report.panic_reason.contains("abcTOKEN123"));
    if !home.is_empty() {
        assert!(!report.panic_reason.contains(&home));
    }
    let json = CrashReporter::serialize_report(&report).unwrap();
    assert_eq!(CrashReporter::parse_report(&json).unwrap(), report);
}
