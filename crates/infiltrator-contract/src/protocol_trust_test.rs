//! DUAL-05-13 contract tests: the typed certificate-trust vocabulary.

use super::*;

fn valid_fingerprint() -> String {
    "AB".repeat(32)
}

#[test]
fn params_validate_structure_and_pin_spelling() {
    let mut params = TlsTrustParams::default();
    assert!(!params.is_present());
    assert!(params.chips().is_empty());
    assert_eq!(params.summary_zh(), "未配置自定义证书信任");

    params.ca_path = "/etc/ssl/ca.pem".to_string();
    params.ca_str = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----".to_string();
    params.fingerprint = format!("sha256:{}", valid_fingerprint());
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");

    let chips = params.chips();
    assert!(chips.contains(&"ca:path".to_string()));
    assert!(chips.contains(&"ca:inline".to_string()));
    assert!(chips.iter().any(|chip| chip.starts_with("pin:ab")));
    assert!(params.has_bundle());
    assert!(params.inline_has_no_core_carrier());
}

#[test]
fn invalid_values_are_refused_with_real_reasons() {
    let mut params = TlsTrustParams {
        ca_path: "-----BEGIN CERTIFICATE-----\nnot a path".to_string(),
        ca_str: "not-a-pem".to_string(),
        fingerprint: "abc123".to_string(),
    };
    let mut issues = Vec::new();
    params.validate(&mut issues);
    let fields: Vec<&str> = issues.iter().map(|issue| issue.field.as_str()).collect();
    assert!(fields.contains(&"ca.path"), "{issues:?}");
    assert!(fields.contains(&"ca.str"), "{issues:?}");
    assert!(fields.contains(&"fingerprint"), "{issues:?}");

    // openssl's `:`-separated spelling is accepted.
    params.fingerprint = valid_fingerprint()
        .as_bytes()
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).expect("ascii"))
        .collect::<Vec<_>>()
        .join(":");
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(
        !issues.iter().any(|issue| issue.field == "fingerprint"),
        "{issues:?}"
    );
}

#[test]
fn carrier_notes_state_the_pinned_core_facts() {
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        ca_str: "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----".to_string(),
        fingerprint: String::new(),
    };
    let notes = params.notes();
    assert!(
        notes
            .iter()
            .any(|note| note.contains("tls.custom-certifactes"))
    );
    assert!(notes.iter().any(|note| note.contains("no carrier")));
}

#[test]
fn resolutions_and_the_report_never_claim_an_unloaded_ca() {
    let unsupported = CaTrustResolution::new(
        CaLoadStatus::Unsupported,
        "ca",
        "this host exposes no CA file reader",
    );
    assert!(!unsupported.status.is_loaded());
    assert_eq!(unsupported.chip(), "ca:unsupported");
    assert_eq!(unsupported.status.label_zh(), "宿主不支持");

    let report = CaTrustReport {
        resolutions: vec![unsupported],
        notes: vec!["path preserved in the draft".to_string()],
    };
    assert!(!report.is_loaded());
    assert!(report.is_unsupported());
    assert_eq!(report.summary_zh(), "ca：宿主不支持");
    assert!(report.lines()[0].contains("宿主不支持"));

    let loaded = CaTrustResolution::loaded("ca", valid_fingerprint(), 1);
    assert!(loaded.status.is_loaded());
    let report = CaTrustReport {
        resolutions: vec![loaded],
        notes: Vec::new(),
    };
    assert!(report.is_loaded());
    assert!(!report.is_unsupported());
    assert!(report.chips().iter().any(|chip| chip.starts_with("fp:ab")));

    let mismatch = CaTrustResolution {
        status: CaLoadStatus::FingerprintMismatch,
        fingerprint: Some(valid_fingerprint()),
        ..CaTrustResolution::new(CaLoadStatus::FingerprintMismatch, "ca", "pin mismatch")
    };
    // A mismatch is "loaded but refused": `is_loaded` reports the real load,
    // while `is_trusted` stays `false` so trust is never claimed.
    assert!(mismatch.status.is_loaded());
    assert!(
        !CaTrustReport {
            resolutions: vec![mismatch.clone()],
            notes: Vec::new(),
        }
        .is_trusted()
    );
    assert_eq!(mismatch.chip(), "ca:pin-mismatch");
}
