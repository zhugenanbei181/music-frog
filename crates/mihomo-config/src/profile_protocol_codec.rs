//! Mihomo-owned custom protocol node and CA trust-anchor YAML operations.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::protocol_fidelity::{
    CodecAudit, NodeCodecFormat, ProtocolDraft, ProtocolFamily,
};
use infiltrator_contract::protocol_params::EchParams;
use infiltrator_contract::protocol_params_ext::TransportParams;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_domain::profile_converter::ProxyNodeItem;
use infiltrator_domain::protocol_node_params::node_from_draft;
use infiltrator_domain::protocol_node_projection::draft_from_node;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationReadout, ProfileProtocolNodeMutationReadback,
};
use serde_yaml_ng::{Mapping, Value};
use std::str::from_utf8;

pub(crate) fn read_protocol_nodes(raw_bytes: &[u8]) -> Result<Vec<ProtocolDraft>, PortError> {
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    protocol_drafts_from_document(&document)
}

pub(crate) fn read_trust_anchors(raw_bytes: &[u8]) -> Result<TlsTrustParams, PortError> {
    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    trust_anchors_from_document(&document)
}

pub(crate) fn upsert_protocol_node(
    raw_bytes: &[u8],
    draft: ProtocolDraft,
) -> Result<ProfileConfigurationMutation, PortError> {
    if let Some(issue) = draft.report().issues.first() {
        return Err(invalid_input(issue.message.clone()));
    }

    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let mut document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    if !document.is_mapping() {
        return Err(invalid_input("profile YAML must be a top-level mapping"));
    }
    let original_sections = sections_without_proxies(&document);
    let node_value =
        serde_yaml_ng::to_value(node_from_draft(&draft)).map_err(configuration_error)?;
    let mut replaced_existing = false;
    let mut unknown_fields = Vec::new();
    let node_count = {
        let proxies = proxies_sequence_mut(&mut document)?;
        let existing_index = proxies
            .iter()
            .position(|entry| entry.get("name").and_then(Value::as_str) == Some(draft.name.trim()));
        if let Some(index) = existing_index {
            replaced_existing = true;
            let previous = proxies.get(index).cloned().unwrap_or(Value::Null);
            proxies[index] = merge_preserving_unknown(
                &previous,
                &node_value,
                draft.family(),
                &mut unknown_fields,
            );
        } else {
            let mut next = Vec::with_capacity(proxies.len() + 1);
            next.push(node_value);
            next.extend(proxies.iter().cloned());
            *proxies = next;
        }
        proxies.len()
    };

    unknown_fields.sort();
    unknown_fields.dedup();
    let document_bytes = serde_yaml_ng::to_string(&document)
        .map_err(configuration_error)?
        .into_bytes();

    let audit_document: Value =
        serde_yaml_ng::from_slice(&document_bytes).map_err(configuration_error)?;
    let structure_preserved = original_sections == sections_without_proxies(&audit_document);
    let audit = CodecAudit {
        source_format: NodeCodecFormat::Uri,
        target_format: NodeCodecFormat::ClashYaml,
        node_count,
        unknown_fields,
        structure_preserved,
        lossless: structure_preserved,
        detail: if replaced_existing {
            format!("replaced node `{}` in place", draft.name.trim())
        } else {
            format!(
                "inserted node `{}` at the top of proxies",
                draft.name.trim()
            )
        },
    };
    let readback = read_protocol_nodes(&document_bytes)?;
    if !readback
        .iter()
        .any(|returned| returned.name.trim() == draft.name.trim())
    {
        return Err(invalid_state(
            "Mihomo protocol node codec did not read back the edited node",
        ));
    }
    Ok(ProfileConfigurationMutation {
        document_bytes,
        readback: ProfileConfigurationReadout::ProtocolNodeMutation(
            ProfileProtocolNodeMutationReadback {
                nodes: readback,
                audit,
                replaced_existing,
            },
        ),
    })
}

pub(crate) fn apply_trust_anchors(
    raw_bytes: &[u8],
    params: TlsTrustParams,
) -> Result<ProfileConfigurationMutation, PortError> {
    if !params.ca_str.trim().is_empty() {
        return Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            "Mihomo v1.19.18 has no profile-level inline PEM trust-anchor carrier; retain `ca-str` on the protocol draft",
        ));
    }
    if !params.fingerprint.trim().is_empty() {
        return Err(invalid_input(
            "peer certificate fingerprints belong to a protocol node, not the profile trust-anchor list",
        ));
    }

    let content = from_utf8(raw_bytes).map_err(configuration_error)?;
    let mut document: Value = serde_yaml_ng::from_str(content).map_err(configuration_error)?;
    if !document.is_mapping() {
        return Err(invalid_input("profile YAML must be a top-level mapping"));
    }
    let changed = add_trust_anchor_path(&mut document, &params.ca_path)?.is_some();
    let document_bytes = if changed {
        serde_yaml_ng::to_string(&document)
            .map_err(configuration_error)?
            .into_bytes()
    } else {
        raw_bytes.to_vec()
    };
    let readback = read_trust_anchors(&document_bytes)?;
    Ok(ProfileConfigurationMutation {
        document_bytes,
        readback: ProfileConfigurationReadout::TrustAnchors(readback),
    })
}

fn protocol_drafts_from_document(document: &Value) -> Result<Vec<ProtocolDraft>, PortError> {
    if !document.is_mapping() {
        return Err(invalid_input("profile YAML must be a top-level mapping"));
    }
    let entries = match document.get("proxies") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Sequence(entries)) => entries,
        Some(_) => return Err(invalid_input("`proxies` must be a list of node mappings")),
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let node: ProxyNodeItem =
                serde_yaml_ng::from_value(entry.clone()).map_err(|error| {
                    configuration_error(format!("failed to decode proxies[{index}]: {error}"))
                })?;
            let mut draft = draft_from_node(&node);
            draft.preserved_fields = node
                .extra
                .keys()
                .filter(|key| !is_typed_extra_key_for(key, draft.family()))
                .cloned()
                .collect();
            draft.preserved_fields.sort();
            Ok(draft)
        })
        .collect()
}

fn trust_anchors_from_document(document: &Value) -> Result<TlsTrustParams, PortError> {
    if !document.is_mapping() {
        return Err(invalid_input("profile YAML must be a top-level mapping"));
    }
    let Some(tls) = document.get("tls") else {
        return Ok(TlsTrustParams::default());
    };
    let tls = tls
        .as_mapping()
        .ok_or_else(|| invalid_input("`tls` must be a mapping"))?;
    let list_key = Value::String("custom-certifactes".to_owned());
    let Some(list) = tls.get(&list_key) else {
        return Ok(TlsTrustParams::default());
    };
    let entries = list
        .as_sequence()
        .ok_or_else(|| invalid_input("`tls.custom-certifactes` must be a list of paths"))?;
    let ca_path = entries
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|entry| !entry.is_empty())
        .unwrap_or_default()
        .to_owned();
    Ok(TlsTrustParams {
        ca_path,
        ..TlsTrustParams::default()
    })
}

fn add_trust_anchor_path(document: &mut Value, ca_path: &str) -> Result<Option<String>, PortError> {
    let ca_path = ca_path.trim();
    if ca_path.is_empty() {
        return Ok(None);
    }
    if ca_path.contains('\n') || ca_path.contains('\r') {
        return Err(invalid_input("CA trust-anchor path must be a single line"));
    }
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| invalid_input("profile YAML must be a top-level mapping"))?;
    let tls_key = Value::String("tls".to_owned());
    let tls_entry = mapping
        .entry(tls_key)
        .or_insert_with(|| Value::Mapping(Mapping::new()));
    let tls = tls_entry
        .as_mapping_mut()
        .ok_or_else(|| invalid_input("`tls` must be a mapping"))?;
    let list_key = Value::String("custom-certifactes".to_owned());
    let list = tls
        .entry(list_key)
        .or_insert_with(|| Value::Sequence(Vec::new()));
    let entries = list
        .as_sequence_mut()
        .ok_or_else(|| invalid_input("`tls.custom-certifactes` must be a list of paths"))?;
    if entries.iter().any(|entry| entry.as_str() == Some(ca_path)) {
        return Ok(None);
    }
    let mut next = Vec::with_capacity(entries.len() + 1);
    next.push(Value::String(ca_path.to_owned()));
    next.extend(entries.iter().cloned());
    *entries = next;
    Ok(Some(ca_path.to_owned()))
}

fn proxies_sequence_mut(document: &mut Value) -> Result<&mut Vec<Value>, PortError> {
    let mapping = document
        .as_mapping_mut()
        .ok_or_else(|| invalid_input("profile YAML must be a top-level mapping"))?;
    let key = Value::String("proxies".to_owned());
    match mapping.get(&key) {
        None | Some(Value::Null) => {
            mapping.insert(key.clone(), Value::Sequence(Vec::new()));
        }
        Some(Value::Sequence(_)) => {}
        Some(_) => return Err(invalid_input("`proxies` must be a list of node mappings")),
    }
    match mapping.get_mut(&key) {
        Some(Value::Sequence(entries)) => Ok(entries),
        _ => Err(invalid_input("`proxies` could not be created")),
    }
}

fn merge_preserving_unknown(
    previous: &Value,
    node: &Value,
    family: ProtocolFamily,
    unknown_fields: &mut Vec<String>,
) -> Value {
    let (Some(previous), Some(node)) = (previous.as_mapping(), node.as_mapping()) else {
        return node.clone();
    };
    let owned = owned_keys(family);
    let mut merged = Mapping::new();
    for (key, value) in previous {
        let Some(name) = key.as_str() else {
            continue;
        };
        if !owned.contains(&name) {
            unknown_fields.push(name.to_owned());
            merged.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in node {
        let nested = key
            .as_str()
            .is_some_and(|name| DRAFT_NESTED_KEYS.contains(&name) && owned.contains(&name));
        if nested {
            let previous_sub = previous.get(key).and_then(Value::as_mapping);
            let Some(node_sub) = value.as_mapping() else {
                merged.insert(key.clone(), value.clone());
                continue;
            };
            let path = key.as_str().unwrap_or_default().to_owned();
            if let Some(previous_sub) = previous_sub {
                merged.insert(
                    key.clone(),
                    Value::Mapping(merge_nested(previous_sub, node_sub, &path, unknown_fields)),
                );
            } else {
                merged.insert(key.clone(), value.clone());
            }
        } else {
            merged.insert(key.clone(), value.clone());
        }
    }
    Value::Mapping(merged)
}

fn merge_nested(
    previous: &Mapping,
    node: &Mapping,
    path: &str,
    unknown: &mut Vec<String>,
) -> Mapping {
    let mut merged = Mapping::new();
    for (key, value) in node {
        let sub_path = key
            .as_str()
            .map(|name| format!("{path}.{name}"))
            .unwrap_or_else(|| path.to_owned());
        match (
            previous.get(key).and_then(Value::as_mapping),
            value.as_mapping(),
        ) {
            (Some(previous), Some(node)) => {
                merged.insert(
                    key.clone(),
                    Value::Mapping(merge_nested(previous, node, &sub_path, unknown)),
                );
            }
            _ => {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    for (key, value) in previous {
        if node.contains_key(key) {
            continue;
        }
        let sub_path = key
            .as_str()
            .map(|name| format!("{path}.{name}"))
            .unwrap_or_else(|| path.to_owned());
        unknown.push(sub_path);
        merged.insert(key.clone(), value.clone());
    }
    merged
}

fn sections_without_proxies(document: &Value) -> Value {
    let mut clone = document.clone();
    if let Some(mapping) = clone.as_mapping_mut() {
        mapping.remove(Value::String("proxies".to_owned()));
    }
    clone
}

fn owned_keys(family: ProtocolFamily) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = DRAFT_OWNED_KEYS.to_vec();
    if EchParams::supported_by(family) {
        keys.push("ech-opts");
    }
    match family {
        ProtocolFamily::Tuic => keys.extend([
            "congestion-controller",
            "udp-relay-mode",
            "reduce-rtt",
            "heartbeat-interval",
            "request-timeout",
            "disable-sni",
            "recv-window-conn",
            "recv-window",
        ]),
        ProtocolFamily::Hysteria2 => keys.extend([
            "ports",
            "hop-interval",
            "obfs",
            "obfs-password",
            "up",
            "down",
            "cwnd",
            "udp-mtu",
        ]),
        ProtocolFamily::WireGuard => keys.extend([
            "private-key",
            "public-key",
            "pre-shared-key",
            "reserved",
            "ip",
            "ipv6",
            "mtu",
            "remote-dns-resolve",
            "workers",
            "persistent-keepalive",
            "allowed-ips",
            "amnezia-wg-option",
            "dns",
        ]),
        ProtocolFamily::Shadowsocks => keys.extend(["plugin", "plugin-opts"]),
        ProtocolFamily::Ssh => keys.extend([
            "username",
            "private-key",
            "private-key-passphrase",
            "host-key-algorithms",
        ]),
        ProtocolFamily::Anytls => keys.extend([
            "idle-session-timeout",
            "idle-session-check-interval",
            "min-idle-session",
        ]),
        ProtocolFamily::Trojan => keys.push("ss-opts"),
        _ => {}
    }
    if TransportParams::supported_by(family) {
        keys.extend([
            "network",
            "packet-encoding",
            "ws-opts",
            "grpc-opts",
            "h2-opts",
            "http-opts",
            "xhttp-opts",
        ]);
    }
    keys
}

const DRAFT_OWNED_KEYS: [&str; 19] = [
    "name",
    "type",
    "server",
    "port",
    "password",
    "uuid",
    "cipher",
    "flow",
    "servername",
    "sni",
    "tls",
    "skip-cert-verify",
    "alpn",
    "client-fingerprint",
    "smux",
    "dialer-proxy",
    "fingerprint",
    "ca",
    "ca-str",
];

const DRAFT_NESTED_KEYS: [&str; 10] = [
    "reality-opts",
    "smux",
    "ech-opts",
    "ss-opts",
    "plugin-opts",
    "ws-opts",
    "grpc-opts",
    "h2-opts",
    "http-opts",
    "xhttp-opts",
];

const TYPED_EXTRA_KEYS: [&str; 10] = [
    "ech-opts",
    "ss-opts",
    "udp-mtu",
    "dns",
    "xhttp-opts",
    "idle-session-timeout",
    "idle-session-check-interval",
    "min-idle-session",
    "ca",
    "ca-str",
];

fn is_typed_extra_key_for(key: &str, family: ProtocolFamily) -> bool {
    if key == "plugin-opts" {
        return family == ProtocolFamily::Shadowsocks;
    }
    if !TYPED_EXTRA_KEYS.contains(&key) {
        return false;
    }
    match key {
        "ech-opts" => EchParams::supported_by(family),
        "ss-opts" => family == ProtocolFamily::Trojan,
        "udp-mtu" => family == ProtocolFamily::Hysteria2,
        "dns" => family == ProtocolFamily::WireGuard,
        "xhttp-opts" => TransportParams::supported_by(family),
        "idle-session-timeout" | "idle-session-check-interval" | "min-idle-session" => {
            family == ProtocolFamily::Anytls
        }
        "ca" | "ca-str" => true,
        _ => false,
    }
}

fn invalid_input(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidInput, message.into(), false))
}

fn invalid_state(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::InvalidState, message.into(), false))
}

fn configuration_error(error: impl std::fmt::Display) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::Configuration,
        error.to_string(),
        false,
    ))
}

#[cfg(test)]
#[path = "profile_protocol_codec_tests.rs"]
mod profile_protocol_codec_tests;
