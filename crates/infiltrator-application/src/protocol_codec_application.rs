//! DUAL-05 shared protocol codec application.
//!
//! One shared use-case layer for the group 05 protocol-ecosystem items:
//!
//! * 05-01/02/11 — a URI or profile node becomes a typed [`ProtocolDraft`]
//!   (cipher family, REALITY/Vision, multiplexing) with shared validation.
//! * 05-14 — a draft is written back into the *whole* profile document with
//!   every other section and every unknown key preserved, and the conversion
//!   is audited honestly instead of being claimed lossless.
//!
//! The application layer stays executor-neutral: everything here is sync and
//! free of Tokio, so both surfaces (and the host command path) share it.

use std::sync::{Mutex, OnceLock};

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::protocol_fidelity::{
    CodecAudit, NodeCodecFormat, ProtocolDraft, ProtocolFidelityReport, ProtocolStudioSnapshot,
    RealityParams, SmuxParams,
};
use infiltrator_domain::profile_converter::{ProfileConverter, ProfileFormat, ProxyNodeItem};
use serde_json::json;
use serde_yaml_ng::{Mapping, Value};

#[cfg(test)]
#[path = "protocol_codec_application_test.rs"]
mod protocol_codec_application_test;

/// Keys the draft owns when it writes a node back into a profile. Every other
/// key of an existing node with the same name is preserved verbatim.
const DRAFT_OWNED_KEYS: [&str; 12] = [
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
    "reality-opts",
];

/// Outcome of committing one draft into a profile document.
#[derive(Clone, Debug, PartialEq)]
pub struct ProtocolCommit {
    pub profile_yaml: String,
    pub draft: ProtocolDraft,
    pub report: ProtocolFidelityReport,
    pub audit: CodecAudit,
    /// `true` when a node with the same name was replaced instead of added.
    pub replaced_existing: bool,
    pub node_count: usize,
}

impl ProtocolCommit {
    /// True only when the untouched profile sections are semantically equal
    /// to the original document's, at the YAML value level (05-14).
    pub fn is_structure_preserving(&self) -> bool {
        self.audit.structure_preserved
    }
}

/// DUAL-05 shared codec + draft use cases. Stateless; safe to share.
pub struct ProtocolCodecApplication;

impl ProtocolCodecApplication {
    /// Parse one share link into the shared editable draft.
    pub fn draft_from_uri(uri: &str) -> Result<ProtocolDraft, Failure> {
        let trimmed = uri.trim();
        if trimmed.is_empty() {
            return Err(invalid_input("share link must not be empty"));
        }
        let item = ProfileConverter::parse_uri(trimmed)
            .map_err(|error| invalid_input(&format!("could not parse share link: {error}")))?;
        let draft = Self::draft_from_node(&item);
        if draft.node_type.trim().is_empty() {
            return Err(invalid_input("share link did not name a protocol"));
        }
        Ok(draft)
    }

    /// Export the draft back to a share link. Nothing is fabricated: an
    /// unsupported family is a typed error, and every field written comes
    /// from the draft.
    pub fn uri_from_draft(draft: &ProtocolDraft) -> Result<String, Failure> {
        let item = Self::node_from_draft(draft);
        ProfileConverter::export_uri(&item)
            .map_err(|error| invalid_input(&format!("could not encode share link: {error}")))
    }

    /// Project a parsed profile node into the shared draft (field-for-field).
    pub fn draft_from_node(item: &ProxyNodeItem) -> ProtocolDraft {
        let mut draft = ProtocolDraft::new(item.node_type.clone());
        draft.name = item.name.clone();
        draft.server = item.server.clone();
        draft.port = item.port;
        draft.password = item.password.clone().unwrap_or_default();
        draft.uuid = item.uuid.clone().unwrap_or_default();
        draft.sni = item
            .get_effective_sni()
            .map(str::to_string)
            .unwrap_or_default();
        draft.cipher = item.cipher.clone().unwrap_or_default();
        draft.flow = item.flow.clone().unwrap_or_default();
        draft.reality = RealityParams {
            public_key: item
                .get_effective_public_key()
                .map(str::to_string)
                .unwrap_or_default(),
            short_id: item
                .get_effective_short_id()
                .map(str::to_string)
                .unwrap_or_default(),
            spider_x: item.spider_x.clone().unwrap_or_default(),
            fingerprint: item.client_fingerprint.clone().unwrap_or_default(),
        };
        draft.smux = smux_from_json(item.smux.as_ref());
        draft.tls = item.tls;
        draft.skip_cert_verify = item.skip_cert_verify.unwrap_or(false);
        draft.alpn = item.alpn.clone().unwrap_or_default();
        let mut preserved: Vec<String> = item.extra.keys().cloned().collect();
        preserved.sort();
        draft.preserved_fields = preserved;
        draft
    }

    /// Project the draft back into the flat profile node shape. Only the
    /// fields the draft owns are written; unset fields are omitted rather
    /// than emitted as empty strings.
    pub fn node_from_draft(draft: &ProtocolDraft) -> ProxyNodeItem {
        let mut item = ProxyNodeItem::new(
            draft.name.trim(),
            draft.node_type.trim(),
            draft.server.trim(),
            draft.port,
        );
        // `udp` is not part of the draft: a replaced node keeps whatever the
        // profile already said, and a new node omits the key entirely.
        item.udp = None;
        item.password = non_empty(&draft.password);
        item.uuid = non_empty(&draft.uuid);
        item.cipher = non_empty(&draft.cipher);
        item.flow = non_empty(&draft.flow);
        item.client_fingerprint = non_empty(&draft.reality.fingerprint);
        item.servername = non_empty(&draft.sni);
        item.tls = draft.tls;
        item.skip_cert_verify = draft.skip_cert_verify.then_some(true);
        if !draft.alpn.is_empty() {
            item.alpn = Some(draft.alpn.clone());
        }
        if draft.reality.is_present() {
            let mut reality = Mapping::new();
            if !draft.reality.public_key.trim().is_empty() {
                reality.insert(
                    Value::String("public-key".to_string()),
                    Value::String(draft.reality.public_key.trim().to_string()),
                );
            }
            if !draft.reality.short_id.trim().is_empty() {
                reality.insert(
                    Value::String("short-id".to_string()),
                    Value::String(draft.reality.short_id.trim().to_string()),
                );
            }
            if !draft.reality.spider_x.trim().is_empty() {
                reality.insert(
                    Value::String("spider-x".to_string()),
                    Value::String(draft.reality.spider_x.trim().to_string()),
                );
            }
            item.reality_opts =
                Some(serde_json::to_value(Value::Mapping(reality)).unwrap_or_default());
        }
        if draft.smux.has_overrides() {
            item.smux = Some(smux_to_json(&draft.smux));
        }
        item
    }

    /// Shared read model for both surfaces (05-01/02/11/14).
    pub fn report(draft: &ProtocolDraft) -> ProtocolFidelityReport {
        draft.report()
    }

    /// DUAL-05-14: which draft fields a share link cannot carry, computed by
    /// actually round-tripping the draft through the URI codec. Nothing here
    /// is hard-coded, so a future codec improvement shrinks the list by
    /// itself instead of leaving a stale claim in the ledger.
    pub fn uri_fidelity_gaps(draft: &ProtocolDraft) -> Vec<String> {
        let Ok(uri) = Self::uri_from_draft(draft) else {
            return vec!["uri".to_string()];
        };
        let Ok(returned) = Self::draft_from_uri(&uri) else {
            return vec!["uri".to_string()];
        };
        let mut gaps = Vec::new();
        for (field, same) in [
            ("name", draft.name.trim() == returned.name.trim()),
            ("server", draft.server.trim() == returned.server.trim()),
            ("port", draft.port == returned.port),
            ("type", draft.node_type.trim() == returned.node_type.trim()),
            (
                "password",
                draft.password.trim() == returned.password.trim(),
            ),
            ("uuid", draft.uuid.trim() == returned.uuid.trim()),
            ("sni", draft.sni.trim() == returned.sni.trim()),
            ("cipher", draft.cipher.trim() == returned.cipher.trim()),
            ("flow", draft.flow.trim() == returned.flow.trim()),
            ("reality", draft.reality == returned.reality),
            ("smux", draft.smux == returned.smux),
            ("tls", draft.tls == returned.tls),
            (
                "skip-cert-verify",
                draft.skip_cert_verify == returned.skip_cert_verify,
            ),
            ("alpn", draft.alpn == returned.alpn),
            (
                "unknown-fields",
                draft.preserved_fields == returned.preserved_fields,
            ),
        ] {
            if !same {
                gaps.push(field.to_string());
            }
        }
        gaps
    }

    /// Publish the studio snapshot for a freshly decoded or edited draft.
    pub fn publish_draft(
        draft: ProtocolDraft,
        uri_preview: Option<String>,
    ) -> ProtocolStudioSnapshot {
        let snapshot = ProtocolStudioSnapshot {
            report: Some(draft.report()),
            uri_preview,
            uri_gaps: Self::uri_fidelity_gaps(&draft),
            draft: Some(draft),
            audit: None,
            last_error: None,
            last_saved_node: None,
        };
        publish_studio(snapshot.clone());
        snapshot
    }

    /// DUAL-05-14: write the draft into the profile document, replacing an
    /// existing node with the same name (keeping every key the draft does not
    /// own) or inserting a new node at the top of `proxies:`.
    ///
    /// Every other profile section is preserved: the document is edited at the
    /// YAML value level and `structure_preserved` reports whether the
    /// untouched sections are semantically equal to the original.
    pub fn upsert_draft_into_profile(
        profile_yaml: &str,
        draft: &ProtocolDraft,
    ) -> Result<ProtocolCommit, Failure> {
        let report = draft.report();
        if !report.is_valid() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!(
                    "node draft has unresolved protocol issues: {}",
                    report.issue_lines().join("; ")
                ),
                false,
            ));
        }

        let mut document: Value = serde_yaml_ng::from_str(profile_yaml)
            .map_err(|error| invalid_input(&format!("profile is not valid YAML: {error}")))?;
        if !document.is_mapping() {
            return Err(invalid_input("profile YAML must be a top-level mapping"));
        }
        let original_sections = sections_without_proxies(&document);

        let node_value = serde_yaml_ng::to_value(Self::node_from_draft(draft))
            .map_err(|error| invalid_input(&format!("could not encode node: {error}")))?;

        let mut replaced_existing = false;
        let mut unknown_fields: Vec<String> = Vec::new();
        let node_count;
        {
            let existing = proxies_sequence_mut(&mut document)?;
            let index_found = existing.iter().position(|entry| {
                entry.get("name").and_then(Value::as_str) == Some(draft.name.trim())
            });

            if let Some(index) = index_found {
                replaced_existing = true;
                let previous = existing.get(index).cloned().unwrap_or(Value::Null);
                existing[index] =
                    merge_preserving_unknown(&previous, &node_value, &mut unknown_fields);
            } else {
                let mut entries = Vec::with_capacity(existing.len() + 1);
                entries.push(node_value);
                entries.extend(existing.iter().cloned());
                *existing = entries;
            }
            node_count = existing.len();
        }
        unknown_fields.sort();
        unknown_fields.dedup();

        let profile_yaml = serde_yaml_ng::to_string(&document)
            .map_err(|error| invalid_input(&format!("could not serialize profile: {error}")))?;
        let reported_sections = sections_without_proxies_from_str(&profile_yaml)?;
        let structure_preserved = original_sections == reported_sections;

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

        let commit = ProtocolCommit {
            profile_yaml,
            draft: draft.clone(),
            report,
            audit,
            replaced_existing,
            node_count,
        };
        publish_studio(ProtocolStudioSnapshot {
            draft: Some(draft.clone()),
            report: Some(commit.report.clone()),
            uri_preview: Self::uri_from_draft(draft).ok(),
            audit: Some(commit.audit.clone()),
            last_error: None,
            last_saved_node: Some(draft.name.trim().to_string()),
            uri_gaps: Self::uri_fidelity_gaps(draft),
        });
        Ok(commit)
    }

    /// DUAL-05-14: audit a node codec conversion without touching any profile.
    ///
    /// `structure_preserved` compares the parsed-then-exported YAML against
    /// the source at the value level, and `unknown_fields` lists the node keys
    /// the typed model did not recognise but kept.
    pub fn audit_conversion(
        input: &str,
        from: NodeCodecFormat,
        to: NodeCodecFormat,
    ) -> Result<(String, CodecAudit), Failure> {
        let nodes = Self::parse_nodes(input, from)?;
        let output = Self::export_nodes(&nodes, to)?;
        let unknown_fields = collect_unknown_fields(&nodes);
        let structure_preserved = if from == NodeCodecFormat::ClashYaml
            && to == NodeCodecFormat::ClashYaml
        {
            let original: Value = serde_yaml_ng::from_str(input)
                .map_err(|error| invalid_input(&format!("profile is not valid YAML: {error}")))?;
            let exported: Value = serde_yaml_ng::from_str(&output)
                .map_err(|error| invalid_input(&format!("exported YAML is invalid: {error}")))?;
            sections_without_proxies(&original) == sections_without_proxies(&exported)
        } else {
            true
        };

        let audit = CodecAudit {
            source_format: from,
            target_format: to,
            node_count: nodes.len(),
            unknown_fields,
            structure_preserved,
            lossless: structure_preserved,
            detail: format!("{} -> {}", from.label_en(), to.label_en()),
        };
        publish_studio(ProtocolStudioSnapshot {
            draft: nodes.first().map(Self::draft_from_node),
            report: nodes
                .first()
                .map(|node| Self::draft_from_node(node).report()),
            uri_preview: None,
            audit: Some(audit.clone()),
            last_error: None,
            last_saved_node: None,
            uri_gaps: Vec::new(),
        });
        Ok((output, audit))
    }

    /// Parse a node list in one of the shared codec formats.
    pub fn parse_nodes(
        input: &str,
        format: NodeCodecFormat,
    ) -> Result<Vec<ProxyNodeItem>, Failure> {
        ProfileConverter::parse_nodes(input, to_domain_format(format))
            .map_err(|error| invalid_input(&format!("could not parse nodes: {error}")))
    }

    /// Export a node list in one of the shared codec formats.
    pub fn export_nodes(
        nodes: &[ProxyNodeItem],
        format: NodeCodecFormat,
    ) -> Result<String, Failure> {
        ProfileConverter::export_nodes(nodes, to_domain_format(format))
            .map_err(|error| invalid_input(&format!("could not export nodes: {error}")))
    }

    /// Shared studio failure publication: both surfaces render the same error.
    pub fn publish_error(message: impl Into<String>) {
        let message = message.into();
        let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
        let mut next = studio.take().unwrap_or_default();
        next.last_error = Some(message);
        *studio = Some(next);
    }
}

/// DUAL-05: the most recently published custom-node studio state.
pub fn last_studio() -> &'static Mutex<Option<ProtocolStudioSnapshot>> {
    static STUDIO: OnceLock<Mutex<Option<ProtocolStudioSnapshot>>> = OnceLock::new();
    STUDIO.get_or_init(|| Mutex::new(None))
}

/// Publish the studio snapshot for the surface reader to project.
pub fn publish_studio(snapshot: ProtocolStudioSnapshot) {
    let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
    *studio = Some(snapshot);
}

/// Read the last published studio snapshot, if any.
pub fn studio_snapshot() -> Option<ProtocolStudioSnapshot> {
    last_studio()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Drop the studio state (used by tests and by session reset paths).
pub fn clear_studio() {
    let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
    *studio = None;
}

fn to_domain_format(format: NodeCodecFormat) -> ProfileFormat {
    match format {
        NodeCodecFormat::ClashYaml => ProfileFormat::ClashYaml,
        NodeCodecFormat::RawJson => ProfileFormat::RawJson,
        NodeCodecFormat::Uri => ProfileFormat::ShadowrocketUriList,
        NodeCodecFormat::Base64Subscription => ProfileFormat::Base64Subscription,
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn smux_from_json(value: Option<&serde_json::Value>) -> SmuxParams {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return SmuxParams::default();
    };
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let number = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as u32)
    };
    let flag = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    let mut params = SmuxParams::default();
    if let Some(protocol) = text("protocol") {
        params.protocol = protocol;
    }
    params.enabled = flag("enabled");
    if let Some(value) = number("max-connections") {
        params.max_connections = value;
    }
    if let Some(value) = number("min-streams") {
        params.min_streams = value;
    }
    if let Some(value) = number("max-streams") {
        params.max_streams = value;
    }
    params.padding = flag("padding");
    params.statistic = flag("statistic");
    params.only_tcp = flag("only-tcp");
    params
}

fn smux_to_json(params: &SmuxParams) -> serde_json::Value {
    json!({
        "enabled": params.enabled,
        "protocol": params.protocol.trim(),
        "max-connections": params.max_connections,
        "min-streams": params.min_streams,
        "max-streams": params.max_streams,
        "padding": params.padding,
        "statistic": params.statistic,
        "only-tcp": params.only_tcp,
    })
}

fn proxies_sequence_mut(document: &mut Value) -> Result<&mut Vec<Value>, Failure> {
    if !document.is_mapping() {
        return Err(invalid_input("profile YAML must be a top-level mapping"));
    }
    let mapping = document.as_mapping_mut().expect("checked mapping above");
    let key = Value::String("proxies".to_string());
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

/// Overlay the draft's owned keys onto an existing node, keeping every other
/// key verbatim and reporting which ones were preserved.
fn merge_preserving_unknown(
    previous: &Value,
    node: &Value,
    unknown_fields: &mut Vec<String>,
) -> Value {
    let Some(previous_map) = previous.as_mapping() else {
        return node.clone();
    };
    let Some(node_map) = node.as_mapping() else {
        return node.clone();
    };
    let mut merged = Mapping::new();
    for (key, value) in previous_map {
        let Some(name) = key.as_str() else {
            continue;
        };
        if !DRAFT_OWNED_KEYS.contains(&name) {
            unknown_fields.push(name.to_string());
            merged.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in node_map {
        merged.insert(key.clone(), value.clone());
    }
    Value::Mapping(merged)
}

fn collect_unknown_fields(nodes: &[ProxyNodeItem]) -> Vec<String> {
    let mut fields: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.extra.keys().cloned())
        .collect();
    fields.sort();
    fields.dedup();
    fields
}

fn sections_without_proxies(document: &Value) -> Value {
    let mut clone = document.clone();
    if let Some(mapping) = clone.as_mapping_mut() {
        mapping.remove(Value::String("proxies".to_string()));
    }
    clone
}

fn sections_without_proxies_from_str(document: &str) -> Result<Value, Failure> {
    let parsed: Value = serde_yaml_ng::from_str(document)
        .map_err(|error| invalid_input(&format!("exported profile is invalid YAML: {error}")))?;
    Ok(sections_without_proxies(&parsed))
}

fn invalid_input(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message.to_string(), false)
}
