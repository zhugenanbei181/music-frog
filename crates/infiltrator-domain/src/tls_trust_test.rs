//! DUAL-05-13 domain tests: PEM bundle validation + SHA-256 fingerprint.

use super::*;

/// A real, tiny self-signed certificate (PEM). Its content is irrelevant to
/// structure validation; the point is that a real bundle validates.
const TEST_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIIBhTCCASugAwIBAgIQIRi6zePL6mKjOipn+dNuaTAKBggqhkjOPQQDAjASMRAw
DgYDVQQKEwdBY21lIENvMB4XDTE3MTAyMDE5NDMwNloXDTE4MTAyMDE5NDMwNlow
EjEQMA4GA1UEChMHQWNtZSBDbzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABD0d
7VNhbWvZLWPuj/RtHFjvtJBEwOkhbN/BnnE8rnZR8+sbwnc/KhCk3FhnpHZnQz7B
5aETbbIgmuvewdjvSBSjYzBhMA4GA1UdDwEB/wQEAwICpDATBgNVHSUEDDAKBggr
BgEFBQcDATAPBgNVHRMBAf8EBTADAQH/MCkGA1UdEQQiMCCCDmxvY2FsaG9zdDo1
NDUzgg4xMjcuMC4wLjE6NTQ1MzAKBggqhkjOPQQDAgNIADBFAiEA2zpJEPQyz6/l
Wf86aX6PepsntZv2GYlA5UpabfT2EZICICpJ5h/iI+i341gBmLiAFQOyTDT+/wQc
6MF9+Yw1Yy0t
-----END CERTIFICATE-----
";

#[test]
fn a_real_bundle_validates_and_gets_a_stable_fingerprint() {
    assert_eq!(count_certificates(TEST_PEM), 1);
    let count = validate_pem_bundle(TEST_PEM).expect("valid bundle");
    assert_eq!(count, 1);
    let first = sha256_fingerprint(TEST_PEM);
    let second = sha256_fingerprint(&normalize_pem(TEST_PEM));
    assert_eq!(first, second, "normalisation must not change identity");
    assert_eq!(first.len(), 64);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(is_sha256_fingerprint(&first));
}

#[test]
fn an_invalid_bundle_is_rejected_with_a_real_reason() {
    assert!(validate_pem_bundle("").is_err());
    assert!(validate_pem_bundle("not a pem file at all").is_err());
    let broken_base64 = format!("{PEM_BEGIN}\n!!!not base64!!!\n{PEM_END}\n");
    let error = validate_pem_bundle(&broken_base64).expect_err("base64 must be validated");
    assert!(error.contains("base64"), "{error}");
    let missing_end = format!("{PEM_BEGIN}\nMIIB\n");
    let error = validate_pem_bundle(&missing_end).expect_err("armour must pair");
    assert!(error.contains("matching"), "{error}");
}

#[test]
fn fingerprints_accept_openssl_spelling_and_reject_noise() {
    let hex = "AABBCCDD".repeat(8);
    assert!(is_sha256_fingerprint(&hex));
    assert!(is_sha256_fingerprint(&format!("sha256:{hex}")));
    let colons = hex
        .as_bytes()
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).expect("ascii"))
        .collect::<Vec<_>>()
        .join(":");
    assert!(is_sha256_fingerprint(&colons));
    assert_eq!(normalize_fingerprint(&colons), hex.to_ascii_lowercase());
    assert!(!is_sha256_fingerprint("not-a-fingerprint"));
    assert!(!is_sha256_fingerprint(&"aa".repeat(63)));
    assert_eq!(normalize_fingerprint("zzzz"), "");
}

#[test]
fn multi_certificate_bundles_count_every_block() {
    let bundle = format!("{TEST_PEM}{TEST_PEM}");
    assert_eq!(validate_pem_bundle(&bundle).expect("two blocks"), 2);
}
