use super::*;
use std::str;

fn valid_vless() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "existing".to_owned();
    draft.server = "new.example.com".to_owned();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    draft.params.transport.network = "ws".to_owned();
    draft.params.transport.ws.path = "/new".to_owned();
    draft
        .params
        .transport
        .ws
        .headers
        .insert("Host".to_owned(), "new.example.com".to_owned());
    draft
}

#[test]
fn protocol_upsert_preserves_foreign_fields_and_decodes_exact_output_bytes() {
    let input = br#"
mode: rule
proxies:
  - name: existing
    type: vless
    server: old.example.com
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    network: ws
    ws-opts:
      path: /old
      headers:
        Host: old.example.com
      future-nested: keep-nested
    future-field: keep-root
proxy-groups:
  - name: PROXY
    type: select
    proxies: [existing]
rules:
  - MATCH,PROXY
"#;
    let mutation = upsert_protocol_node(input, valid_vless()).expect("typed upsert");
    let yaml = str::from_utf8(&mutation.document_bytes).expect("UTF-8 output");
    let output: Value = serde_yaml_ng::from_slice(&mutation.document_bytes).unwrap();
    let ProfileConfigurationReadout::ProtocolNodeMutation(readback) = mutation.readback else {
        panic!("expected protocol-node mutation readback");
    };

    assert!(readback.replaced_existing);
    assert_eq!(readback.audit.node_count, 1);
    assert!(readback.audit.structure_preserved);
    assert_eq!(
        readback.audit.unknown_fields,
        vec![
            "future-field".to_owned(),
            "ws-opts.future-nested".to_owned()
        ]
    );
    assert_eq!(readback.nodes[0].server, "new.example.com");
    assert_eq!(readback.nodes[0].params.transport.ws.path, "/new");
    assert_eq!(readback.nodes[0].preserved_fields, vec!["future-field"]);
    assert_eq!(output.get("mode").and_then(Value::as_str), Some("rule"));
    assert_eq!(
        output
            .get("proxy-groups")
            .and_then(Value::as_sequence)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        output
            .get("rules")
            .and_then(Value::as_sequence)
            .unwrap()
            .len(),
        1
    );
    assert!(yaml.contains("future-nested: keep-nested"));

    assert_eq!(
        read_protocol_nodes(&mutation.document_bytes).unwrap(),
        readback.nodes
    );
}

#[test]
fn invalid_protocol_draft_and_malformed_yaml_fail_before_a_mutation_is_returned() {
    let mut invalid = valid_vless();
    invalid.uuid.clear();
    assert!(matches!(
        upsert_protocol_node(b"not: [yaml", invalid),
        Err(PortError::Rejected(Failure {
            code: ErrorCode::InvalidInput,
            ..
        }))
    ));
    assert!(upsert_protocol_node(b"not: [yaml", valid_vless()).is_err());
}

#[test]
fn ca_path_operation_is_idempotent_and_keeps_the_other_tls_members() {
    let input = b"mode: rule\ntls:\n  certificate: /etc/ssl/cert.pem\n  custom-certifactes:\n    - /etc/ssl/existing.pem\nrules:\n  - MATCH,DIRECT\n";
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/custom-ca.pem".to_owned(),
        ..TlsTrustParams::default()
    };
    let first = apply_trust_anchors(input, params.clone()).expect("path update");
    let ProfileConfigurationReadout::TrustAnchors(readback) = first.readback else {
        panic!("expected CA path readback");
    };
    assert_eq!(readback.ca_path, "/etc/ssl/custom-ca.pem");
    let yaml = str::from_utf8(&first.document_bytes).unwrap();
    assert!(yaml.contains("certificate: /etc/ssl/cert.pem"));
    assert!(yaml.contains("/etc/ssl/existing.pem"));
    assert!(yaml.contains("MATCH,DIRECT"));

    let second = apply_trust_anchors(&first.document_bytes, params).expect("idempotent update");
    assert_eq!(second.document_bytes, first.document_bytes);
    assert_eq!(
        read_trust_anchors(&second.document_bytes).unwrap().ca_path,
        "/etc/ssl/custom-ca.pem"
    );
}

#[test]
fn inline_pem_and_peer_fingerprint_are_not_profile_path_anchors() {
    let inline = TlsTrustParams {
        ca_str: "-----BEGIN CERTIFICATE-----\ninline\n-----END CERTIFICATE-----".to_owned(),
        ..TlsTrustParams::default()
    };
    assert!(matches!(
        apply_trust_anchors(b"mode: rule\n", inline),
        Err(PortError::Unsupported { .. })
    ));

    let fingerprint = TlsTrustParams {
        fingerprint: "00".repeat(32),
        ..TlsTrustParams::default()
    };
    assert!(matches!(
        apply_trust_anchors(b"mode: rule\n", fingerprint),
        Err(PortError::Rejected(Failure {
            code: ErrorCode::InvalidInput,
            ..
        }))
    ));
}

#[test]
fn malformed_ca_profile_shapes_are_rejected_before_candidate_bytes_exist() {
    let params = TlsTrustParams {
        ca_path: "/etc/ssl/custom-ca.pem".to_owned(),
        ..TlsTrustParams::default()
    };
    assert!(apply_trust_anchors(b"tls: invalid\n", params.clone()).is_err());
    assert!(apply_trust_anchors(b"tls:\n  custom-certifactes: false\n", params).is_err());
}
