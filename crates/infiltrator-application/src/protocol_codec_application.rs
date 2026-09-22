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
};
use infiltrator_contract::protocol_trust::CaTrustReport;
use infiltrator_domain::profile_converter::{ProfileConverter, ProfileFormat, ProxyNodeItem};
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use serde_yaml_ng::{Mapping, Value};

use crate::certificate_authority_application::CertificateAuthorityApplication;
use crate::dialer_chain_application::DialerChainApplication;
use crate::protocol_node_params::node_from_draft;
use crate::protocol_node_projection::{
    DRAFT_NESTED_KEYS, draft_from_node, is_typed_extra_key_for, owned_keys,
};

#[cfg(test)]
#[path = "protocol_codec_application_test.rs"]
mod protocol_codec_application_test;

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
    /// DUAL-05-09/10: the dialer chains + loop findings of the produced
    /// document, so the caller sees a loop the save would introduce.
    pub dialer: infiltrator_contract::dialer_chain::DialerChainReport,
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
        draft_from_node(item)
    }

    /// Project the draft back into the flat profile node shape. Only the
    /// fields the draft owns are written; unset fields are omitted rather
    /// than emitted as empty strings.
    pub fn node_from_draft(draft: &ProtocolDraft) -> ProxyNodeItem {
        node_from_draft(draft)
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
            // DUAL-05-03…05-12 typed parameter blocks: measured, not assumed.
            ("ech", draft.params.ech == returned.params.ech),
            ("tuic", draft.params.tuic == returned.params.tuic),
            (
                "hysteria2",
                draft.params.hysteria2 == returned.params.hysteria2,
            ),
            (
                "wireguard",
                draft.params.wireguard == returned.params.wireguard,
            ),
            (
                "transport",
                draft.params.transport == returned.params.transport,
            ),
            ("plugin", draft.params.plugin == returned.params.plugin),
            ("ssh", draft.params.ssh == returned.params.ssh),
            ("anytls", draft.params.anytls == returned.params.anytls),
            (
                "trojan-ss-opts",
                draft.params.trojan_ss == returned.params.trojan_ss,
            ),
            // DUAL-05-09/13: measured, not assumed — a share link carries
            // neither the dialer hop nor any certificate trust.
            (
                "dialer-proxy",
                draft.dialer_proxy.trim() == returned.dialer_proxy.trim(),
            ),
            (
                "tls-trust",
                draft.params.tls_trust == returned.params.tls_trust,
            ),
        ] {
            if !same {
                gaps.push(field.to_string());
            }
        }
        gaps
    }

    /// Publish the studio snapshot for a freshly decoded or edited draft.
    ///
    /// The dialer graph is a *profile* fact, so the report published by the last
    /// [`Self::publish_dialer_report`] call survives draft edits; the CA
    /// resolution is draft-specific and is therefore cleared here and re-derived
    /// by whichever caller holds the host reader.
    pub fn publish_draft(
        draft: ProtocolDraft,
        uri_preview: Option<String>,
    ) -> ProtocolStudioSnapshot {
        let dialer = studio_snapshot()
            .map(|previous| previous.dialer)
            .unwrap_or_default();
        let snapshot = ProtocolStudioSnapshot {
            report: Some(draft.report()),
            uri_preview,
            uri_gaps: Self::uri_fidelity_gaps(&draft),
            draft: Some(draft),
            audit: None,
            last_error: None,
            last_saved_node: None,
            dialer,
            ca_trust: CaTrustReport::default(),
        };
        publish_studio(snapshot.clone());
        snapshot
    }

    /// DUAL-05-09/10: analyse a profile document's dialer/relay graph and
    /// publish the chains + loop findings for both surfaces.
    pub fn publish_dialer_report(
        profile_yaml: &str,
    ) -> Result<infiltrator_contract::dialer_chain::DialerChainReport, Failure> {
        let report = DialerChainApplication::analyze_profile(profile_yaml)?;
        let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
        let mut next = studio.take().unwrap_or_default();
        next.dialer = report.clone();
        *studio = Some(next);
        Ok(report)
    }

    /// DUAL-05-10: the dialer report both surfaces render (empty until a
    /// profile was analysed).
    pub fn dialer_report() -> infiltrator_contract::dialer_chain::DialerChainReport {
        studio_snapshot()
            .map(|studio| studio.dialer)
            .unwrap_or_default()
    }

    /// DUAL-05-13: resolve the draft's CA request against the host reader and
    /// publish the outcome. A host without a reader publishes the typed
    /// unsupported state instead of a claimed load.
    pub fn publish_ca_trust(
        params: &infiltrator_contract::protocol_trust::TlsTrustParams,
        port: Option<&dyn CertificateAuthorityPort>,
    ) -> CaTrustReport {
        let report = CertificateAuthorityApplication::resolve(params, port);
        let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
        let mut next = studio.take().unwrap_or_default();
        next.ca_trust = report.clone();
        *studio = Some(next);
        report
    }

    /// DUAL-05-13: the published CA resolution.
    pub fn ca_trust_report() -> CaTrustReport {
        studio_snapshot()
            .map(|studio| studio.ca_trust)
            .unwrap_or_default()
    }

    /// DUAL-05-09/13: edit one whitelisted field of the published draft.
    ///
    /// Only the fields this batch adds are editable through this seam; an
    /// unknown name is a typed `InvalidInput`, never a silently ignored write.
    pub fn update_draft_field(field: &str, value: &str) -> Result<ProtocolStudioSnapshot, Failure> {
        let studio = studio_snapshot().ok_or_else(|| {
            invalid_input("no custom-node draft is published; import or open a node first")
        })?;
        let mut draft = studio
            .draft
            .clone()
            .ok_or_else(|| invalid_input("no custom-node draft is published"))?;
        match field {
            "dialer-proxy" => draft.dialer_proxy = value.to_string(),
            "ca-path" => draft.params.tls_trust.ca_path = value.to_string(),
            "ca-str" => draft.params.tls_trust.ca_str = value.to_string(),
            "ca-fingerprint" => draft.params.tls_trust.fingerprint = value.to_string(),
            other => {
                return Err(invalid_input(&format!(
                    "`{other}` is not an editable custom-node draft field"
                )));
            }
        }
        let preview = Self::uri_from_draft(&draft).ok();
        Ok(Self::publish_draft(draft, preview))
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
                existing[index] = merge_preserving_unknown(
                    &previous,
                    &node_value,
                    draft.family(),
                    &mut unknown_fields,
                );
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

        // DUAL-05-13: project the file-path anchor into the global
        // `tls.custom-certifactes` list — the carrier the pinned mihomo
        // v1.19.18 really reads. Everything else in `tls:` stays untouched.
        let trust_added = CertificateAuthorityApplication::write_ca_path(
            &mut document,
            &draft.params.tls_trust.ca_path,
        );

        let profile_yaml = serde_yaml_ng::to_string(&document)
            .map_err(|error| invalid_input(&format!("could not serialize profile: {error}")))?;
        // The audit compares the untouched sections; the one CA entry this
        // application added is its own key, so it is removed before comparing.
        let mut reported: Value = serde_yaml_ng::from_str(&profile_yaml).map_err(|error| {
            invalid_input(&format!("exported profile is invalid YAML: {error}"))
        })?;
        if let Some(added) = trust_added.as_deref() {
            CertificateAuthorityApplication::remove_ca_path(&mut reported, added);
        }
        let structure_preserved = original_sections == sections_without_proxies(&reported);

        let dialer = DialerChainApplication::analyze_profile(&profile_yaml).unwrap_or_default();

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
            dialer: dialer.clone(),
        };
        publish_studio(ProtocolStudioSnapshot {
            draft: Some(draft.clone()),
            report: Some(commit.report.clone()),
            uri_preview: Self::uri_from_draft(draft).ok(),
            audit: Some(commit.audit.clone()),
            last_error: None,
            last_saved_node: Some(draft.name.trim().to_string()),
            uri_gaps: Self::uri_fidelity_gaps(draft),
            dialer,
            ca_trust: CaTrustReport::default(),
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
            ..ProtocolStudioSnapshot::default()
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
/// key verbatim and reporting which ones were preserved. Map-valued owned keys
/// are merged recursively so unknown sub-keys (e.g. `ws-opts.headers.User-Agent`)
/// survive too; their dotted paths land in `unknown_fields`.
fn merge_preserving_unknown(
    previous: &Value,
    node: &Value,
    family: infiltrator_contract::protocol_fidelity::ProtocolFamily,
    unknown_fields: &mut Vec<String>,
) -> Value {
    let Some(previous_map) = previous.as_mapping() else {
        return node.clone();
    };
    let Some(node_map) = node.as_mapping() else {
        return node.clone();
    };
    let owned = owned_keys(family);
    let mut merged = Mapping::new();
    for (key, value) in previous_map {
        let Some(name) = key.as_str() else {
            continue;
        };
        if !owned.contains(&name) {
            unknown_fields.push(name.to_string());
            merged.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in node_map {
        let nested = key
            .as_str()
            .is_some_and(|name| DRAFT_NESTED_KEYS.contains(&name) && owned.contains(&name));
        if nested {
            let previous_sub = previous_map.get(key).and_then(Value::as_mapping);
            let Some(node_sub) = value.as_mapping() else {
                merged.insert(key.clone(), value.clone());
                continue;
            };
            let path = key.as_str().unwrap_or_default().to_string();
            match previous_sub {
                Some(previous_sub) => {
                    let sub = merge_nested(previous_sub, node_sub, &path, unknown_fields);
                    merged.insert(key.clone(), Value::Mapping(sub));
                }
                None => {
                    merged.insert(key.clone(), value.clone());
                }
            }
            continue;
        }
        merged.insert(key.clone(), value.clone());
    }
    Value::Mapping(merged)
}

/// Merge one map-valued draft-owned key, keeping previous sub-keys the draft
/// did not write (recursively) and reporting their dotted paths.
fn merge_nested(
    previous: &Mapping,
    node: &Mapping,
    path: &str,
    unknown_fields: &mut Vec<String>,
) -> Mapping {
    let mut merged = Mapping::new();
    for (key, value) in node {
        let sub_path = match key.as_str() {
            Some(name) => format!("{path}.{name}"),
            None => path.to_string(),
        };
        match (
            previous.get(key).and_then(Value::as_mapping),
            value.as_mapping(),
        ) {
            (Some(previous_sub), Some(node_sub)) => {
                merged.insert(
                    key.clone(),
                    Value::Mapping(merge_nested(
                        previous_sub,
                        node_sub,
                        &sub_path,
                        unknown_fields,
                    )),
                );
            }
            _ => {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    for (key, value) in previous {
        if node.get(key).is_some() {
            continue;
        }
        let sub_path = match key.as_str() {
            Some(name) => format!("{path}.{name}"),
            None => path.to_string(),
        };
        unknown_fields.push(sub_path);
        merged.insert(key.clone(), value.clone());
    }
    merged
}

fn collect_unknown_fields(nodes: &[ProxyNodeItem]) -> Vec<String> {
    let mut fields: Vec<String> = nodes
        .iter()
        .flat_map(|node| {
            let family = infiltrator_contract::protocol_fidelity::ProtocolFamily::from_type_str(
                &node.node_type,
            );
            node.extra
                .keys()
                .filter(|key| !is_typed_extra_key_for(key, family))
                .cloned()
                .collect::<Vec<String>>()
        })
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

fn invalid_input(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message.to_string(), false)
}
