//! DUAL-05-13: custom CA / certificate-whitelist application.
//!
//! One shared flow both surfaces and the host command path use:
//!
//! * [`CertificateAuthorityApplication::resolve`] turns a draft's
//!   [`TlsTrustParams`] into a typed [`CaTrustReport`] by *asking the host*
//!   (through [`CertificateAuthorityPort`]) to read a path, and by validating
//!   inline PEM locally. A host without a reader is reported as
//!   [`CaLoadStatus::Unsupported`]; the report never claims a CA was loaded
//!   when nothing was read.
//! * profile-level trust-anchor inspection and mutation use the selected
//!   engine's typed profile codec; this application never parses profile YAML.
//!
//! The application layer stays executor-neutral: no Tokio, no filesystem
//! access of its own; reading is the port's job.

use crate::profile_application::ProfileApplication;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::protocol_trust::{
    CaLoadStatus, CaTrustReport, CaTrustResolution, TlsTrustParams,
};
use infiltrator_domain::tls_trust;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationQuery, ProfileConfigurationReadout,
    ProfileConfigurationUpdate,
};
use lurker_core_contract::error::{ErrorCode, Failure};

/// DUAL-05-13 shared CA use cases.
pub struct CertificateAuthorityApplication;

impl CertificateAuthorityApplication {
    /// Resolve the draft's trust request against the host reader.
    ///
    /// The returned report states exactly what happened:
    /// [`CaLoadStatus::Unsupported`] when the host has no reader,
    /// [`CaLoadStatus::ReadFailed`] when the read failed,
    /// [`CaLoadStatus::InvalidBundle`] when the content is not a PEM bundle,
    /// [`CaLoadStatus::FingerprintMismatch`] when the whitelist pin does not
    /// match, and [`CaLoadStatus::Loaded`] only after a real read + validation.
    pub fn resolve(
        params: &TlsTrustParams,
        port: Option<&dyn CertificateAuthorityPort>,
    ) -> CaTrustReport {
        let mut resolutions = Vec::new();
        let mut notes = params.notes();

        let ca_path = params.ca_path.trim();
        if !ca_path.is_empty() {
            resolutions.push(Self::resolve_path(ca_path, params, port));
        }
        let ca_str = params.ca_str.trim();
        if !ca_str.is_empty() {
            resolutions.push(Self::resolve_inline(ca_str, &params.fingerprint));
        }
        if ca_path.is_empty() && ca_str.is_empty() && !params.fingerprint.trim().is_empty() {
            resolutions.push(CaTrustResolution::new(
                CaLoadStatus::NotRequested,
                "fingerprint",
                "the pinned core enforces `fingerprint` itself; no CA bundle was loaded here",
            ));
        }
        if resolutions.is_empty() {
            resolutions.push(CaTrustResolution::new(
                CaLoadStatus::NotRequested,
                "ca",
                "the draft asks for no custom CA or certificate pin",
            ));
        }
        if params.inline_has_no_core_carrier() {
            notes.push(
                "the inline bundle was validated in-process only; the pinned mihomo v1.19.18 has no `ca-str` carrier, so it is not part of a core config"
                    .to_string(),
            );
        }
        CaTrustReport { resolutions, notes }
    }

    fn resolve_path(
        path: &str,
        params: &TlsTrustParams,
        port: Option<&dyn CertificateAuthorityPort>,
    ) -> CaTrustResolution {
        let Some(port) = port else {
            return CaTrustResolution::new(
                CaLoadStatus::Unsupported,
                "ca",
                format!(
                    "this host exposes no CA file reader, so `{path}` was not loaded; the path is preserved in the draft"
                ),
            );
        };
        let file = match port.read_ca_file(path) {
            Ok(file) => file,
            Err(error) => {
                // A host without a reader reports a typed *unsupported* state,
                // not a read failure: the two must never be conflated.
                let status = if matches!(error, PortError::Unsupported { .. }) {
                    CaLoadStatus::Unsupported
                } else {
                    CaLoadStatus::ReadFailed
                };
                return CaTrustResolution::new(
                    status,
                    "ca",
                    format!("could not read `{path}`: {error}"),
                );
            }
        };
        Self::validate_loaded(&file.pem, "ca", &file.path, &params.fingerprint)
    }

    fn resolve_inline(pem: &str, expected_fingerprint: &str) -> CaTrustResolution {
        Self::validate_loaded(pem, "ca-str", "inline PEM", expected_fingerprint)
    }

    fn validate_loaded(
        pem: &str,
        source: &str,
        label: &str,
        expected_fingerprint: &str,
    ) -> CaTrustResolution {
        let certificate_count = match tls_trust::validate_pem_bundle(pem) {
            Ok(count) => count,
            Err(error) => {
                return CaTrustResolution::new(
                    CaLoadStatus::InvalidBundle,
                    source,
                    format!("`{label}` is not a valid PEM bundle: {error}"),
                );
            }
        };
        let fingerprint = tls_trust::sha256_fingerprint(pem);
        let expected = tls_trust::normalize_fingerprint(expected_fingerprint);
        if !expected.is_empty() && expected != fingerprint {
            let mut resolution = CaTrustResolution::new(
                CaLoadStatus::FingerprintMismatch,
                source,
                format!(
                    "`{label}` loaded but its SHA-256 `{fingerprint}` does not match the whitelist pin"
                ),
            );
            resolution.fingerprint = Some(fingerprint);
            resolution.certificate_count = Some(certificate_count);
            return resolution;
        }
        let mut resolution = CaTrustResolution::loaded(source, fingerprint, certificate_count);
        resolution.detail =
            format!("`{label}` read and validated ({certificate_count} certificate(s))");
        resolution
    }

    /// Read the profile-level path carrier through the engine-selected codec.
    /// The adapter returns only the first path represented by `ca_path`; any
    /// additional paths remain opaque in the original document bytes.
    pub fn trust_anchors_from_profile(
        profiles: &ProfileApplication,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
    ) -> Result<TlsTrustParams, Failure> {
        match profiles.read_profile_configuration(
            identity,
            raw_bytes,
            ProfileConfigurationQuery::TrustAnchors,
        )? {
            ProfileConfigurationReadout::TrustAnchors(params) => Ok(params),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "profile configuration codec returned the wrong CA trust-anchor readback",
                false,
            )),
        }
    }

    /// Build a source-free CA trust-anchor candidate through the selected
    /// engine codec. The caller commits its exact returned bytes with the
    /// workspace transaction.
    pub fn apply_profile_trust_anchors(
        profiles: &ProfileApplication,
        identity: &ProfileDocumentIdentity,
        raw_bytes: &[u8],
        params: TlsTrustParams,
    ) -> Result<ProfileConfigurationMutation, Failure> {
        profiles.apply_profile_configuration(
            identity,
            raw_bytes,
            ProfileConfigurationUpdate::TrustAnchors(params),
        )
    }
}

#[cfg(test)]
#[path = "certificate_authority_application_test.rs"]
mod certificate_authority_application_test;
