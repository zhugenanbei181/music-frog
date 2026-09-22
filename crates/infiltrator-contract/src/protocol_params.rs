//! DUAL-05 typed protocol parameter blocks (05-03…05-12).
//!
//! The pinned core is mihomo v1.19.18 (`scripts/fetch-mihomo.sh`); every
//! vocabulary below was checked against that release's outbound option
//! structs and against the vendored binary's `proxy:` tag table. Two facts
//! deserve to be stated here because the studio must never pretend otherwise:
//!
//! * `masquerade` is a *listener* field (`Masquerade-yaml:"masquerade"` in the
//!   binary); no client node schema carries it, so the node draft does not
//!   invent one. The ledger keeps this grep evidence.
//! * v1.19.18 has no `xhttp` transport and no `quic` network: VLESS switches on
//!   `tcp`/`ws`/`http`/`h2`/`grpc` only and falls back to TCP for anything
//!   else. The draft still models and preserves `xhttp-opts` (the domain has
//!   typed support and the codec is forward-compatible), but the report tells
//!   the truth with a non-blocking note instead of claiming runtime support.
//!
//! The typed blocks are deliberately *scalar and lossless*: unknown values
//! round-trip verbatim, validation only reports what breaks the config, and
//! facts the pinned core silently ignores/falls back on are notes rather than
//! blocking issues (see [`ProtocolParamsReport::notes`]).

use serde::{Deserialize, Serialize};

use crate::protocol_fidelity::{ProtocolDraft, ProtocolFamily, ProtocolIssue};
use crate::protocol_params_ext::{
    AnyTlsParams, Sip003Plugin, SshParams, TransportParams, TrojanSsParams, WireGuardParams,
    is_base64, is_positive_number_or_bandwidth, note, push,
};

/// SIP003 plugins the pinned mihomo v1.19.18 implements (`adapter/outbound/shadowsocks.go`).
pub const KNOWN_SIP003_PLUGINS: [&str; 6] = [
    "obfs",
    "v2ray-plugin",
    "gost-plugin",
    "shadow-tls",
    "restls-plugin",
    "kcptun",
];

/// TUIC congestion controllers the vendored core can hand to the QUIC stack.
pub const KNOWN_CONGESTION_CONTROLLERS: [&str; 3] = ["bbr", "cubic", "new-reno"];

/// TUIC v5 UDP relay modes; anything else silently becomes `native`.
pub const KNOWN_UDP_RELAY_MODES: [&str; 2] = ["native", "quic"];

/// Hysteria2 obfuscation types in v1.19.18 (salamander only).
pub const KNOWN_HYSTERIA2_OBFS: [&str; 1] = ["salamander"];

/// Transport networks v1.19.18 understands (vless/trojan/vmess `network:`).
pub const KNOWN_TRANSPORT_NETWORKS: [&str; 6] = ["tcp", "ws", "grpc", "h2", "http", "xhttp"];

/// XHTTP modes this build documents; unknown modes are preserved, not rewritten.
pub const XHTTP_MODES: [&str; 4] = ["auto", "packet-up", "stream-up", "stream-one"];

/// DUAL-05-12: `ech-opts` — Encrypted Client Hello (mihomo `ECHPptions`).
///
/// `enable: true` with an empty `config` is valid: the core resolves the
/// ECHConfigList over DNS. A base64 `config` is the pinned inline form.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EchParams {
    pub enabled: bool,
    /// base64 ECHConfigList; empty means the core resolves it by name.
    pub config: String,
}

impl EchParams {
    pub fn is_present(&self) -> bool {
        self.enabled || !self.config.trim().is_empty()
    }

    /// Families whose node schema carries `ech-opts` in v1.19.18.
    pub const fn supported_by(family: ProtocolFamily) -> bool {
        matches!(
            family,
            ProtocolFamily::Vless
                | ProtocolFamily::Hysteria2
                | ProtocolFamily::Tuic
                | ProtocolFamily::Trojan
                | ProtocolFamily::Anytls
        )
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        let config = self.config.trim();
        if !self.enabled && !config.is_empty() {
            push(
                issues,
                "ech.enable",
                "ECH config is set while ech-opts.enable is off; the core ignores it",
            );
        }
        if self.enabled && !config.is_empty() && !is_base64(config) {
            push(
                issues,
                "ech.config",
                "ECH config must be a base64 ECHConfigList (or empty to resolve it over DNS)",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        if self.config.trim().is_empty() {
            vec!["ech:dns".to_string()]
        } else {
            vec!["ech:inline".to_string()]
        }
    }
}

/// DUAL-05-03: TUIC v5 congestion/relay parameters (`adapter/outbound/tuic.go`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TuicParams {
    /// `congestion-controller` — `bbr` / `cubic` / `new-reno`.
    pub congestion_controller: String,
    /// `udp-relay-mode` — `native` / `quic`; anything else becomes `native`.
    pub udp_relay_mode: String,
    pub reduce_rtt: bool,
    /// milliseconds; `0` means the core default (10000).
    pub heartbeat_interval: u64,
    /// milliseconds; `0` means the core default (8000).
    pub request_timeout: u64,
    pub recv_window_conn: u64,
    pub recv_window: u64,
    pub disable_sni: bool,
}

impl TuicParams {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn congestion_controller_known(&self) -> bool {
        let value = self.congestion_controller.trim();
        value.is_empty() || KNOWN_CONGESTION_CONTROLLERS.contains(&value)
    }

    pub fn udp_relay_mode_known(&self) -> bool {
        let value = self.udp_relay_mode.trim();
        value.is_empty() || KNOWN_UDP_RELAY_MODES.contains(&value)
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if self.recv_window > 0 && self.recv_window_conn > self.recv_window {
            push(
                issues,
                "tuic.recv-window",
                "recv-window must be at least recv-window-conn",
            );
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        let cc = self.congestion_controller.trim();
        if !cc.is_empty() {
            chips.push(format!("cc:{cc}"));
        }
        let mode = self.udp_relay_mode.trim();
        if !mode.is_empty() {
            chips.push(format!("udp:{mode}"));
        }
        if self.reduce_rtt {
            chips.push("reduce-rtt".to_string());
        }
        if self.heartbeat_interval > 0 {
            chips.push(format!("hb:{}ms", self.heartbeat_interval));
        }
        if self.recv_window > 0 {
            chips.push(format!("recv:{}", self.recv_window));
        }
        if self.disable_sni {
            chips.push("no-sni".to_string());
        }
        chips
    }

    pub fn notes(&self, notes: &mut Vec<String>) {
        if !self.congestion_controller_known() {
            note(
                notes,
                format!(
                    "congestion-controller `{}` is preserved but is not one of bbr / cubic / new-reno; the TUIC client may reject it at runtime",
                    self.congestion_controller.trim()
                ),
            );
        }
        if !self.udp_relay_mode_known() {
            note(
                notes,
                format!(
                    "udp-relay-mode `{}` is silently treated as `native` by the pinned core",
                    self.udp_relay_mode.trim()
                ),
            );
        }
    }
}

/// DUAL-05-03: Hysteria2 port hopping / obfs / bandwidth (`adapter/outbound/hysteria2.go`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hysteria2Params {
    /// `ports:` — comma list of ports and `start-end` ranges (`mport` in links).
    pub ports: String,
    /// `hop-interval` in seconds; the core clamps anything below 5.
    pub hop_interval: u64,
    /// `obfs:` — `salamander` in v1.19.18.
    pub obfs: String,
    pub obfs_password: String,
    pub up: String,
    pub down: String,
    /// `cwnd` — congestion window; `0` means the core default.
    pub cwnd: u64,
    /// `udp-mtu`; `0` means the core default (1197).
    pub udp_mtu: u64,
}

impl Hysteria2Params {
    pub fn is_present(&self) -> bool {
        self != &Self::default()
    }

    pub fn obfs_known(&self) -> bool {
        let value = self.obfs.trim();
        value.is_empty() || KNOWN_HYSTERIA2_OBFS.contains(&value)
    }

    /// Validate the comma list of ports/ranges; `true` when it parses.
    pub fn ports_valid(&self) -> bool {
        let value = self.ports.trim();
        if value.is_empty() {
            return true;
        }
        let mut seen = false;
        for chunk in value.split(',') {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            seen = true;
            let range = chunk.split_once('-');
            let (start, end) = match range {
                Some((start, end)) => (start.trim(), Some(end.trim())),
                None => (chunk, None),
            };
            let Ok(start) = start.parse::<u16>() else {
                return false;
            };
            if start == 0 {
                return false;
            }
            if let Some(end) = end {
                let Ok(end) = end.parse::<u16>() else {
                    return false;
                };
                if end == 0 || start > end {
                    return false;
                }
            }
        }
        seen
    }

    pub fn port_count(&self) -> usize {
        let mut total = 0;
        for chunk in self.ports.split(',') {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                continue;
            }
            total += match chunk.split_once('-') {
                Some((start, end)) => {
                    let start = start.trim().parse::<usize>().unwrap_or(0);
                    let end = end.trim().parse::<usize>().unwrap_or(0);
                    end.saturating_sub(start) + 1
                }
                None => 1,
            };
        }
        total
    }

    pub fn validate(&self, issues: &mut Vec<ProtocolIssue>) {
        if !self.ports_valid() {
            push(
                issues,
                "hysteria2.ports",
                "port hopping must be a comma list of ports or `start-end` ranges (1-65535)",
            );
        }
        if !self.obfs.trim().is_empty() {
            if !self.obfs_known() {
                push(
                    issues,
                    "hysteria2.obfs",
                    "the pinned core only implements `salamander` obfs",
                );
            }
            if self.obfs_password.trim().is_empty() {
                push(
                    issues,
                    "hysteria2.obfs-password",
                    "obfs requires an obfs-password",
                );
            }
        } else if !self.obfs_password.trim().is_empty() {
            push(
                issues,
                "hysteria2.obfs",
                "obfs-password is set without an obfs type",
            );
        }
        if self.hop_interval > 0 && self.ports.trim().is_empty() {
            push(
                issues,
                "hysteria2.hop-interval",
                "hop-interval needs a `ports` (mport) list to hop over",
            );
        }
        for (field, value) in [("hysteria2.up", &self.up), ("hysteria2.down", &self.down)] {
            let value = value.trim();
            if !value.is_empty() && !is_positive_number_or_bandwidth(value) {
                push(
                    issues,
                    field,
                    "bandwidth must start with a number, e.g. `100 Mbps` or `100`",
                );
            }
        }
    }

    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        let ports = self.ports.trim();
        if !ports.is_empty() {
            chips.push(format!("ports:{ports}"));
        }
        if self.hop_interval > 0 {
            chips.push(format!("hop:{}s", self.hop_interval));
        }
        let obfs = self.obfs.trim();
        if !obfs.is_empty() {
            chips.push(format!("obfs:{obfs}"));
        }
        if !self.up.trim().is_empty() {
            chips.push(format!("up:{}", self.up.trim()));
        }
        if !self.down.trim().is_empty() {
            chips.push(format!("down:{}", self.down.trim()));
        }
        if self.cwnd > 0 {
            chips.push(format!("cwnd:{}", self.cwnd));
        }
        if self.udp_mtu > 0 {
            chips.push(format!("udp-mtu:{}", self.udp_mtu));
        }
        chips
    }

    pub fn notes(&self, notes: &mut Vec<String>) {
        if self.hop_interval > 0 && self.hop_interval < 5 {
            note(
                notes,
                "hop-interval below 5s is raised to 5s by the pinned core",
            );
        }
    }
}

/// DUAL-05-03…05-12: every typed protocol parameter block in one value.
///
/// `ProtocolDraft` carries this as one field so a new block never requires a
/// new draft method; validation and the report stay here with the vocabulary.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtocolParams {
    pub ech: EchParams,
    pub tuic: TuicParams,
    pub hysteria2: Hysteria2Params,
    pub wireguard: WireGuardParams,
    pub transport: TransportParams,
    pub plugin: Sip003Plugin,
    pub ssh: SshParams,
    pub anytls: AnyTlsParams,
    pub trojan_ss: TrojanSsParams,
    /// DUAL-05-13: custom CA / certificate whitelist.
    pub tls_trust: crate::protocol_trust::TlsTrustParams,
}

impl ProtocolParams {
    /// Validate every block that is set, gated by the node family.
    pub fn validate(
        &self,
        family: ProtocolFamily,
        password: &str,
        alpn: &[String],
        issues: &mut Vec<ProtocolIssue>,
    ) {
        let gate = |present: bool, supported: bool, unsupported_field: &str| {
            if present && !supported {
                return Some(unsupported_field.to_string());
            }
            None
        };
        if let Some(field) = gate(
            self.ech.is_present(),
            EchParams::supported_by(family),
            "ech.unsupported",
        ) {
            push(
                issues,
                &field,
                "ech-opts is not part of this protocol's v1.19.18 schema; the core ignores it",
            );
        } else if self.ech.is_present() {
            self.ech.validate(issues);
        }
        if let Some(field) = gate(
            self.tuic.is_present(),
            family == ProtocolFamily::Tuic,
            "tuic.unsupported",
        ) {
            push(
                issues,
                &field,
                "TUIC parameters are set on a non-TUIC node; the core ignores them",
            );
        } else if self.tuic.is_present() {
            self.tuic.validate(issues);
        }
        if let Some(field) = gate(
            self.hysteria2.is_present(),
            family == ProtocolFamily::Hysteria2,
            "hysteria2.unsupported",
        ) {
            push(
                issues,
                &field,
                "Hysteria2 parameters are set on a non-Hysteria2 node; the core ignores them",
            );
        } else if self.hysteria2.is_present() {
            self.hysteria2.validate(issues);
        }
        if let Some(field) = gate(
            self.wireguard.is_present(),
            family == ProtocolFamily::WireGuard,
            "wireguard.unsupported",
        ) {
            push(
                issues,
                &field,
                "WireGuard parameters are set on a non-WireGuard node; the core ignores them",
            );
        } else if self.wireguard.is_present() {
            self.wireguard.validate(issues);
        }
        if let Some(field) = gate(
            self.transport.is_present(),
            TransportParams::supported_by(family),
            "transport.unsupported",
        ) {
            push(
                issues,
                &field,
                "transport options are set on a protocol whose v1.19.18 schema has no network block; the core ignores them",
            );
        } else if self.transport.is_present() {
            self.transport.validate(issues);
        }
        if let Some(field) = gate(
            self.plugin.is_present(),
            family == ProtocolFamily::Shadowsocks,
            "plugin.unsupported",
        ) {
            push(
                issues,
                &field,
                "SIP003 plugins only exist on the Shadowsocks schema; the core ignores them here",
            );
        } else if self.plugin.is_present() {
            self.plugin.validate(issues);
        }
        if family == ProtocolFamily::Ssh {
            // The identity rules always apply to an SSH node, even when every
            // optional slot is still empty.
            self.ssh.validate(password, issues);
        } else if self.ssh.is_present() {
            push(
                issues,
                "ssh.unsupported",
                "SSH parameters are set on a non-SSH node; the core ignores them",
            );
        }
        if let Some(field) = gate(
            self.anytls.is_present(),
            family == ProtocolFamily::Anytls,
            "anytls.unsupported",
        ) {
            push(
                issues,
                &field,
                "AnyTLS session parameters are set on a non-AnyTLS node; the core ignores them",
            );
        }
        if let Some(field) = gate(
            self.trojan_ss.is_present(),
            family == ProtocolFamily::Trojan,
            "trojan.ss-opts.unsupported",
        ) {
            push(
                issues,
                &field,
                "trojan ss-opts is set on a non-Trojan node; the core ignores it",
            );
        } else if self.trojan_ss.is_present() {
            self.trojan_ss.validate(issues);
        }

        // DUAL-05-13: certificate trust is a profile-level store; structural
        // validation always runs and the carrier facts are non-blocking notes
        // (`ca` -> global `tls.custom-certifactes`, inline `ca-str` has no
        // v1.19.18 carrier), never a fabricated per-node claim.
        if self.tls_trust.is_present() {
            self.tls_trust.validate(issues);
        }
        // Multi-version ALPN is an ordered preference list: preserve the order,
        // reject duplicates (the negotiation order would be ambiguous).
        let mut seen: Vec<&str> = Vec::new();
        for entry in alpn {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if seen.contains(&entry) {
                push(
                    issues,
                    "alpn",
                    "ALPN entries must be unique to keep one negotiation order",
                );
                break;
            }
            seen.push(entry);
        }
    }

    /// The honest, non-blocking facts the surfaces display next to the issues.
    pub fn notes(&self, family: ProtocolFamily) -> Vec<String> {
        let mut notes = Vec::new();
        if family == ProtocolFamily::Tuic {
            self.tuic.notes(&mut notes);
        }
        if family == ProtocolFamily::Hysteria2 {
            self.hysteria2.notes(&mut notes);
        }
        if TransportParams::supported_by(family) {
            self.transport.notes(&mut notes);
        }
        if family == ProtocolFamily::Shadowsocks {
            self.plugin.notes(&mut notes);
        }
        notes.extend(self.tls_trust.notes());
        notes
    }
}

/// DUAL-05-03…05-12: the shared read model for the typed parameter blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProtocolParamsReport {
    pub ech_chip: Option<String>,
    pub alpn_chips: Vec<String>,
    pub congestion_chips: Vec<String>,
    pub wireguard_chips: Vec<String>,
    pub transport_chips: Vec<String>,
    pub plugin_chips: Vec<String>,
    pub ssh_chips: Vec<String>,
    pub anytls_chips: Vec<String>,
    /// DUAL-05-08: trojan-go's `ss-opts` chips (kept separate from AnyTLS).
    pub trojan_chips: Vec<String>,
    /// DUAL-05-13: custom CA / certificate whitelist chips.
    pub ca_chips: Vec<String>,
    /// Facts the pinned core ignores or falls back on; never blocking.
    pub notes: Vec<String>,
}

impl ProtocolParamsReport {
    pub fn from_draft(draft: &ProtocolDraft) -> Self {
        let family = draft.family();
        let params = &draft.params;
        let mut congestion_chips = Vec::new();
        if family == ProtocolFamily::Tuic {
            congestion_chips.extend(params.tuic.chips());
        }
        if family == ProtocolFamily::Hysteria2 {
            congestion_chips.extend(params.hysteria2.chips());
        }
        let mut transport_chips = Vec::new();
        if TransportParams::supported_by(family) {
            transport_chips.extend(params.transport.chips());
        }
        let wireguard_chips = if family == ProtocolFamily::WireGuard {
            params.wireguard.chips()
        } else {
            Vec::new()
        };
        let plugin_chips = if family == ProtocolFamily::Shadowsocks {
            params.plugin.chips()
        } else {
            Vec::new()
        };
        let ssh_chips = if family == ProtocolFamily::Ssh {
            params.ssh.chips()
        } else {
            Vec::new()
        };
        let anytls_chips = if family == ProtocolFamily::Anytls {
            params.anytls.chips()
        } else {
            Vec::new()
        };
        let trojan_chips = if family == ProtocolFamily::Trojan {
            params.trojan_ss.chips()
        } else {
            Vec::new()
        };
        Self {
            ech_chip: if EchParams::supported_by(family) {
                params.ech.chips().into_iter().next()
            } else {
                None
            },
            alpn_chips: draft
                .alpn
                .iter()
                .enumerate()
                .filter(|(_, entry)| !entry.trim().is_empty())
                .map(|(index, entry)| format!("alpn[{index}]:{}", entry.trim()))
                .collect(),
            congestion_chips,
            wireguard_chips,
            transport_chips,
            plugin_chips,
            ssh_chips,
            anytls_chips,
            trojan_chips,
            ca_chips: params.tls_trust.chips(),
            notes: params.notes(family),
        }
    }

    /// Every chip in one list, in render order, for the surface chip rows.
    pub fn chips(&self) -> Vec<String> {
        let mut chips = Vec::new();
        if let Some(ech) = &self.ech_chip {
            chips.push(ech.clone());
        }
        chips.extend(self.alpn_chips.iter().cloned());
        chips.extend(self.congestion_chips.iter().cloned());
        chips.extend(self.wireguard_chips.iter().cloned());
        chips.extend(self.transport_chips.iter().cloned());
        chips.extend(self.plugin_chips.iter().cloned());
        chips.extend(self.ssh_chips.iter().cloned());
        chips.extend(self.anytls_chips.iter().cloned());
        chips.extend(self.trojan_chips.iter().cloned());
        chips.extend(self.ca_chips.iter().cloned());
        chips
    }
}
