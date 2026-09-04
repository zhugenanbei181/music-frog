//! Vector clock tracking, 3-way profile/YAML merge engine, and P2P pairing tokens.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[cfg(test)]
#[path = "vector_clock_test.rs"]
mod tests;

/// Relative ordering between two vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockOrdering {
    /// Both vector clocks have identical generations for all devices.
    Equal,
    /// `self` is strictly ahead of `other` (has >= in all components, and > in at least one).
    Dominates,
    /// `self` is strictly behind `other` (has <= in all components, and < in at least one).
    Subordinate,
    /// Clocks have diverged with independent concurrent updates.
    Concurrent,
}

pub type ClockRelation = ClockOrdering;

impl ClockOrdering {
    pub fn is_equal(&self) -> bool {
        matches!(self, ClockOrdering::Equal)
    }
    pub fn dominates(&self) -> bool {
        matches!(self, ClockOrdering::Dominates)
    }
    pub fn is_subordinate(&self) -> bool {
        matches!(self, ClockOrdering::Subordinate)
    }
    pub fn is_concurrent(&self) -> bool {
        matches!(self, ClockOrdering::Concurrent)
    }
    pub fn is_conflict(&self) -> bool {
        matches!(self, ClockOrdering::Concurrent)
    }
}

/// Logical vector clock tracking generations across distributed actors/devices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    pub entries: HashMap<String, u64>,
    #[serde(default)]
    pub updated_at: u64,
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            updated_at: current_unix_timestamp(),
        }
    }

    pub fn with_device(device_id: impl Into<String>, generation: u64) -> Self {
        let mut entries = HashMap::new();
        entries.insert(device_id.into(), generation);
        Self {
            entries,
            updated_at: current_unix_timestamp(),
        }
    }

    pub fn with_entries(entries: HashMap<String, u64>, updated_at: u64) -> Self {
        Self {
            entries,
            updated_at,
        }
    }

    pub fn get(&self, device_id: &str) -> u64 {
        self.entries.get(device_id).copied().unwrap_or(0)
    }

    pub fn set(&mut self, device_id: &str, generation: u64) {
        self.entries.insert(device_id.to_string(), generation);
        self.updated_at = current_unix_timestamp();
    }

    pub fn increment(&mut self, device_id: &str) {
        let count = self.entries.entry(device_id.to_string()).or_insert(0);
        *count += 1;
        self.updated_at = current_unix_timestamp();
    }

    pub fn merge(&mut self, other: &VectorClock) {
        for (device_id, &remote_gen) in &other.entries {
            let entry = self.entries.entry(device_id.clone()).or_insert(0);
            *entry = (*entry).max(remote_gen);
        }
        self.updated_at = self.updated_at.max(other.updated_at);
    }

    pub fn compare(&self, other: &VectorClock) -> ClockOrdering {
        let mut self_has_greater = false;
        let mut other_has_greater = false;

        let mut all_devices: HashSet<&String> = self.entries.keys().collect();
        all_devices.extend(other.entries.keys());

        for device in all_devices {
            let v1 = self.entries.get(device).copied().unwrap_or(0);
            let v2 = other.entries.get(device).copied().unwrap_or(0);
            match v1.cmp(&v2) {
                Ordering::Greater => self_has_greater = true,
                Ordering::Less => other_has_greater = true,
                Ordering::Equal => {}
            }
        }

        match (self_has_greater, other_has_greater) {
            (false, false) => ClockOrdering::Equal,
            (true, false) => ClockOrdering::Dominates,
            (false, true) => ClockOrdering::Subordinate,
            (true, true) => ClockOrdering::Concurrent,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn sum_generations(&self) -> u64 {
        self.entries.values().sum()
    }
}

/// Generic wrapper that tags any payload entity with vector clock and actor metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedDocument<T> {
    pub data: T,
    pub clock: VectorClock,
    pub actor_id: String,
    pub modified_at: u64,
}

impl<T> VersionedDocument<T> {
    pub fn new(data: T, actor_id: impl Into<String>) -> Self {
        let actor = actor_id.into();
        let mut clock = VectorClock::new();
        clock.increment(&actor);
        let now = current_unix_timestamp();
        Self {
            data,
            clock,
            actor_id: actor,
            modified_at: now,
        }
    }

    pub fn with_clock(data: T, actor_id: impl Into<String>, clock: VectorClock) -> Self {
        let now = current_unix_timestamp();
        Self {
            data,
            clock,
            actor_id: actor_id.into(),
            modified_at: now,
        }
    }

    pub fn update(&mut self, new_data: T, actor_id: &str) {
        self.data = new_data;
        self.actor_id = actor_id.to_string();
        self.clock.increment(actor_id);
        self.modified_at = current_unix_timestamp();
    }

    pub fn compare(&self, other: &VersionedDocument<T>) -> ClockOrdering {
        self.clock.compare(&other.clock)
    }

    pub fn into_inner(self) -> T {
        self.data
    }
    pub fn data(&self) -> &T {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }
    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }
    pub fn modified_at(&self) -> u64 {
        self.modified_at
    }
}

/// Detailed diff chunk produced during 3-way synchronization analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffChunk {
    /// Field modified or added only in local copy.
    LocalOnly { key: String, local: String },
    /// Field modified or added only in remote copy.
    RemoteOnly { key: String, remote: String },
    /// Conflicting edits between local and remote relative to base.
    Conflict {
        key: String,
        base: Option<String>,
        local: String,
        remote: String,
    },
    /// Clean resolution applied automatically.
    Resolved { key: String, value: String },
}

impl DiffChunk {
    pub fn key(&self) -> &str {
        match self {
            DiffChunk::LocalOnly { key, .. }
            | DiffChunk::RemoteOnly { key, .. }
            | DiffChunk::Conflict { key, .. }
            | DiffChunk::Resolved { key, .. } => key,
        }
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, DiffChunk::Conflict { .. })
    }
}

/// Description of a specific merge conflict encountered during 3-way merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeConflict {
    pub key: String,
    pub base: Option<String>,
    pub local: String,
    pub remote: String,
}

/// Result of 3-way YAML merge execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeResult {
    pub merged_yaml: String,
    pub is_clean: bool,
    pub conflicts: Vec<MergeConflict>,
    pub chunks: Vec<DiffChunk>,
}

impl MergeResult {
    pub fn is_clean(&self) -> bool {
        self.is_clean
    }
    pub fn has_conflicts(&self) -> bool {
        !self.is_clean
    }
    pub fn merged_yaml(&self) -> &str {
        &self.merged_yaml
    }
    pub fn merged_content(&self) -> &str {
        &self.merged_yaml
    }
}

struct MergeContext {
    conflicts: Vec<MergeConflict>,
    chunks: Vec<DiffChunk>,
    is_clean: bool,
}

fn value_to_string_lossless(val: &serde_yaml_ng::Value) -> String {
    match val {
        serde_yaml_ng::Value::String(s) => s.clone(),
        serde_yaml_ng::Value::Bool(b) => b.to_string(),
        serde_yaml_ng::Value::Number(n) => n.to_string(),
        serde_yaml_ng::Value::Null => "null".to_string(),
        _ => serde_yaml_ng::to_string(val)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn merge_recursive(
    base: Option<&serde_yaml_ng::Value>,
    local: Option<&serde_yaml_ng::Value>,
    remote: Option<&serde_yaml_ng::Value>,
    path: &str,
    ctx: &mut MergeContext,
) -> Option<serde_yaml_ng::Value> {
    match (base, local, remote) {
        (None, None, None) => None,
        (_, Some(l), Some(r)) if l == r => {
            if base != Some(l) && !path.is_empty() {
                ctx.chunks.push(DiffChunk::Resolved {
                    key: path.to_string(),
                    value: value_to_string_lossless(l),
                });
            }
            Some(l.clone())
        }
        (_, None, None) => None,
        (
            b,
            Some(serde_yaml_ng::Value::Mapping(l_map)),
            Some(serde_yaml_ng::Value::Mapping(r_map)),
        ) => {
            let b_map = match b {
                Some(serde_yaml_ng::Value::Mapping(bm)) => Some(bm),
                _ => None,
            };
            let mut all_keys: Vec<serde_yaml_ng::Value> = Vec::new();
            let mut seen = HashSet::new();
            if let Some(bm) = b_map {
                for k in bm.keys() {
                    if seen.insert(k.clone()) {
                        all_keys.push(k.clone());
                    }
                }
            }
            for k in l_map.keys().chain(r_map.keys()) {
                if seen.insert(k.clone()) {
                    all_keys.push(k.clone());
                }
            }
            let mut merged_map = serde_yaml_ng::Mapping::new();
            for k in all_keys {
                let k_str = match &k {
                    serde_yaml_ng::Value::String(s) => s.clone(),
                    _ => value_to_string_lossless(&k),
                };
                let subpath = if path.is_empty() {
                    k_str
                } else {
                    format!("{path}.{k_str}")
                };
                let sub_b = b_map.and_then(|bm| bm.get(&k));
                let sub_l = l_map.get(&k);
                let sub_r = r_map.get(&k);
                if let Some(merged_val) = merge_recursive(sub_b, sub_l, sub_r, &subpath, ctx) {
                    merged_map.insert(k, merged_val);
                }
            }
            Some(serde_yaml_ng::Value::Mapping(merged_map))
        }
        (b, Some(l), Some(r)) if b == Some(l) => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::RemoteOnly {
                    key: path.to_string(),
                    remote: value_to_string_lossless(r),
                });
                ctx.chunks.push(DiffChunk::Resolved {
                    key: path.to_string(),
                    value: value_to_string_lossless(r),
                });
            }
            Some(r.clone())
        }
        (None, None, Some(r)) => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::RemoteOnly {
                    key: path.to_string(),
                    remote: value_to_string_lossless(r),
                });
                ctx.chunks.push(DiffChunk::Resolved {
                    key: path.to_string(),
                    value: value_to_string_lossless(r),
                });
            }
            Some(r.clone())
        }
        (Some(b), Some(l), None) if b == l => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::RemoteOnly {
                    key: path.to_string(),
                    remote: "<deleted>".to_string(),
                });
            }
            None
        }
        (b, Some(l), Some(r)) if b == Some(r) => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::LocalOnly {
                    key: path.to_string(),
                    local: value_to_string_lossless(l),
                });
                ctx.chunks.push(DiffChunk::Resolved {
                    key: path.to_string(),
                    value: value_to_string_lossless(l),
                });
            }
            Some(l.clone())
        }
        (None, Some(l), None) => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::LocalOnly {
                    key: path.to_string(),
                    local: value_to_string_lossless(l),
                });
                ctx.chunks.push(DiffChunk::Resolved {
                    key: path.to_string(),
                    value: value_to_string_lossless(l),
                });
            }
            Some(l.clone())
        }
        (Some(b), None, Some(r)) if b == r => {
            if !path.is_empty() {
                ctx.chunks.push(DiffChunk::LocalOnly {
                    key: path.to_string(),
                    local: "<deleted>".to_string(),
                });
            }
            None
        }
        (
            Some(serde_yaml_ng::Value::Sequence(b_seq)),
            Some(serde_yaml_ng::Value::Sequence(l_seq)),
            Some(serde_yaml_ng::Value::Sequence(r_seq)),
        ) => {
            let b_len = b_seq.len();
            let l_starts_with_base = l_seq.len() >= b_len && &l_seq[..b_len] == b_seq.as_slice();
            let r_starts_with_base = r_seq.len() >= b_len && &r_seq[..b_len] == b_seq.as_slice();
            if l_starts_with_base && r_starts_with_base {
                let mut merged_seq = b_seq.clone();
                let mut existing_items: HashSet<String> =
                    b_seq.iter().map(value_to_string_lossless).collect();
                for item in &l_seq[b_len..] {
                    let str_repr = value_to_string_lossless(item);
                    if existing_items.insert(str_repr) {
                        merged_seq.push(item.clone());
                    }
                }
                for item in &r_seq[b_len..] {
                    let str_repr = value_to_string_lossless(item);
                    if existing_items.insert(str_repr) {
                        merged_seq.push(item.clone());
                    }
                }
                if !path.is_empty() {
                    ctx.chunks.push(DiffChunk::Resolved {
                        key: path.to_string(),
                        value: value_to_string_lossless(&serde_yaml_ng::Value::Sequence(
                            merged_seq.clone(),
                        )),
                    });
                }
                Some(serde_yaml_ng::Value::Sequence(merged_seq))
            } else {
                ctx.is_clean = false;
                let b_str = Some(value_to_string_lossless(&serde_yaml_ng::Value::Sequence(
                    b_seq.clone(),
                )));
                let l_str =
                    value_to_string_lossless(&serde_yaml_ng::Value::Sequence(l_seq.clone()));
                let r_str =
                    value_to_string_lossless(&serde_yaml_ng::Value::Sequence(r_seq.clone()));
                ctx.conflicts.push(MergeConflict {
                    key: path.to_string(),
                    base: b_str.clone(),
                    local: l_str.clone(),
                    remote: r_str.clone(),
                });
                ctx.chunks.push(DiffChunk::Conflict {
                    key: path.to_string(),
                    base: b_str,
                    local: l_str,
                    remote: r_str,
                });
                Some(serde_yaml_ng::Value::Sequence(l_seq.clone()))
            }
        }
        (b, l, r) => {
            ctx.is_clean = false;
            let b_str = b.map(value_to_string_lossless);
            let l_str = l
                .map(value_to_string_lossless)
                .unwrap_or_else(|| "<deleted>".to_string());
            let r_str = r
                .map(value_to_string_lossless)
                .unwrap_or_else(|| "<deleted>".to_string());
            ctx.conflicts.push(MergeConflict {
                key: path.to_string(),
                base: b_str.clone(),
                local: l_str.clone(),
                remote: r_str.clone(),
            });
            ctx.chunks.push(DiffChunk::Conflict {
                key: path.to_string(),
                base: b_str,
                local: l_str,
                remote: r_str,
            });
            l.cloned().or_else(|| r.cloned())
        }
    }
}

/// Perform 3-way merge on YAML documents (base, local, remote).
pub fn merge_3way(base: &str, local: &str, remote: &str) -> MergeResult {
    let base_val: Option<serde_yaml_ng::Value> = if base.trim().is_empty() {
        Some(serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()))
    } else {
        serde_yaml_ng::from_str(base).ok()
    };

    let local_val: Result<serde_yaml_ng::Value, _> = if local.trim().is_empty() {
        Ok(serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()))
    } else {
        serde_yaml_ng::from_str(local)
    };

    let remote_val: Result<serde_yaml_ng::Value, _> = if remote.trim().is_empty() {
        Ok(serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()))
    } else {
        serde_yaml_ng::from_str(remote)
    };

    let mut ctx = MergeContext {
        conflicts: Vec::new(),
        chunks: Vec::new(),
        is_clean: true,
    };

    let (l_v, r_v) = match (local_val, remote_val) {
        (Ok(l), Ok(r)) => (l, r),
        (Err(err_l), Ok(_)) => {
            let conflict = MergeConflict {
                key: "$root".to_string(),
                base: Some(base.to_string()),
                local: format!("parse error: {err_l}"),
                remote: remote.to_string(),
            };
            return MergeResult {
                merged_yaml: local.to_string(),
                is_clean: false,
                conflicts: vec![conflict.clone()],
                chunks: vec![DiffChunk::Conflict {
                    key: conflict.key,
                    base: conflict.base,
                    local: conflict.local,
                    remote: conflict.remote,
                }],
            };
        }
        (Ok(_), Err(err_r)) => {
            let conflict = MergeConflict {
                key: "$root".to_string(),
                base: Some(base.to_string()),
                local: local.to_string(),
                remote: format!("parse error: {err_r}"),
            };
            return MergeResult {
                merged_yaml: local.to_string(),
                is_clean: false,
                conflicts: vec![conflict.clone()],
                chunks: vec![DiffChunk::Conflict {
                    key: conflict.key,
                    base: conflict.base,
                    local: conflict.local,
                    remote: conflict.remote,
                }],
            };
        }
        (Err(err_l), Err(err_r)) => {
            let conflict = MergeConflict {
                key: "$root".to_string(),
                base: Some(base.to_string()),
                local: format!("parse error: {err_l}"),
                remote: format!("parse error: {err_r}"),
            };
            return MergeResult {
                merged_yaml: local.to_string(),
                is_clean: false,
                conflicts: vec![conflict.clone()],
                chunks: vec![DiffChunk::Conflict {
                    key: conflict.key,
                    base: conflict.base,
                    local: conflict.local,
                    remote: conflict.remote,
                }],
            };
        }
    };

    let merged_value = merge_recursive(base_val.as_ref(), Some(&l_v), Some(&r_v), "", &mut ctx);

    let merged_yaml = match merged_value {
        Some(val) => {
            if let serde_yaml_ng::Value::Mapping(ref m) = val
                && m.is_empty()
                && local.trim().is_empty()
                && remote.trim().is_empty()
            {
                String::new()
            } else {
                serde_yaml_ng::to_string(&val).unwrap_or_default()
            }
        }
        None => String::new(),
    };

    MergeResult {
        merged_yaml,
        is_clean: ctx.is_clean,
        conflicts: ctx.conflicts,
        chunks: ctx.chunks,
    }
}

/// Scope of configurations included during synchronization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SyncScope {
    #[default]
    Full,
    SubscriptionsOnly,
    CustomRulesOnly,
    SelectedProfiles(Vec<String>),
}

/// P2P quick pairing information for LAN direct transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct P2pPairingPayload {
    pub pairing_code: String,
    pub endpoint: String,
    pub cert_fingerprint: String,
    pub expires_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default)]
    pub sync_scope: SyncScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PairingError {
    #[error("pairing token or code has expired")]
    Expired,
    #[error("invalid pairing code")]
    InvalidCode,
    #[error("invalid pairing token")]
    InvalidToken,
    #[error("invalid pairing URI: {0}")]
    InvalidUri(String),
    #[error("missing required field in pairing payload: {0}")]
    MissingField(String),
    #[error("malformed endpoint: {0}")]
    InvalidEndpoint(String),
}

pub struct P2pPairingHelper;

impl P2pPairingHelper {
    /// Generate a 6-digit numeric pairing code.
    pub fn generate_pairing_code() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        format!("{:06}", nanos % 1_000_000)
    }

    /// Generate an HMAC / SHA256 sync authentication token.
    pub fn generate_token(secret: &str, device_id: &str, expires_at: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.update(b":");
        hasher.update(device_id.as_bytes());
        hasher.update(b":");
        hasher.update(expires_at.to_string().as_bytes());
        let result = hasher.finalize();
        let mut hex_str = String::with_capacity(result.len() * 2);
        for byte in result {
            use std::fmt::Write;
            let _ = write!(hex_str, "{:02x}", byte);
        }
        hex_str
    }

    /// Create a full pairing payload with expiration and optional auth token.
    pub fn create_payload(
        endpoint: impl Into<String>,
        cert_fingerprint: impl Into<String>,
        ttl_secs: u64,
        device_id: Option<String>,
        sync_scope: SyncScope,
        secret: Option<&str>,
    ) -> P2pPairingPayload {
        let code = Self::generate_pairing_code();
        let now = current_unix_timestamp();
        let expires_at = now.saturating_add(ttl_secs);
        let token = if let (Some(sec), Some(dev)) = (secret, device_id.as_deref()) {
            Some(Self::generate_token(sec, dev, expires_at))
        } else {
            None
        };

        P2pPairingPayload {
            pairing_code: code,
            endpoint: endpoint.into(),
            cert_fingerprint: cert_fingerprint.into(),
            expires_at,
            device_id,
            sync_scope,
            token,
        }
    }

    /// Format pairing payload into a URI scheme string (`infiltrator-p2p://...`).
    pub fn format_pairing_uri(payload: &P2pPairingPayload) -> String {
        let mut uri = format!(
            "infiltrator-p2p://{}?code={}&fp={}&exp={}",
            payload.endpoint, payload.pairing_code, payload.cert_fingerprint, payload.expires_at
        );
        if let Some(dev) = &payload.device_id {
            uri.push_str("&dev=");
            uri.push_str(dev);
        }
        if let Some(tok) = &payload.token {
            uri.push_str("&token=");
            uri.push_str(tok);
        }
        match &payload.sync_scope {
            SyncScope::Full => {}
            SyncScope::SubscriptionsOnly => uri.push_str("&scope=subs"),
            SyncScope::CustomRulesOnly => uri.push_str("&scope=rules"),
            SyncScope::SelectedProfiles(profiles) => {
                uri.push_str("&scope=profiles:");
                uri.push_str(&profiles.join(","));
            }
        }
        uri
    }

    /// Parse an `infiltrator-p2p://...` URI into a `P2pPairingPayload`.
    pub fn parse_pairing_uri(uri: &str) -> Result<P2pPairingPayload, PairingError> {
        let (scheme, rest) = uri
            .split_once("://")
            .ok_or_else(|| PairingError::InvalidUri("missing scheme delimiter '://'".into()))?;
        if scheme != "infiltrator-p2p" {
            return Err(PairingError::InvalidUri(format!(
                "expected scheme 'infiltrator-p2p', found '{scheme}'"
            )));
        }
        let (endpoint, query) = match rest.split_once('?') {
            Some((ep, q)) => (ep, Some(q)),
            None => (rest, None),
        };
        if endpoint.is_empty() {
            return Err(PairingError::InvalidEndpoint("empty endpoint".into()));
        }

        let mut pairing_code = None;
        let mut cert_fingerprint = None;
        let mut expires_at = None;
        let mut device_id = None;
        let mut token = None;
        let mut sync_scope = SyncScope::Full;

        if let Some(query_str) = query {
            for pair in query_str.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let (k, v) = match pair.split_once('=') {
                    Some((k, v)) => (k, v),
                    None => (pair, ""),
                };
                match k {
                    "code" => pairing_code = Some(v.to_string()),
                    "fp" | "cert_fingerprint" => cert_fingerprint = Some(v.to_string()),
                    "exp" | "expires_at" => {
                        let exp_val = v.parse::<u64>().map_err(|_| {
                            PairingError::InvalidUri(format!("invalid expires_at value: {v}"))
                        })?;
                        expires_at = Some(exp_val);
                    }
                    "dev" | "device_id" => device_id = Some(v.to_string()),
                    "token" => token = Some(v.to_string()),
                    "scope" => {
                        if v == "subs" {
                            sync_scope = SyncScope::SubscriptionsOnly;
                        } else if v == "rules" {
                            sync_scope = SyncScope::CustomRulesOnly;
                        } else if let Some(prof_list) = v.strip_prefix("profiles:") {
                            let profiles = prof_list
                                .split(',')
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .collect();
                            sync_scope = SyncScope::SelectedProfiles(profiles);
                        } else {
                            sync_scope = SyncScope::Full;
                        }
                    }
                    _ => {}
                }
            }
        }

        let pairing_code = pairing_code.ok_or_else(|| PairingError::MissingField("code".into()))?;
        let cert_fingerprint =
            cert_fingerprint.ok_or_else(|| PairingError::MissingField("fp".into()))?;
        let expires_at = expires_at.unwrap_or(0);

        Ok(P2pPairingPayload {
            pairing_code,
            endpoint: endpoint.to_string(),
            cert_fingerprint,
            expires_at,
            device_id,
            sync_scope,
            token,
        })
    }

    /// Verify a pairing payload with the user-entered pairing code and current time.
    pub fn verify_pairing_code(
        payload: &P2pPairingPayload,
        entered_code: &str,
        current_time: u64,
    ) -> Result<(), PairingError> {
        if payload.expires_at > 0 && current_time > payload.expires_at {
            return Err(PairingError::Expired);
        }
        if payload.pairing_code != entered_code {
            return Err(PairingError::InvalidCode);
        }
        if payload.endpoint.trim().is_empty() {
            return Err(PairingError::InvalidEndpoint("empty endpoint".into()));
        }
        if payload.cert_fingerprint.trim().is_empty() {
            return Err(PairingError::MissingField("cert_fingerprint".into()));
        }
        Ok(())
    }

    /// Verify an authentication token against the shared secret, device ID, and timestamp.
    pub fn verify_token(
        token: &str,
        secret: &str,
        device_id: &str,
        current_time: u64,
        expires_at: u64,
    ) -> Result<(), PairingError> {
        if expires_at > 0 && current_time > expires_at {
            return Err(PairingError::Expired);
        }
        let expected = Self::generate_token(secret, device_id, expires_at);
        if token != expected {
            return Err(PairingError::InvalidToken);
        }
        Ok(())
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
