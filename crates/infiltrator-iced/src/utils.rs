//! `utils` — presentation helpers: byte formatting and redacted UI text.

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Structural redaction for user-visible text (CORE-001): toasts and the
/// error banner render raw error chains that can embed subscription query
/// tokens, `Authorization` headers or userinfo passwords, so everything
/// bound for the screen passes through [`infiltrator_core::redact::redact_line`]
/// first. Preserves plain text byte-for-byte and is idempotent.
pub fn sanitize_ui_text(text: &str) -> String {
    infiltrator_core::redact::redact_line(text, &[])
}

#[cfg(test)]
#[path = "../tests/gui/utils_tests.rs"]
mod tests;
