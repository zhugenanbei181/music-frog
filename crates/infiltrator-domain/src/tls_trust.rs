//! DUAL-05-13: domain facts for custom CA bundles.
//!
//! The typed vocabulary lives in `infiltrator-contract::protocol_trust`; this
//! module owns what can be *computed* from the text itself without lying about
//! the host: PEM structure validation and the SHA-256 fingerprint of a decoded
//! bundle. Reading a certificate from a file is a host capability and lives in
//! `infiltrator-ports::certificate_authority`, never here.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

/// PEM armour of an X.509 certificate.
pub const PEM_BEGIN: &str = "-----BEGIN CERTIFICATE-----";
pub const PEM_END: &str = "-----END CERTIFICATE-----";

/// Canonicalise a PEM bundle: LF line endings, no leading/trailing blank
/// noise, exactly one trailing newline. The fingerprint is taken over this
/// form so re-indenting a file does not change the identity of the bundle.
pub fn normalize_pem(text: &str) -> String {
    let mut normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized = normalized.trim().to_string();
    if normalized.is_empty() {
        return normalized;
    }
    format!("{normalized}\n")
}

/// Number of `CERTIFICATE` blocks in the bundle.
pub fn count_certificates(pem: &str) -> usize {
    pem.matches(PEM_BEGIN).count()
}

/// `true` when the text looks like a PEM certificate bundle at all.
pub fn looks_like_pem(text: &str) -> bool {
    text.contains(PEM_BEGIN)
}

/// Validate a PEM bundle and return the decoded certificate count.
///
/// Only structure is checked (armour pairs + standard base64 body of each
/// block). Certificate *semantics* are the core's business; this build does not
/// parse X.509 and must not pretend it did.
pub fn validate_pem_bundle(pem: &str) -> Result<usize, String> {
    let text = normalize_pem(pem);
    if text.is_empty() {
        return Err("CA bundle is empty".to_string());
    }
    if !looks_like_pem(&text) {
        return Err(format!("missing `{PEM_BEGIN}` PEM armour"));
    }
    let mut count = 0usize;
    let mut remainder = text.as_str();
    while let Some(begin) = remainder.find(PEM_BEGIN) {
        let after_begin = &remainder[begin + PEM_BEGIN.len()..];
        let Some(end) = after_begin.find(PEM_END) else {
            return Err(format!("`{PEM_BEGIN}` without a matching `{PEM_END}`"));
        };
        let body: String = after_begin[..end]
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("");
        if body.is_empty() {
            return Err("certificate block carries no base64 body".to_string());
        }
        if !body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        {
            return Err("certificate body is not standard base64".to_string());
        }
        if STANDARD.decode(&body).is_err() {
            return Err("certificate body is not decodable base64".to_string());
        }
        count += 1;
        remainder = &after_begin[end + PEM_END.len()..];
    }
    if count == 0 {
        return Err("CA bundle carries no certificate block".to_string());
    }
    Ok(count)
}

/// Lowercase hex SHA-256 of the normalised bundle text.
pub fn sha256_fingerprint(pem: &str) -> String {
    let digest = Sha256::digest(normalize_pem(pem).as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Canonicalise a fingerprint for comparison: lowercase, no separators, no
/// `sha256:` prefix. Returns an empty string when the text carries anything
/// that is not a hex digit, so a comparison can never match by accident.
pub fn normalize_fingerprint(text: &str) -> String {
    let trimmed = text.trim();
    let trimmed = trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("SHA256:"))
        .unwrap_or(trimmed);
    let compact: String = trimmed
        .chars()
        .filter(|c| !matches!(c, ':' | ' ' | '\t'))
        .collect();
    if compact.is_empty() || !compact.chars().all(|c| c.is_ascii_hexdigit()) {
        return String::new();
    }
    compact.to_ascii_lowercase()
}

/// `true` when the text is a well-formed SHA-256 fingerprint (64 hex digits,
/// optionally `:`-separated or `sha256:`-prefixed).
pub fn is_sha256_fingerprint(text: &str) -> bool {
    normalize_fingerprint(text).len() == 64
}

#[cfg(test)]
#[path = "tls_trust_test.rs"]
mod tests;
