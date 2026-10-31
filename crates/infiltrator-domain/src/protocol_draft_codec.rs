//! Engine-neutral protocol draft/URI conversions and measured URI fidelity.

use crate::error::DomainError;
use crate::profile_converter::ProfileConverter;
use crate::protocol_node_params::node_from_draft;
use crate::protocol_node_projection::draft_from_node;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;

pub fn draft_from_uri(uri: &str) -> Result<ProtocolDraft, DomainError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(DomainError::invalid("share link must not be empty"));
    }
    let item = ProfileConverter::parse_uri(trimmed)
        .map_err(|error| DomainError::invalid(format!("could not parse share link: {error}")))?;
    let draft = draft_from_node(&item);
    if draft.node_type.trim().is_empty() {
        return Err(DomainError::invalid("share link did not name a protocol"));
    }
    Ok(draft)
}

pub fn uri_from_draft(draft: &ProtocolDraft) -> Result<String, DomainError> {
    let item = node_from_draft(draft);
    ProfileConverter::export_uri(&item)
        .map_err(|error| DomainError::invalid(format!("could not encode share link: {error}")))
}

/// Compare a draft with an actual URI round-trip instead of assuming that the
/// serializer preserves fields it does not encode.
pub fn uri_fidelity_gaps(draft: &ProtocolDraft) -> Vec<String> {
    let Ok(uri) = uri_from_draft(draft) else {
        return vec!["uri".to_string()];
    };
    let Ok(returned) = draft_from_uri(&uri) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_draft_uri_round_trip_keeps_encoded_fields() {
        let mut draft = ProtocolDraft::new("vless");
        draft.name = "edge node".to_owned();
        draft.server = "edge.example.test".to_owned();
        draft.port = 443;
        draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
        draft.sni = "cdn.example.test".to_owned();

        let returned = draft_from_uri(&uri_from_draft(&draft).unwrap()).unwrap();

        assert_eq!(returned.name, draft.name);
        assert_eq!(returned.server, draft.server);
        assert_eq!(returned.port, draft.port);
        assert_eq!(returned.uuid, draft.uuid);
        assert_eq!(returned.sni, draft.sni);
    }

    #[test]
    fn uri_fidelity_is_measured_for_fields_that_the_link_cannot_carry() {
        let mut draft = ProtocolDraft::new("vless");
        draft.name = "edge node".to_owned();
        draft.server = "edge.example.test".to_owned();
        draft.port = 443;
        draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
        draft.dialer_proxy = "gateway".to_owned();
        draft.params.tls_trust.ca_path = "/etc/ssl/custom.pem".to_owned();
        draft.preserved_fields.push("future-field".to_owned());

        let gaps = uri_fidelity_gaps(&draft);

        assert!(gaps.contains(&"dialer-proxy".to_owned()));
        assert!(gaps.contains(&"tls-trust".to_owned()));
        assert!(gaps.contains(&"unknown-fields".to_owned()));
    }
}
