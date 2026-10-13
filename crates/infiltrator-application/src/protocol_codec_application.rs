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
//! Draft projection and studio operations are executor-neutral. Saving is the
//! one asynchronous use case: it asks the host runtime to commit adapter-owned
//! bytes through the source-bound workspace transaction.

use crate::certificate_authority_application::CertificateAuthorityApplication;
use crate::dialer_chain_application::DialerChainApplication;
use crate::profile_application::{ProfileApplication, ProfileRuntimeAccess};
use crate::protocol_node_params::node_from_draft;
use crate::protocol_node_projection::draft_from_node;
use infiltrator_contract::dialer_chain::DialerChainReport;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_contract::protocol_fidelity::{
    CodecAudit, ProtocolDraft, ProtocolFidelityReport, ProtocolStudioSnapshot,
};
use infiltrator_contract::protocol_trust::{CaTrustReport, TlsTrustParams};
use infiltrator_domain::profile_converter::{ProfileConverter, ProxyNodeItem};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::certificate_authority::CertificateAuthorityPort;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationReadout, ProfileConfigurationUpdate,
};
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use std::sync::{Arc, Mutex, OnceLock};

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
    pub dialer: DialerChainReport,
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
        let mut snapshot = Self::project_draft(draft, uri_preview);
        snapshot.dialer = dialer;
        publish_studio(snapshot.clone());
        snapshot
    }

    /// Fold a staged draft without publishing or touching persistent storage.
    pub fn project_draft(
        draft: ProtocolDraft,
        uri_preview: Option<String>,
    ) -> ProtocolStudioSnapshot {
        ProtocolStudioSnapshot {
            report: Some(draft.report()),
            uri_preview,
            uri_gaps: Self::uri_fidelity_gaps(&draft),
            draft: Some(draft),
            audit: None,
            last_error: None,
            last_saved_node: None,
            dialer: DialerChainReport::default(),
            ca_trust: CaTrustReport::default(),
        }
    }

    /// DUAL-05-09/10: analyse a profile document's dialer/relay graph and
    /// publish the chains + loop findings for both surfaces.
    pub fn publish_dialer_report(profile_yaml: &str) -> Result<DialerChainReport, Failure> {
        let report = DialerChainApplication::analyze_profile(profile_yaml)?;
        let mut studio = last_studio().lock().unwrap_or_else(|e| e.into_inner());
        let mut next = studio.take().unwrap_or_default();
        next.dialer = report.clone();
        *studio = Some(next);
        Ok(report)
    }

    /// DUAL-05-10: the dialer report both surfaces render (empty until a
    /// profile was analysed).
    pub fn dialer_report() -> DialerChainReport {
        studio_snapshot()
            .map(|studio| studio.dialer)
            .unwrap_or_default()
    }

    /// DUAL-05-13: resolve the draft's CA request against the host reader and
    /// publish the outcome. A host without a reader publishes the typed
    /// unsupported state instead of a claimed load.
    pub fn publish_ca_trust(
        params: &TlsTrustParams,
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

    /// Publish saved facts only after the caller's persistence transaction succeeds.
    pub fn publish_commit(commit: &ProtocolCommit) {
        publish_studio(ProtocolStudioSnapshot {
            draft: Some(commit.draft.clone()),
            report: Some(commit.report.clone()),
            uri_preview: Self::uri_from_draft(&commit.draft).ok(),
            audit: Some(commit.audit.clone()),
            last_error: None,
            last_saved_node: Some(commit.draft.name.trim().to_string()),
            uri_gaps: Self::uri_fidelity_gaps(&commit.draft),
            dialer: commit.dialer.clone(),
            ca_trust: CaTrustReport::default(),
        });
    }

    /// Apply the protocol draft and its profile-level CA path through the
    /// engine codec, then commit the exact candidate through the source-bound
    /// workspace transaction. Every typed readback is decoded from the
    /// adapter-produced bytes before the transaction is allowed to write.
    pub async fn save_draft_to_active_profile<R: ActiveProfileReferenceApplier + ?Sized>(
        profiles: &ProfileApplication,
        runtime: Arc<R>,
        draft: &ProtocolDraft,
        authorization: ProfileWriteAuthorization,
    ) -> Result<ProtocolCommit, Failure> {
        let workspace = profiles.load_active_workspace().await?;
        let expected = workspace.identity();
        let trust_update = TlsTrustParams {
            ca_path: draft.params.tls_trust.ca_path.clone(),
            ..TlsTrustParams::default()
        };
        let trust_mutation = CertificateAuthorityApplication::apply_profile_trust_anchors(
            profiles,
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            trust_update,
        )?;
        let ProfileConfigurationReadout::TrustAnchors(_) = trust_mutation.readback else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "profile CA codec returned the wrong typed readback",
                false,
            ));
        };
        let protocol_mutation = profiles.apply_profile_configuration(
            &workspace.document.identity,
            &trust_mutation.document_bytes,
            ProfileConfigurationUpdate::UpsertProtocolNode(draft.clone()),
        )?;
        let ProfileConfigurationReadout::ProtocolNodeMutation(readback) =
            protocol_mutation.readback
        else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "profile protocol codec returned the wrong typed readback",
                false,
            ));
        };
        let saved_draft = readback
            .nodes
            .iter()
            .find(|node| node.name.trim() == draft.name.trim())
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "profile protocol codec did not read back the saved node",
                    false,
                )
            })?;
        let mut expected_readback = Self::draft_from_node(&Self::node_from_draft(draft));
        expected_readback.preserved_fields = saved_draft.preserved_fields.clone();
        if saved_draft != expected_readback {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "profile protocol codec readback differs from the requested typed draft",
                false,
            ));
        }
        profiles.validate_document_write(&expected, &protocol_mutation.document_bytes)?;
        let updated = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::DirectEdit { authorization },
            document_bytes: protocol_mutation.document_bytes,
            sidecar_bytes: workspace.sidecar_bytes.clone(),
        };
        let committed = profiles
            .commit_workspace(
                ProfileRuntimeAccess::CoreOwner(runtime),
                &expected,
                &updated,
            )
            .await?;
        let profile_yaml = committed
            .document
            .utf8_text()
            .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?
            .to_owned();
        let dialer = DialerChainApplication::analyze_profile(&profile_yaml).unwrap_or_default();
        let node_count = readback.audit.node_count;
        Ok(ProtocolCommit {
            profile_yaml,
            report: saved_draft.report(),
            draft: saved_draft,
            audit: readback.audit,
            replaced_existing: readback.replaced_existing,
            node_count,
            dialer,
        })
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

fn invalid_input(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message.to_string(), false)
}
