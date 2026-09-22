//! DUAL-05-13: typed custom-CA / certificate-whitelist vocabulary.
//!
//! Three carriers exist in the mihomo family, and this build is explicit about
//! which one the pinned v1.19.18 core actually reads:
//!
//! * `ca:` — a path to a PEM bundle. v1.19.18 has **no per-node** `ca` key
//!   (its `proxy:` tag table has no such entry); the supported carrier is the
//!   *global* `tls.custom-certifactes:` list of paths
//!   (`config.RawTLS.CustomTrustCert`), so a path is projected there.
//! * `ca-str:` — inline PEM. v1.19.18 has no carrier for an inline CA bundle
//!   at all (the global list holds paths, the node schema holds neither key),
//!   so an inline bundle stays a typed, validated, preserved draft value with
//!   an honest note and is never claimed as loaded by the core.
//! * `fingerprint:` — the certificate whitelist the core *does* enforce: the
//!   SHA-256 fingerprint of the peer certificate (upstream TLS config docs).
//!
//! Reading a path from disk is a host capability; [`CaTrustResolution`] records
//! what the host really reported, and a host that cannot read a CA file
//! degrades to [`CaLoadStatus::Unsupported`] instead of claiming the CA is
//! loaded.

use serde::{Deserialize, Serialize};

use crate::protocol_fidelity::ProtocolIssue;
use crate::protocol_params_ext::{is_valid_sha256_fingerprint, push};

/// DUAL-05-13: one draft's certificate-trust settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsTrustParams {
    /// `ca:` — path to a PEM bundle. Projected into the global
    /// `tls.custom-certifactes` list on save.
    pub ca_path: String,
    /// `ca-str:` — inline PEM bundle. Validated here, preserved in the draft,
    /// and honestly marked as having no v1.19.18 carrier.
    pub ca_str: String,
    /// `fingerprint:` — SHA-256 of the peer certificate (certificate
    /// whitelist); the pinned core enforces this key.
    pub fingerprint: String,
}

impl TlsTrustParams {
    pub fn is_present(&self) -> bool {
        !self.ca_path.trim().is_empty()
            || !self.ca_str.trim().is_empty()
            || !self.fingerprint.trim().is_empty()
    }

    /// `true` when this draft asks for a CA bundle (not just a pin).
    pub fn has_bundle(&self) -> bool {
        !self.ca_path.trim().is_empty() || !self.ca_str.trim().is_empty()
    }

    /// `true` when an inline bundle has no carrier in the pinned core.
    pub fn inline_has_no_core_carrier(&self) -> bool {
        !self.ca_str.trim().is_empty()
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        let ca_path = self.ca_path.trim();
        let ca_str = self.ca_str.trim();
        let fingerprint = self.fingerprint.trim();
        if !ca_path.is_empty() && ca_path.contains('\n') {
            issues.push(ProtocolIssue::new(
                "ca.path",
                "`ca` must be a single file path, not PEM content; use `ca-str` for inline PEM",
            ));
        }
        if !ca_str.is_empty() && !looks_like_pem(ca_str) {
            push(
                issues,
                "ca.str",
                "`ca-str` must be a PEM bundle (-----BEGIN CERTIFICATE----- ...)",
            );
        }
        if !fingerprint.is_empty() && !is_valid_sha256_fingerprint(fingerprint) {
            push(
                issues,
                "fingerprint",
                "`fingerprint` must be a SHA-256 cert fingerprint (64 hex digits; `:` separators and a `sha256:` prefix are accepted)",
            );
        }
    }

    /// Chips both surfaces render (05-13).
    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if !self.ca_path.trim().is_empty() {
            chips.push("ca:path".to_string());
        }
        if !self.ca_str.trim().is_empty() {
            chips.push("ca:inline".to_string());
        }
        let fingerprint = self.fingerprint.trim();
        if !fingerprint.is_empty() {
            let compact = fingerprint
                .strip_prefix("sha256:")
                .or_else(|| fingerprint.strip_prefix("SHA256:"))
                .unwrap_or(fingerprint)
                .replace(':', "")
                .to_ascii_lowercase();
            let short = compact.get(..8).unwrap_or(compact.as_str()).to_string();
            chips.push(format!("pin:{short}"));
        }
        chips
    }

    /// Non-blocking facts about the pinned core; never a fabricated claim.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if !self.ca_path.trim().is_empty() {
            notes.push(
                "`ca` is written to the global `tls.custom-certifactes` list (the v1.19.18 carrier); the pinned node schema has no per-node `ca` key".to_string(),
            );
        }
        if !self.ca_str.trim().is_empty() {
            notes.push(
                "`ca-str` has no carrier in the pinned mihomo v1.19.18 (the global list holds paths and the node schema has no `ca-str`); the inline bundle is preserved but the core will not read it".to_string(),
            );
        }
        if !self.fingerprint.trim().is_empty() && !self.has_bundle() {
            notes.push(
                "no CA bundle is configured: `fingerprint` alone pins the peer certificate and skips chain-of-trust building".to_string(),
            );
        }
        notes
    }

    pub fn summary_zh(&self) -> String {
        let mut parts = Vec::new();
        if !self.ca_path.trim().is_empty() {
            parts.push("自定义 CA 路径".to_string());
        }
        if !self.ca_str.trim().is_empty() {
            parts.push("内嵌 PEM（v1.19.18 无载体）".to_string());
        }
        if !self.fingerprint.trim().is_empty() {
            parts.push("证书指纹白名单".to_string());
        }
        if parts.is_empty() {
            "未配置自定义证书信任".to_string()
        } else {
            parts.join(" · ")
        }
    }
}

/// `true` when the text carries PEM certificate armour.
pub fn looks_like_pem(text: &str) -> bool {
    text.contains("-----BEGIN CERTIFICATE-----")
}

/// What the host actually did with one CA request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaLoadStatus {
    /// The draft asks for no certificate trust.
    #[default]
    NotRequested,
    /// The host read (or the caller supplied) a bundle and it validated.
    Loaded,
    /// This host exposes no CA file reader: typed unsupported, no claim made.
    Unsupported,
    /// The host has a reader but the read failed (missing file, permission,
    /// size limit).
    ReadFailed,
    /// Content was available but is not a valid PEM bundle.
    InvalidBundle,
    /// The bundle loaded, but its fingerprint does not match the whitelist pin.
    FingerprintMismatch,
}

impl CaLoadStatus {
    pub const fn is_loaded(self) -> bool {
        matches!(self, Self::Loaded | Self::FingerprintMismatch)
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::NotRequested => "未请求",
            Self::Loaded => "已加载校验",
            Self::Unsupported => "宿主不支持",
            Self::ReadFailed => "读取失败",
            Self::InvalidBundle => "PEM 无效",
            Self::FingerprintMismatch => "指纹不匹配",
        }
    }

    pub fn label_en(self) -> &'static str {
        match self {
            Self::NotRequested => "Not requested",
            Self::Loaded => "Loaded and validated",
            Self::Unsupported => "Unsupported by this host",
            Self::ReadFailed => "Read failed",
            Self::InvalidBundle => "Invalid PEM",
            Self::FingerprintMismatch => "Fingerprint mismatch",
        }
    }

    pub fn chip(self) -> &'static str {
        match self {
            Self::NotRequested => "ca:none",
            Self::Loaded => "ca:verified",
            Self::Unsupported => "ca:unsupported",
            Self::ReadFailed => "ca:read-failed",
            Self::InvalidBundle => "ca:invalid",
            Self::FingerprintMismatch => "ca:pin-mismatch",
        }
    }
}

/// One typed outcome for the CA request of one draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaTrustResolution {
    pub status: CaLoadStatus,
    /// Which carrier was resolved (`ca` / `ca-str` / `fingerprint`).
    pub source: String,
    /// SHA-256 of the loaded bundle, when one really was loaded.
    pub fingerprint: Option<String>,
    /// Number of certificates in the loaded bundle, when known.
    pub certificate_count: Option<usize>,
    pub detail: String,
}

impl CaTrustResolution {
    pub fn new(status: CaLoadStatus, source: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            source: source.into(),
            fingerprint: None,
            certificate_count: None,
            detail: detail.into(),
        }
    }

    pub fn loaded(
        source: impl Into<String>,
        fingerprint: impl Into<String>,
        certificate_count: usize,
    ) -> Self {
        let source = source.into();
        Self {
            status: CaLoadStatus::Loaded,
            detail: format!("bundle loaded from `{source}`"),
            source,
            fingerprint: Some(fingerprint.into()),
            certificate_count: Some(certificate_count),
        }
    }

    pub fn chip(&self) -> String {
        self.status.chip().to_string()
    }
}

/// DUAL-05-13 read model: what both surfaces display for the CA request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaTrustReport {
    #[serde(default)]
    pub resolutions: Vec<CaTrustResolution>,
    /// Honest notes (carrier facts, unsupported reasons) shown by both surfaces.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl CaTrustReport {
    pub fn status(&self) -> CaLoadStatus {
        self.resolutions
            .first()
            .map(|resolution| resolution.status)
            .unwrap_or_default()
    }

    /// `true` when the host really read (or the caller supplied inline) a
    /// bundle and it validated. An unsupported host never returns `true`, and a
    /// whitelist mismatch keeps [`Self::is_trusted`] `false` while still
    /// reporting the real load.
    pub fn is_loaded(&self) -> bool {
        self.resolutions.iter().any(|r| r.status.is_loaded())
    }

    /// `true` only when every resolution is [`CaLoadStatus::Loaded`]: no
    /// unsupported host, no read failure, no invalid bundle, no pin mismatch.
    pub fn is_trusted(&self) -> bool {
        !self.resolutions.is_empty()
            && self
                .resolutions
                .iter()
                .all(|r| r.status == CaLoadStatus::Loaded)
    }

    pub fn is_unsupported(&self) -> bool {
        self.resolutions
            .iter()
            .any(|r| r.status == CaLoadStatus::Unsupported)
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips: Vec<String> = self
            .resolutions
            .iter()
            .map(CaTrustResolution::chip)
            .collect();
        if let Some(resolution) = self
            .resolutions
            .iter()
            .find(|r| r.fingerprint.is_some())
            .and_then(|r| r.fingerprint.as_deref())
        {
            let compact = resolution.to_ascii_lowercase();
            let short = compact.get(..8).unwrap_or(compact.as_str());
            chips.push(format!("fp:{short}"));
        }
        chips
    }

    /// One line per resolution plus the notes, in render order.
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .resolutions
            .iter()
            .map(|resolution| {
                format!(
                    "[{}] {}: {}",
                    resolution.status.label_zh(),
                    resolution.source,
                    resolution.detail
                )
            })
            .collect();
        lines.extend(self.notes.iter().cloned());
        lines
    }

    pub fn summary_zh(&self) -> String {
        if self.resolutions.is_empty() {
            return "未配置自定义证书信任".to_string();
        }
        self.resolutions
            .iter()
            .map(|resolution| format!("{}：{}", resolution.source, resolution.status.label_zh()))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[cfg(test)]
#[path = "protocol_trust_test.rs"]
mod tests;
