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
//! * [`CertificateAuthorityApplication::write_ca_path`] projects a file-path
//!   anchor into the global `tls.custom-certifactes` list — the carrier the
//!   pinned mihomo v1.19.18 actually reads (`config.RawTLS.CustomTrustCert`) —
//!   while preserving every other `tls:` key and profile section.
//! * [`CertificateAuthorityApplication::trust_anchors_from_profile`] reads that
//!   list back so a profile round trip keeps the anchor.
//!
//! The application layer stays executor-neutral: no Tokio, no filesystem
//! access of its own; reading is the port's job.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::protocol_trust::{
    CaLoadStatus, CaTrustReport, CaTrustResolution, TlsTrustParams,
};
use infiltrator_domain::tls_trust;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use serde_yaml_ng::{Mapping, Value};

/// Outcome of writing trust anchors into a profile document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustCommit {
    pub profile_yaml: String,
    /// The `tls.custom-certifactes` list as written.
    pub ca_paths: Vec<String>,
    /// `true` when the write changed the document.
    pub changed: bool,
}

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
                let status = if matches!(
                    error,
                    infiltrator_ports::error::PortError::Unsupported { .. }
                ) {
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

    /// Read the profile's global trust anchors back into the typed params.
    ///
    /// The pinned v1.19.18 carrier is a list of **paths**, so only the first
    /// entry can be modelled by the single `ca_path` slot; the rest stay in the
    /// profile untouched and the caller keeps them losslessly.
    pub fn trust_anchors_from_profile(profile_yaml: &str) -> Result<TlsTrustParams, Failure> {
        let document: Value = serde_yaml_ng::from_str(profile_yaml).map_err(|error| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("profile is not valid YAML: {error}"),
                false,
            )
        })?;
        let Some(list) = document
            .get("tls")
            .and_then(|tls| tls.get("custom-certifactes"))
            .and_then(Value::as_sequence)
        else {
            return Ok(TlsTrustParams::default());
        };
        let ca_path = list
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .find(|entry| !entry.is_empty())
            .unwrap_or_default()
            .to_string();
        Ok(TlsTrustParams {
            ca_path,
            ..TlsTrustParams::default()
        })
    }

    /// Project a file-path anchor into the global `tls.custom-certifactes`
    /// list. Returns the path that was newly inserted, or `None` when the list
    /// already carried it (nothing changed).
    ///
    /// Only the `ca`-style path is written here: the pinned core resolves the
    /// list as file paths, so an inline bundle must never be smuggled into it.
    pub fn write_ca_path(document: &mut Value, ca_path: &str) -> Option<String> {
        let ca_path = ca_path.trim();
        if ca_path.is_empty() {
            return None;
        }
        let mapping = document.as_mapping_mut()?;
        let tls_key = Value::String("tls".to_string());
        let tls_entry = mapping
            .entry(tls_key)
            .or_insert_with(|| Value::Mapping(Mapping::new()));
        let tls_mapping = tls_entry.as_mapping_mut()?;
        let list_key = Value::String("custom-certifactes".to_string());
        let list_entry = tls_mapping
            .entry(list_key)
            .or_insert_with(|| Value::Sequence(Vec::new()));
        let Value::Sequence(entries) = list_entry else {
            return None;
        };
        if entries.iter().any(|entry| entry.as_str() == Some(ca_path)) {
            return None;
        }
        let mut next = Vec::with_capacity(entries.len() + 1);
        next.push(Value::String(ca_path.to_string()));
        next.extend(entries.iter().cloned());
        *entries = next;
        Some(ca_path.to_string())
    }

    /// Reverse of [`Self::write_ca_path`], used by the honest audit so
    /// `structure_preserved` only ignores the entry this application owns.
    pub fn remove_ca_path(document: &mut Value, ca_path: &str) {
        let Some(mapping) = document.as_mapping_mut() else {
            return;
        };
        let Some(tls) = mapping
            .get_mut(Value::String("tls".to_string()))
            .and_then(Value::as_mapping_mut)
        else {
            return;
        };
        let list_key = Value::String("custom-certifactes".to_string());
        let Some(entries) = tls.get_mut(&list_key).and_then(Value::as_sequence_mut) else {
            return;
        };
        entries.retain(|entry| entry.as_str() != Some(ca_path));
        if entries.is_empty() {
            tls.remove(&list_key);
        }
        if tls.is_empty() {
            mapping.remove(Value::String("tls".to_string()));
        }
    }

    /// Write the trust anchors into a profile document. Everything else is
    /// preserved; the document is only rewritten when the anchor changed it.
    pub fn upsert_trust_anchors_into_profile(
        profile_yaml: &str,
        params: &TlsTrustParams,
    ) -> Result<TrustCommit, Failure> {
        let mut document: Value = serde_yaml_ng::from_str(profile_yaml).map_err(|error| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("profile is not valid YAML: {error}"),
                false,
            )
        })?;
        if !document.is_mapping() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "profile YAML must be a top-level mapping".to_string(),
                false,
            ));
        }
        let changed = Self::write_ca_path(&mut document, &params.ca_path).is_some();
        let profile_yaml = if changed {
            serde_yaml_ng::to_string(&document).map_err(|error| {
                Failure::new(
                    ErrorCode::Internal,
                    format!("could not serialize profile: {error}"),
                    false,
                )
            })?
        } else {
            profile_yaml.to_string()
        };
        let ca_paths = document
            .get("tls")
            .and_then(|tls| tls.get("custom-certifactes"))
            .and_then(Value::as_sequence)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Ok(TrustCommit {
            profile_yaml,
            ca_paths,
            changed,
        })
    }
}

#[cfg(test)]
#[path = "certificate_authority_application_test.rs"]
mod certificate_authority_application_test;
