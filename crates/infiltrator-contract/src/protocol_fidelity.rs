//! DUAL-05 protocol-ecosystem contract: node drafts, cipher families,
//! VLESS REALITY/Vision parameters, stream multiplexing, and the shared
//! nodes-codec audit vocabulary.
//!
//! Group 05 closes on the same four-layer standard as groups 06/12/14: one
//! shared contract + application source, both surfaces, dual headless tests,
//! and an honest per-item ledger. This module owns the *typed vocabulary*;
//! parsing, lossless document splicing and validation messages live in
//! `infiltrator-application::protocol_codec_application`, and the underlying
//! text codecs live in `infiltrator-domain::profile_converter`.
//!
//! The draft keeps the on-the-wire text (`cipher`, `flow`, `protocol`) as the
//! source of truth and derives the typed family on demand. That is deliberate:
//! a cipher this build does not know must still round-trip verbatim instead of
//! being silently rewritten into a "close enough" variant (DUAL-05-14
//! 未知字段无损流通).

use serde::{Deserialize, Serialize};

/// DUAL-05-01: Shadowsocks cipher family, including the whole 2022-blake3 set.
///
/// `parse` is strict about names and case because mihomo derives the key
/// length from them; an unrecognized cipher stays in [`ProtocolDraft::cipher`]
/// verbatim so the codec stays lossless. Serde names are the mihomo wire
/// strings, so a serialized draft never carries a private spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShadowsocksCipher {
    #[serde(rename = "aes-128-gcm")]
    Aes128Gcm,
    #[serde(rename = "aes-256-gcm")]
    Aes256Gcm,
    #[serde(rename = "chacha20-ietf-poly1305")]
    Chacha20IetfPoly1305,
    #[serde(rename = "xchacha20-ietf-poly1305")]
    Xchacha20IetfPoly1305,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "table")]
    Table,
    #[serde(rename = "rc4-md5")]
    Rc4Md5,
    #[serde(rename = "aes-128-cfb")]
    Aes128Cfb,
    #[serde(rename = "aes-192-cfb")]
    Aes192Cfb,
    #[serde(rename = "aes-256-cfb")]
    Aes256Cfb,
    #[serde(rename = "2022-blake3-aes-128-gcm")]
    Blake3Aes128Gcm,
    #[serde(rename = "2022-blake3-aes-256-gcm")]
    Blake3Aes256Gcm,
    #[serde(rename = "2022-blake3-chacha20-poly1305")]
    Blake3Chacha20Poly1305,
    #[serde(rename = "2022-blake3-chacha8-poly1305")]
    Blake3Chacha8Poly1305,
}

impl ShadowsocksCipher {
    /// Every 2022-blake3 cipher, in mihomo's documentation order.
    pub const ALL_2022: [Self; 4] = [
        Self::Blake3Aes128Gcm,
        Self::Blake3Aes256Gcm,
        Self::Blake3Chacha20Poly1305,
        Self::Blake3Chacha8Poly1305,
    ];

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "aes-128-gcm" => Some(Self::Aes128Gcm),
            "aes-256-gcm" => Some(Self::Aes256Gcm),
            "chacha20-ietf-poly1305" => Some(Self::Chacha20IetfPoly1305),
            "xchacha20-ietf-poly1305" => Some(Self::Xchacha20IetfPoly1305),
            "none" => Some(Self::None),
            "table" => Some(Self::Table),
            "rc4-md5" => Some(Self::Rc4Md5),
            "aes-128-cfb" => Some(Self::Aes128Cfb),
            "aes-192-cfb" => Some(Self::Aes192Cfb),
            "aes-256-cfb" => Some(Self::Aes256Cfb),
            "2022-blake3-aes-128-gcm" => Some(Self::Blake3Aes128Gcm),
            "2022-blake3-aes-256-gcm" => Some(Self::Blake3Aes256Gcm),
            "2022-blake3-chacha20-poly1305" => Some(Self::Blake3Chacha20Poly1305),
            "2022-blake3-chacha8-poly1305" => Some(Self::Blake3Chacha8Poly1305),
            _ => None,
        }
    }

    /// The exact `cipher:` string written back to the profile.
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Aes128Gcm => "aes-128-gcm",
            Self::Aes256Gcm => "aes-256-gcm",
            Self::Chacha20IetfPoly1305 => "chacha20-ietf-poly1305",
            Self::Xchacha20IetfPoly1305 => "xchacha20-ietf-poly1305",
            Self::None => "none",
            Self::Table => "table",
            Self::Rc4Md5 => "rc4-md5",
            Self::Aes128Cfb => "aes-128-cfb",
            Self::Aes192Cfb => "aes-192-cfb",
            Self::Aes256Cfb => "aes-256-cfb",
            Self::Blake3Aes128Gcm => "2022-blake3-aes-128-gcm",
            Self::Blake3Aes256Gcm => "2022-blake3-aes-256-gcm",
            Self::Blake3Chacha20Poly1305 => "2022-blake3-chacha20-poly1305",
            Self::Blake3Chacha8Poly1305 => "2022-blake3-chacha8-poly1305",
        }
    }

    /// `true` for the AEAD-2022 family: PSK length is part of the contract.
    pub const fn is_2022(self) -> bool {
        matches!(
            self,
            Self::Blake3Aes128Gcm
                | Self::Blake3Aes256Gcm
                | Self::Blake3Chacha20Poly1305
                | Self::Blake3Chacha8Poly1305
        )
    }

    /// `true` for ciphers mihomo accepts but that carry no AEAD guarantee.
    pub const fn is_stream_cipher(self) -> bool {
        matches!(
            self,
            Self::None
                | Self::Table
                | Self::Rc4Md5
                | Self::Aes128Cfb
                | Self::Aes192Cfb
                | Self::Aes256Cfb
        )
    }

    /// Base64 key length mihomo derives from the cipher name; `None` for the
    /// stream ciphers, whose password is free-form.
    pub const fn key_bytes(self) -> Option<usize> {
        match self {
            Self::Blake3Aes128Gcm => Some(16),
            Self::Blake3Aes256Gcm | Self::Blake3Chacha20Poly1305 | Self::Blake3Chacha8Poly1305 => {
                Some(32)
            }
            _ => None,
        }
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Blake3Aes128Gcm => "2022 · AES-128-GCM",
            Self::Blake3Aes256Gcm => "2022 · AES-256-GCM",
            Self::Blake3Chacha20Poly1305 => "2022 · ChaCha20-Poly1305",
            Self::Blake3Chacha8Poly1305 => "2022 · ChaCha8-Poly1305",
            Self::Aes128Gcm => "AEAD · AES-128-GCM",
            Self::Aes256Gcm => "AEAD · AES-256-GCM",
            Self::Chacha20IetfPoly1305 => "AEAD · ChaCha20-IETF",
            Self::Xchacha20IetfPoly1305 => "AEAD · XChaCha20-IETF",
            Self::None => "明文 none",
            Self::Table => "流密码 · table",
            Self::Rc4Md5 => "流密码 · rc4-md5",
            Self::Aes128Cfb => "流密码 · AES-128-CFB",
            Self::Aes192Cfb => "流密码 · AES-192-CFB",
            Self::Aes256Cfb => "流密码 · AES-256-CFB",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Blake3Aes128Gcm => "2022 · AES-128-GCM",
            Self::Blake3Aes256Gcm => "2022 · AES-256-GCM",
            Self::Blake3Chacha20Poly1305 => "2022 · ChaCha20-Poly1305",
            Self::Blake3Chacha8Poly1305 => "2022 · ChaCha8-Poly1305",
            Self::Aes128Gcm => "AEAD · AES-128-GCM",
            Self::Aes256Gcm => "AEAD · AES-256-GCM",
            Self::Chacha20IetfPoly1305 => "AEAD · ChaCha20-IETF",
            Self::Xchacha20IetfPoly1305 => "AEAD · XChaCha20-IETF",
            Self::None => "Plaintext none",
            Self::Table => "Stream · table",
            Self::Rc4Md5 => "Stream · rc4-md5",
            Self::Aes128Cfb => "Stream · AES-128-CFB",
            Self::Aes192Cfb => "Stream · AES-192-CFB",
            Self::Aes256Cfb => "Stream · AES-256-CFB",
        }
    }
}

/// DUAL-05-02: VLESS flow-control values mihomo recognises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VlessFlow {
    Vision,
    VisionUdp443,
}

impl VlessFlow {
    pub const ALL: [Self; 2] = [Self::Vision, Self::VisionUdp443];

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "xtls-rprx-vision" => Some(Self::Vision),
            "xtls-rprx-vision-udp443" => Some(Self::VisionUdp443),
            _ => None,
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Vision => "xtls-rprx-vision",
            Self::VisionUdp443 => "xtls-rprx-vision-udp443",
        }
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Vision => "Vision 流控",
            Self::VisionUdp443 => "Vision 流控 (UDP443)",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Vision => "Vision flow",
            Self::VisionUdp443 => "Vision flow (UDP443)",
        }
    }
}

/// uTLS fingerprints this build documents (not an exhaustive mihomo list).
pub const UTLS_FINGERPRINTS: [&str; 8] = [
    "chrome",
    "chrome-auto",
    "firefox",
    "safari",
    "edge",
    "ios",
    "android",
    "random",
];

/// DUAL-05-02: XTLS-REALITY parameter block (`pbk` / `sid` / `spx` / `fp`).
///
/// `fingerprint` stays text because mihomo ships a wider uTLS list than this
/// build enumerates; [`RealityParams::fingerprint_known`] reports whether the
/// value is one this build documents, and validation only warns.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealityParams {
    /// `pbk` — X25519 public key, 43-44 base64 characters.
    #[serde(default)]
    pub public_key: String,
    /// `sid` — even-length hex short id, at most 16 characters.
    #[serde(default)]
    pub short_id: String,
    /// `spx` — spider path, e.g. `/spider`.
    #[serde(default)]
    pub spider_x: String,
    /// `fp` — uTLS fingerprint name.
    #[serde(default)]
    pub fingerprint: String,
}

impl RealityParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_public_key(public_key: impl Into<String>) -> Self {
        Self {
            public_key: public_key.into(),
            ..Self::default()
        }
    }

    /// `true` when any REALITY field carries a value.
    pub fn is_present(&self) -> bool {
        !self.public_key.trim().is_empty()
            || !self.short_id.trim().is_empty()
            || !self.spider_x.trim().is_empty()
            || !self.fingerprint.trim().is_empty()
    }

    /// `true` when the REALITY handshake has the field mihomo requires.
    pub fn is_complete(&self) -> bool {
        !self.public_key.trim().is_empty()
    }

    pub fn fingerprint_known(&self) -> bool {
        let fingerprint = self.fingerprint.trim();
        fingerprint.is_empty() || UTLS_FINGERPRINTS.contains(&fingerprint)
    }

    /// Human-readable REALITY problems; an empty push list means no problem.
    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        let public_key = self.public_key.trim();
        if !public_key.is_empty() {
            if !(43..=44).contains(&public_key.chars().count()) {
                issues.push(ProtocolIssue::new(
                    "reality.public-key",
                    "REALITY public-key (pbk) must be a 32-byte base64 string (43-44 chars)",
                ));
            }
        } else if !self.short_id.trim().is_empty() || !self.spider_x.trim().is_empty() {
            issues.push(ProtocolIssue::new(
                "reality.public-key",
                "REALITY short-id / spider-x require a public-key (pbk)",
            ));
        }

        let short_id = self.short_id.trim();
        if !short_id.is_empty() {
            if short_id.len() > 16 || !short_id.len().is_multiple_of(2) {
                issues.push(ProtocolIssue::new(
                    "reality.short-id",
                    "REALITY short-id (sid) must be an even-length hex string of at most 16 chars",
                ));
            } else if !short_id.chars().all(|c| c.is_ascii_hexdigit()) {
                issues.push(ProtocolIssue::new(
                    "reality.short-id",
                    "REALITY short-id (sid) contains non-hex characters",
                ));
            }
        }

        if !self.spider_x.trim().is_empty() && !self.spider_x.trim().starts_with('/') {
            issues.push(ProtocolIssue::new(
                "reality.spider-x",
                "REALITY spider-x (spx) must start with '/'",
            ));
        }

        if !self.fingerprint_known() {
            issues.push(ProtocolIssue::new(
                "reality.fingerprint",
                "uTLS fingerprint is not one of chrome / chrome-auto / firefox / safari / edge / ios / android / random",
            ));
        }
    }

    /// Chips rendered by both surfaces (05-02).
    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if self.is_complete() {
            chips.push("Reality".to_string());
        }
        let short_id = self.short_id.trim();
        if !short_id.is_empty() {
            chips.push(format!("sid:{short_id}"));
        }
        let spider_x = self.spider_x.trim();
        if !spider_x.is_empty() {
            chips.push(format!("spx:{spider_x}"));
        }
        let fingerprint = self.fingerprint.trim();
        if !fingerprint.is_empty() {
            chips.push(format!("fp:{fingerprint}"));
        }
        chips
    }
}

/// DUAL-05-11: Smux / Yamux / H2Mux connection-reuse parameters.
///
/// mihomo stores these under a `smux:` mapping; `validate` mirrors the
/// invariants that can be checked without a live core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmuxParams {
    pub enabled: bool,
    /// `smux` / `yamux` / `h2mux`.
    pub protocol: String,
    pub max_connections: u32,
    pub min_streams: u32,
    pub max_streams: u32,
    pub padding: bool,
    pub statistic: bool,
    pub only_tcp: bool,
}

impl Default for SmuxParams {
    fn default() -> Self {
        Self {
            enabled: false,
            protocol: "smux".to_string(),
            max_connections: 4,
            min_streams: 0,
            max_streams: 0,
            padding: false,
            statistic: false,
            only_tcp: false,
        }
    }
}

impl SmuxParams {
    /// Multiplexer names mihomo accepts.
    pub const PROTOCOLS: [&'static str; 3] = ["smux", "yamux", "h2mux"];

    /// `true` when any field differs from the mihomo default shape.
    pub fn has_overrides(&self) -> bool {
        *self != Self::default()
    }

    pub fn protocol_known(&self) -> bool {
        Self::PROTOCOLS.contains(&self.protocol.trim())
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if !self.enabled && self.has_overrides() {
            issues.push(ProtocolIssue::new(
                "smux.enabled",
                "multiplexing parameters are set while `smux.enabled` is off; mihomo ignores them",
            ));
        }
        if self.enabled && !self.protocol_known() {
            issues.push(ProtocolIssue::new(
                "smux.protocol",
                "multiplexer must be one of smux / yamux / h2mux",
            ));
        }
        if self.max_connections == 0 {
            issues.push(ProtocolIssue::new(
                "smux.max-connections",
                "max-connections must be positive",
            ));
        }
        if self.max_streams > 0 && self.min_streams > self.max_streams {
            issues.push(ProtocolIssue::new(
                "smux.min-streams",
                "min-streams must be less than or equal to max-streams",
            ));
        }
        if self.protocol.trim() == "h2mux" && self.padding {
            issues.push(ProtocolIssue::new(
                "smux.padding",
                "h2mux ignores smux padding; drop the padding flag or pick smux/yamux",
            ));
        }
    }

    /// Chips rendered by both surfaces (05-11); empty when at defaults.
    pub fn chips(&self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        let mut chips = vec![format!("mux:{}", self.protocol.trim())];
        chips.push(format!("mc:{}", self.max_connections));
        if self.max_streams > 0 {
            chips.push(format!("ms:{}", self.max_streams));
        }
        if self.min_streams > 0 {
            chips.push(format!("min:{}", self.min_streams));
        }
        if self.padding {
            chips.push("padding".to_string());
        }
        if self.statistic {
            chips.push("statistic".to_string());
        }
        if self.only_tcp {
            chips.push("tcp-only".to_string());
        }
        chips
    }

    pub fn summary_zh(&self) -> String {
        if !self.enabled {
            return "未启用".to_string();
        }
        let max_streams = if self.max_streams == 0 {
            "∞".to_string()
        } else {
            self.max_streams.to_string()
        };
        format!(
            "{} · 连接 {} · 流 {}~{}",
            self.protocol.trim(),
            self.max_connections,
            self.min_streams,
            max_streams
        )
    }
}

/// Node protocol identity derived from the mihomo `type` string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamily {
    Vless,
    Hysteria2,
    Tuic,
    WireGuard,
    Shadowsocks,
    Anytls,
    Trojan,
    Vmess,
    Ssh,
    #[default]
    Unknown,
}

impl ProtocolFamily {
    pub fn from_type_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "vless" => Self::Vless,
            "hysteria2" | "hy2" => Self::Hysteria2,
            "tuic" => Self::Tuic,
            "wireguard" | "wg" | "awg" | "amnezia-wg" => Self::WireGuard,
            "ss" | "shadowsocks" => Self::Shadowsocks,
            "anytls" => Self::Anytls,
            "trojan" | "trojan-go" => Self::Trojan,
            "vmess" => Self::Vmess,
            "ssh" => Self::Ssh,
            _ => Self::Unknown,
        }
    }

    /// Families whose mihomo schema carries the REALITY block.
    pub const fn supports_reality(self) -> bool {
        matches!(self, Self::Vless | Self::Anytls)
    }

    /// Families whose mihomo schema carries a `smux:` mapping.
    pub const fn supports_smux(self) -> bool {
        matches!(
            self,
            Self::Vless | Self::Vmess | Self::Trojan | Self::Shadowsocks | Self::Anytls
        )
    }

    /// Families whose `cipher` is a Shadowsocks cipher name.
    pub const fn supports_cipher_family(self) -> bool {
        matches!(self, Self::Shadowsocks)
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Vless => "VLESS",
            Self::Hysteria2 => "Hysteria 2",
            Self::Tuic => "TUIC v5",
            Self::WireGuard => "WireGuard / AmneziaWG",
            Self::Shadowsocks => "Shadowsocks",
            Self::Anytls => "AnyTLS",
            Self::Trojan => "Trojan",
            Self::Vmess => "VMess",
            Self::Ssh => "SSH",
            Self::Unknown => "未知协议",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Vless => "VLESS",
            Self::Hysteria2 => "Hysteria 2",
            Self::Tuic => "TUIC v5",
            Self::WireGuard => "WireGuard / AmneziaWG",
            Self::Shadowsocks => "Shadowsocks",
            Self::Anytls => "AnyTLS",
            Self::Trojan => "Trojan",
            Self::Vmess => "VMess",
            Self::Ssh => "SSH",
            Self::Unknown => "Unknown protocol",
        }
    }
}

/// One human-readable protocol problem raised by the shared application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolIssue {
    /// Stable machine key, e.g. `reality.public-key`.
    pub field: String,
    pub message: String,
}

impl ProtocolIssue {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// DUAL-05-14: the shared editable node draft used by both surfaces.
///
/// Text fields stay text: the draft never invents a cipher, fingerprint, or
/// flow. Typed families are *derived* (`family`, `cipher_family`,
/// `flow_family`), so the surfaces render shared vocabulary while the codec
/// keeps values it does not understand byte-for-byte.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDraft {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub node_type: String,
    /// `password:` — Shadowsocks / Trojan / Hysteria2 / AnyTLS / SSH.
    pub password: String,
    /// `uuid:` — VLESS / VMess (and TUIC's `uuid` half).
    pub uuid: String,
    pub sni: String,
    pub cipher: String,
    pub flow: String,
    pub reality: RealityParams,
    pub smux: SmuxParams,
    pub tls: bool,
    pub skip_cert_verify: bool,
    pub alpn: Vec<String>,
    /// DUAL-05-03…05-12: typed parameter blocks (congestion, WireGuard keys,
    /// transports, SIP003 plugins, SSH, AnyTLS, ECH). One field keeps the
    /// draft vocabulary in `protocol_params`.
    #[serde(default)]
    pub params: crate::protocol_params::ProtocolParams,
    /// Keys captured by the codec's flatten catch-all and passed through
    /// verbatim (DUAL-05-14 未知字段无损流通).
    #[serde(default)]
    pub preserved_fields: Vec<String>,
}

impl ProtocolDraft {
    pub fn new(node_type: impl Into<String>) -> Self {
        Self {
            node_type: node_type.into(),
            ..Self::default()
        }
    }

    pub fn family(&self) -> ProtocolFamily {
        ProtocolFamily::from_type_str(&self.node_type)
    }

    pub fn cipher_family(&self) -> Option<ShadowsocksCipher> {
        ShadowsocksCipher::parse(&self.cipher)
    }

    pub fn flow_family(&self) -> Option<VlessFlow> {
        VlessFlow::parse(&self.flow)
    }

    /// `true` when REALITY would be requested by the codec for this draft.
    pub fn uses_reality(&self) -> bool {
        self.family().supports_reality() && self.reality.is_complete()
    }

    /// The credential slots this family requires, in mihomo's schema terms.
    pub fn required_credentials(&self) -> &'static [&'static str] {
        match self.family() {
            ProtocolFamily::Vless | ProtocolFamily::Vmess => &["uuid"],
            ProtocolFamily::Tuic => &["uuid", "password"],
            ProtocolFamily::Shadowsocks
            | ProtocolFamily::Trojan
            | ProtocolFamily::Anytls
            | ProtocolFamily::Hysteria2 => &["password"],
            // SSH accepts a password *or* a private key; that OR-rule lives in
            // `SshParams::validate`, so the slot list stays empty here.
            ProtocolFamily::Ssh => &[],
            ProtocolFamily::WireGuard => &["private-key"],
            ProtocolFamily::Unknown => &[],
        }
    }

    /// `true` when the family's required credential slots are filled.
    pub fn has_required_credentials(&self) -> bool {
        self.required_credentials().iter().all(|slot| match *slot {
            "uuid" => !self.uuid.trim().is_empty(),
            "password" => !self.password.trim().is_empty(),
            "private-key" => !self.params.wireguard.private_key.trim().is_empty(),
            _ => true,
        })
    }

    /// The single credential text a two-slot family (TUIC) shows in one field.
    /// `None` when neither slot carries a value.
    pub fn password_or_uuid(&self) -> String {
        if !self.uuid.trim().is_empty() {
            self.uuid.clone()
        } else {
            self.password.clone()
        }
    }

    pub fn validate(&self) -> Vec<ProtocolIssue> {
        let mut issues = Vec::new();
        if self.name.trim().is_empty() {
            issues.push(ProtocolIssue::new("name", "node name must not be empty"));
        }
        if self.server.trim().is_empty() {
            issues.push(ProtocolIssue::new("server", "server must not be empty"));
        }
        if self.port == 0 {
            issues.push(ProtocolIssue::new("port", "port must be positive"));
        }
        for slot in self.required_credentials() {
            let filled = match *slot {
                "uuid" => !self.uuid.trim().is_empty(),
                "password" => !self.password.trim().is_empty(),
                "private-key" => !self.params.wireguard.private_key.trim().is_empty(),
                _ => true,
            };
            if !filled {
                issues.push(ProtocolIssue::new(
                    *slot,
                    format!("`{slot}` is required by {}", self.family().label_en()),
                ));
            }
        }

        if !self.cipher.trim().is_empty() && self.cipher_family().is_none() {
            issues.push(ProtocolIssue::new(
                "cipher",
                "cipher is not a mihomo cipher name; it is written back verbatim",
            ));
        }
        if let Some(cipher) = self.cipher_family()
            && cipher.is_2022()
            && let Some(key_bytes) = cipher.key_bytes()
        {
            check_2022_psk(self.password.trim(), key_bytes, &mut issues);
        }

        if !self.flow.trim().is_empty() {
            match self.flow_family() {
                Some(_) if self.family() != ProtocolFamily::Vless => issues.push(
                    ProtocolIssue::new("flow", "flow control only applies to VLESS"),
                ),
                Some(_) => {}
                None => issues.push(ProtocolIssue::new(
                    "flow",
                    "unknown VLESS flow; expected xtls-rprx-vision or xtls-rprx-vision-udp443",
                )),
            }
        }

        if self.reality.is_present() {
            if self.family().supports_reality() {
                self.reality.validate(&mut issues);
            } else {
                issues.push(ProtocolIssue::new(
                    "reality.unsupported",
                    "REALITY fields are set for a protocol whose schema has no reality-opts block; the core ignores them",
                ));
            }
        }

        if self.smux.has_overrides() && !self.family().supports_smux() {
            issues.push(ProtocolIssue::new(
                "smux.unsupported",
                "multiplexing parameters are set for a protocol whose schema has no smux block; the core ignores them",
            ));
        }
        self.smux.validate(&mut issues);

        if self.alpn.iter().any(|entry| entry.trim().is_empty()) {
            issues.push(ProtocolIssue::new("alpn", "ALPN entries must not be empty"));
        }

        // DUAL-05-03…05-12: typed parameter blocks, gated by the family.
        self.params
            .validate(self.family(), &self.password, &self.alpn, &mut issues);

        issues
    }

    pub fn report(&self) -> ProtocolFidelityReport {
        ProtocolFidelityReport::from_draft(self)
    }
}

/// Validate a Shadowsocks-2022 PSK: one base64 key, or `key:key` for the
/// multi-user form, each decoding to the cipher's key size.
fn check_2022_psk(password: &str, key_bytes: usize, issues: &mut Vec<ProtocolIssue>) {
    if password.is_empty() {
        return;
    }
    for part in password.split(':') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK components must not be empty",
            ));
            return;
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK must be standard base64 (one key, or key:key for multi-user)",
            ));
            return;
        }
        let padding = trimmed.chars().filter(|c| *c == '=').count();
        let decoded = (trimmed.len() * 3 / 4).saturating_sub(padding);
        if decoded != key_bytes {
            issues.push(ProtocolIssue::new(
                "password",
                "2022 PSK length does not match the cipher key size",
            ));
            return;
        }
    }
}

/// DUAL-05-01/02/11/14: the derived read model both surfaces render.
///
/// Every field is computed from [`ProtocolDraft`] with nothing stored twice,
/// so Iced and Bevy cannot disagree about a node's protocol facts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolFidelityReport {
    pub family: ProtocolFamily,
    pub cipher_chip: Option<String>,
    pub cipher_is_2022: bool,
    pub cipher_key_bytes: Option<usize>,
    pub flow_chip: Option<String>,
    pub reality_chips: Vec<String>,
    pub smux_chips: Vec<String>,
    pub smux_overrides: bool,
    pub issues: Vec<ProtocolIssue>,
    /// DUAL-05-03…05-12: typed parameter chips + honest non-blocking notes.
    #[serde(default)]
    pub params: crate::protocol_params::ProtocolParamsReport,
}

impl ProtocolFidelityReport {
    pub fn from_draft(draft: &ProtocolDraft) -> Self {
        let cipher = draft.cipher_family();
        let smux = &draft.smux;
        Self {
            family: draft.family(),
            cipher_chip: cipher.map(|value| value.label_zh().to_string()),
            cipher_is_2022: cipher.is_some_and(ShadowsocksCipher::is_2022),
            cipher_key_bytes: cipher.and_then(ShadowsocksCipher::key_bytes),
            flow_chip: draft
                .flow_family()
                .map(|value| value.label_zh().to_string()),
            reality_chips: if draft.family().supports_reality() {
                draft.reality.chips()
            } else {
                Vec::new()
            },
            smux_chips: if draft.family().supports_smux() {
                smux.chips()
            } else {
                Vec::new()
            },
            smux_overrides: smux.has_overrides(),
            issues: draft.validate(),
            params: crate::protocol_params::ProtocolParamsReport::from_draft(draft),
        }
    }

    /// Every chip both surfaces render: family, cipher, flow, REALITY, smux and
    /// the DUAL-05-03…05-12 parameter chips.
    pub fn all_chips(&self) -> Vec<String> {
        let mut chips = vec![self.family.label_zh().to_string()];
        if let Some(cipher) = &self.cipher_chip {
            chips.push(cipher.clone());
        }
        if let Some(flow) = &self.flow_chip {
            chips.push(flow.clone());
        }
        chips.extend(self.reality_chips.iter().cloned());
        chips.extend(self.smux_chips.iter().cloned());
        if let Some(bytes) = self.cipher_key_bytes {
            chips.push(format!("PSK {bytes}B"));
        }
        chips.extend(self.params.chips());
        chips
    }

    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn issue_lines(&self) -> Vec<String> {
        self.issues
            .iter()
            .map(|issue| format!("[{}] {}", issue.field, issue.message))
            .collect()
    }
}

/// DUAL-05-14: node/profile codec formats understood by the shared converter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCodecFormat {
    #[default]
    ClashYaml,
    RawJson,
    Uri,
    Base64Subscription,
}

impl NodeCodecFormat {
    pub const ALL: [Self; 4] = [
        Self::ClashYaml,
        Self::RawJson,
        Self::Uri,
        Self::Base64Subscription,
    ];

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::ClashYaml => "Clash YAML",
            Self::RawJson => "节点 JSON",
            Self::Uri => "分享链接 URI",
            Self::Base64Subscription => "Base64 订阅",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::ClashYaml => "Clash YAML",
            Self::RawJson => "Node JSON",
            Self::Uri => "Share-link URI",
            Self::Base64Subscription => "Base64 subscription",
        }
    }
}

/// DUAL-05-14: honest audit of one nodes-codec conversion.
///
/// `unknown_fields` lists keys the typed model did not recognise but passed
/// through verbatim; `structure_preserved` is `true` only when the document
/// round-trip is semantically equal at the YAML value level.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodecAudit {
    pub source_format: NodeCodecFormat,
    pub target_format: NodeCodecFormat,
    pub node_count: usize,
    pub unknown_fields: Vec<String>,
    pub structure_preserved: bool,
    pub lossless: bool,
    pub detail: String,
}

/// DUAL-05: the shared custom-node studio snapshot published by the
/// application and rendered by both surfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStudioSnapshot {
    #[serde(default)]
    pub draft: Option<ProtocolDraft>,
    #[serde(default)]
    pub report: Option<ProtocolFidelityReport>,
    #[serde(default)]
    pub uri_preview: Option<String>,
    #[serde(default)]
    pub audit: Option<CodecAudit>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_saved_node: Option<String>,
    /// Draft fields a share link cannot carry, computed by actually
    /// round-tripping the draft through the URI codec (honest, never hard-coded).
    #[serde(default)]
    pub uri_gaps: Vec<String>,
}

impl ProtocolStudioSnapshot {
    pub fn issue_lines(&self) -> Vec<String> {
        self.report
            .as_ref()
            .map(ProtocolFidelityReport::issue_lines)
            .unwrap_or_default()
    }

    pub fn has_blocking_issue(&self) -> bool {
        self.report
            .as_ref()
            .is_some_and(|report| !report.is_valid())
    }

    /// `true` when a share link cannot express every field of the draft.
    pub fn uri_is_lossy(&self) -> bool {
        !self.uri_gaps.is_empty()
    }

    /// This snapshot with [`ProtocolStudioSnapshot::last_error`] replaced.
    /// Surfaces use it to keep the error local instead of re-reading the
    /// process-wide publication.
    pub fn with_error(mut self, message: impl Into<String>) -> Self {
        self.last_error = Some(message.into());
        self
    }
}
