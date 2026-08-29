//! Digest verification for the kernel delivery chain (UP-001 / CORE-006).
//!
//! Every runtime-downloaded artifact must present a SHA-256 digest obtained
//! from a trusted channel (the GitHub release API asset `digest` field). A
//! missing, malformed, or mismatching digest is always an error: an
//! unverified binary never reaches the versions directory.

use mihomo_api::error::{MihomoError, Result};
use sha2::{Digest, Sha256};

/// Expected digest prefix used by the GitHub release API.
const SHA256_PREFIX: &str = "sha256:";

/// Lowercase hex encoding of the SHA-256 digest of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Fail-closed verification of `bytes` against `expected`.
///
/// `expected` is the raw digest string as reported by the release API
/// (`sha256:<64 hex>`). Verification fails when the digest is absent,
/// malformed, computed over a different prefix, or does not match. `label`
/// identifies the artifact in error messages (version or file name).
pub fn verify_bytes(bytes: &[u8], expected: Option<&str>, label: &str) -> Result<()> {
    let Some(expected) = expected else {
        return Err(MihomoError::Version(format!(
            "refusing to install {label}: no SHA-256 digest published for this artifact (fail-closed)"
        )));
    };

    let expected_hex = expected
        .strip_prefix(SHA256_PREFIX)
        .ok_or_else(|| {
            MihomoError::Version(format!(
                "refusing to install {label}: digest {expected:?} lacks the {SHA256_PREFIX:?} prefix"
            ))
        })?
        .trim()
        .to_ascii_lowercase();

    if expected_hex.len() != 64 || !expected_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(MihomoError::Version(format!(
            "refusing to install {label}: digest {expected:?} is not a well-formed SHA-256 hex string"
        )));
    }

    let actual = sha256_hex(bytes);
    if actual != expected_hex {
        return Err(MihomoError::Version(format!(
            "refusing to install {label}: SHA-256 mismatch (expected {expected_hex}, got {actual})"
        )));
    }

    log::info!("digest verified for {label} (sha256:{actual})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &[u8] = b"mihomo test payload";

    fn hello_digest() -> String {
        format!("sha256:{}", sha256_hex(HELLO))
    }

    #[test]
    fn matching_digest_passes() {
        assert!(verify_bytes(HELLO, Some(&hello_digest()), "test.gz").is_ok());
    }

    #[test]
    fn uppercase_hex_digest_passes() {
        let upper = format!("SHA256:{}", sha256_hex(HELLO).to_uppercase());
        // API form is lowercase `sha256:`; a malformed prefix must fail...
        assert!(verify_bytes(HELLO, Some(&upper), "test.gz").is_err());

        // ...but a well-formed digest with uppercase hex digits is accepted.
        let mixed = format!("sha256:{}", sha256_hex(HELLO).to_uppercase());
        assert!(verify_bytes(HELLO, Some(&mixed), "test.gz").is_ok());
    }

    #[test]
    fn tampered_bytes_fail() {
        let digest = hello_digest();
        let tampered = b"mihomo test payloaX";
        let err = verify_bytes(tampered, Some(&digest), "test.gz").unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"));
    }

    #[test]
    fn missing_digest_fails_closed() {
        let err = verify_bytes(HELLO, None, "test.gz").unwrap_err();
        assert!(err.to_string().contains("fail-closed"));
    }

    #[test]
    fn malformed_digest_fails() {
        for bad in ["", "sha256:", "sha256:abc", "deadbeef", "md5:deadbeef"] {
            let err = verify_bytes(HELLO, Some(bad), "test.gz").unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("prefix") || msg.contains("well-formed"),
                "digest {bad:?} should be rejected as malformed, got: {msg}"
            );
        }
    }

    #[test]
    fn sha256_hex_is_stable_and_lowercase() {
        let hex = sha256_hex(b"");
        assert_eq!(
            hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(hex, hex.to_lowercase());
    }
}
