//! `utils` — presentation helpers: byte formatting and redacted UI text.

use infiltrator_domain::redact::redact_line;
/// Structural redaction for user-visible text (CORE-001): toasts and the
/// error banner render raw error chains that can embed subscription query
/// tokens, `Authorization` headers or userinfo passwords, so everything
/// bound for the screen passes through [`infiltrator_domain::redact::redact_line`]
/// first. Preserves plain text byte-for-byte and is idempotent.
pub fn sanitize_ui_text(text: &str) -> String {
    redact_line(text, &[])
}

#[cfg(test)]
#[path = "../tests/gui/utils_tests.rs"]
mod tests;
