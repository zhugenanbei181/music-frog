//! DUAL-05-13 application tests: the shared CA flow never claims a load the
//! host did not perform, and the profile carrier is written losslessly.

use super::*;
use infiltrator_ports::error::PortError;

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

/// A reader that returns the requested bundle (deterministic test double).
struct StaticCaReader {
    pem: String,
}

impl CertificateAuthorityPort for StaticCaReader {
    fn read_ca_file(
        &self,
        path: &str,
    ) -> Result<infiltrator_ports::certificate_authority::CaFile, PortError> {
        Ok(infiltrator_ports::certificate_authority::CaFile {
            path: path.to_string(),
            pem: self.pem.clone(),
        })
    }
}

/// A reader whose host reports an I/O failure.
struct FailingCaReader;

impl CertificateAuthorityPort for FailingCaReader {
    fn read_ca_file(
        &self,
        path: &str,
    ) -> Result<infiltrator_ports::certificate_authority::CaFile, PortError> {
        Err(PortError::Io(format!("`{path}`: permission denied")))
    }
}

#[test]
fn a_host_without_a_reader_reports_typed_unsupported_instead_of_a_load() {
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&params, None);
    assert_eq!(report.status(), CaLoadStatus::Unsupported);
    assert!(report.is_unsupported());
    assert!(!report.is_loaded());
    assert!(report.chips().contains(&"ca:unsupported".to_string()));
    let line = report.lines().join(" ");
    assert!(line.contains("宿主不支持"), "{line}");
    assert!(line.contains("was not loaded"), "{line}");
}

#[test]
fn a_real_reader_loads_the_bundle_and_computes_the_fingerprint() {
    let reader = StaticCaReader {
        pem: TEST_PEM.to_string(),
    };
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&params, Some(&reader));
    assert_eq!(report.status(), CaLoadStatus::Loaded);
    assert!(report.is_loaded());
    assert!(!report.is_unsupported());
    let resolution = &report.resolutions[0];
    assert_eq!(resolution.certificate_count, Some(1));
    let fingerprint = resolution.fingerprint.clone().expect("fingerprint");
    assert_eq!(
        fingerprint,
        infiltrator_domain::tls_trust::sha256_fingerprint(TEST_PEM)
    );

    // A matching whitelist pin stays loaded...
    let matching = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        fingerprint: fingerprint.clone(),
        ..TlsTrustParams::default()
    };
    assert_eq!(
        CertificateAuthorityApplication::resolve(&matching, Some(&reader)).status(),
        CaLoadStatus::Loaded
    );
    // ...and a mismatching pin is refused with the real fingerprint attached.
    let mismatching = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        fingerprint: "00".repeat(32),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&mismatching, Some(&reader));
    assert_eq!(report.status(), CaLoadStatus::FingerprintMismatch);
    assert_eq!(report.chips()[0], "ca:pin-mismatch");
}

#[test]
fn a_failing_reader_is_a_read_failure_not_an_unsupported_host() {
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&params, Some(&FailingCaReader));
    assert_eq!(report.status(), CaLoadStatus::ReadFailed);
    assert!(!report.is_loaded());
    assert!(!report.is_unsupported());
    assert!(report.lines()[0].contains("permission denied"));
}

#[test]
fn an_invalid_inline_bundle_is_never_left_untyped() {
    let params = TlsTrustParams {
        ca_str: "definitely not pem".to_string(),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&params, None);
    assert_eq!(report.status(), CaLoadStatus::InvalidBundle);
    assert!(!report.is_loaded());
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("no `ca-str` carrier")),
        "{:?}",
        report.notes
    );
}

#[test]
fn the_path_anchor_lands_in_the_real_v11918_carrier_and_preserves_the_rest() {
    let profile = r#"
mode: rule
tls:
  certificate: /etc/ssl/cert.pem
  private-key: /etc/ssl/key.pem
  custom-certifactes:
    - /etc/ssl/existing.pem
proxies: []
rules:
  - MATCH,DIRECT
"#;
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/new-ca.pem".to_string(),
        ..TlsTrustParams::default()
    };
    let commit =
        CertificateAuthorityApplication::upsert_trust_anchors_into_profile(profile, &params)
            .expect("trust commit");
    assert!(commit.changed);
    assert_eq!(
        commit.ca_paths,
        vec![
            "/etc/ssl/new-ca.pem".to_string(),
            "/etc/ssl/existing.pem".to_string()
        ]
    );
    assert!(
        commit
            .profile_yaml
            .contains("certificate: /etc/ssl/cert.pem")
    );
    assert!(
        commit
            .profile_yaml
            .contains("private-key: /etc/ssl/key.pem")
    );
    assert!(commit.profile_yaml.contains("MATCH,DIRECT"));

    // Read back: the first path is the modelled anchor; a second write is a
    // no-op (the carrier already lists it).
    let read_back =
        CertificateAuthorityApplication::trust_anchors_from_profile(&commit.profile_yaml)
            .expect("read back");
    assert_eq!(read_back.ca_path, "/etc/ssl/new-ca.pem");
    let again = CertificateAuthorityApplication::upsert_trust_anchors_into_profile(
        &commit.profile_yaml,
        &params,
    )
    .expect("second commit");
    assert!(!again.changed);
    assert_eq!(again.profile_yaml, commit.profile_yaml);

    // A profile without the section stays untouched when no anchor is set.
    let untouched = CertificateAuthorityApplication::upsert_trust_anchors_into_profile(
        profile,
        &TlsTrustParams::default(),
    )
    .expect("no-op");
    assert!(!untouched.changed);
    assert_eq!(untouched.profile_yaml, profile);
}

#[test]
fn trust_is_only_claimed_when_every_resolution_loaded() {
    let reader = StaticCaReader {
        pem: TEST_PEM.to_string(),
    };
    let matching = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        fingerprint: infiltrator_domain::tls_trust::sha256_fingerprint(TEST_PEM),
        ..TlsTrustParams::default()
    };
    assert!(CertificateAuthorityApplication::resolve(&matching, Some(&reader)).is_trusted());

    let mismatching = TlsTrustParams {
        ca_path: "/etc/ssl/ca.pem".to_string(),
        fingerprint: "00".repeat(32),
        ..TlsTrustParams::default()
    };
    let report = CertificateAuthorityApplication::resolve(&mismatching, Some(&reader));
    assert!(report.is_loaded());
    assert!(!report.is_trusted(), "a pin mismatch must never be trusted");

    let unsupported = CertificateAuthorityApplication::resolve(&matching, None);
    assert!(!unsupported.is_trusted());
}
