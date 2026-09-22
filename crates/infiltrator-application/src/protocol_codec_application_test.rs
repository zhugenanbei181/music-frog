//! DUAL-05 shared-codec application tests (protocol drafts + lossless splice).

use super::*;
use infiltrator_contract::protocol_fidelity::{ProtocolFamily, ShadowsocksCipher, VlessFlow};
use infiltrator_domain::profile_converter::ProfileFormat;

const PROFILE: &str = r#"
mixed-port: 7890
mode: rule
proxies:
  - name: "hk-01"
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: 2022-blake3-aes-128-gcm
    password: "AAAAAAAAAAAAAAAAAAAAAA=="
    custom-future-meta-flag: true
    custom-numeric-parameter: 42
  - name: "jp-01"
    type: trojan
    server: 2.2.2.2
    port: 443
    password: pw
proxy-groups:
  - name: PROXY
    type: select
    proxies: [hk-01, jp-01]
rules:
  - MATCH,PROXY
"#;

/// A structurally valid REALITY public key: 43 base64 characters.
fn reality_pbk() -> String {
    format!("{}{}", "PubKey1234567890", "A".repeat(27))
}

fn ss_draft() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "hk-01".into();
    draft.server = "3.3.3.3".into();
    draft.port = 8388;
    draft.cipher = "2022-blake3-aes-256-gcm".into();
    draft.password = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into();
    draft
}

#[test]
fn uri_draft_round_trip_keeps_the_2022_cipher_family() {
    // 2022-blake3-aes-128-gcm with a 16-byte PSK.
    let uri = "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206QUFBQUFBQUFBQUFBQUFBQUFBQUE9@ss.example.com:8388#SS2022";
    let draft = ProtocolCodecApplication::draft_from_uri(uri).unwrap();
    assert_eq!(draft.node_type, "ss");
    assert_eq!(draft.cipher, "2022-blake3-aes-128-gcm");
    assert_eq!(
        draft.cipher_family(),
        Some(ShadowsocksCipher::Blake3Aes128Gcm)
    );
    let report = ProtocolCodecApplication::report(&draft);
    assert!(report.cipher_is_2022);
    assert_eq!(report.cipher_key_bytes, Some(16));
    assert_eq!(report.family, ProtocolFamily::Shadowsocks);

    let exported = ProtocolCodecApplication::uri_from_draft(&draft).unwrap();
    let returned = ProtocolCodecApplication::draft_from_uri(&exported).unwrap();
    assert_eq!(returned.cipher, draft.cipher);
    assert_eq!(returned.password, draft.password);
    assert!(
        ProtocolCodecApplication::uri_fidelity_gaps(&draft)
            .iter()
            .all(|gap| gap != "cipher" && gap != "password"),
        "{:?}",
        ProtocolCodecApplication::uri_fidelity_gaps(&draft)
    );
}

#[test]
fn vless_reality_vision_draft_carries_shared_chips_and_issues() {
    let uri = format!(
        "vless://b831381d-6324-4d53-ad4f-8cda48b30811@us.example.com:443?security=reality&encryption=none&pbk={}&sid=abcd1234&spx=%2Fspider&fp=chrome&flow=xtls-rprx-vision&sni=reality.example.com#US-Reality",
        reality_pbk()
    );
    let draft = ProtocolCodecApplication::draft_from_uri(&uri).unwrap();
    assert_eq!(draft.node_type, "vless");
    assert_eq!(draft.flow, "xtls-rprx-vision");
    assert_eq!(draft.flow_family(), Some(VlessFlow::Vision));
    assert_eq!(draft.reality.short_id, "abcd1234");
    assert_eq!(draft.reality.spider_x, "/spider");
    assert_eq!(draft.reality.fingerprint, "chrome");
    assert!(draft.uses_reality());

    let report = ProtocolCodecApplication::report(&draft);
    assert_eq!(report.flow_chip.as_deref(), Some("Vision 流控"));
    assert!(report.reality_chips.contains(&"Reality".to_string()));
    assert!(report.reality_chips.contains(&"sid:abcd1234".to_string()));
    assert!(report.issues.is_empty(), "{:?}", report.issues);

    // A wrong-length pbk is surfaced, never silently accepted.
    let mut broken = draft.clone();
    broken.reality.public_key = "short".into();
    let report = ProtocolCodecApplication::report(&broken);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "reality.public-key")
    );
}

#[test]
fn smux_overrides_are_yaml_only_and_reported_as_a_uri_gap() {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "mux".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".into();
    draft.smux.enabled = true;
    draft.smux.protocol = "yamux".into();
    draft.smux.max_connections = 8;
    draft.smux.min_streams = 2;
    draft.smux.max_streams = 16;
    draft.smux.padding = true;

    let report = ProtocolCodecApplication::report(&draft);
    assert_eq!(
        report.smux_chips,
        vec![
            "mux:yamux".to_string(),
            "mc:8".to_string(),
            "ms:16".to_string(),
            "min:2".to_string(),
            "padding".to_string(),
        ]
    );

    // Share links cannot express smux: that is a format fact, and the gap is
    // computed by a real round-trip rather than hard-coded.
    let gaps = ProtocolCodecApplication::uri_fidelity_gaps(&draft);
    assert!(gaps.contains(&"smux".to_string()), "{gaps:?}");

    // The YAML path keeps every override.
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    let parsed =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    assert_eq!(parsed.len(), 1);
    let returned = ProtocolCodecApplication::draft_from_node(&parsed[0]);
    assert_eq!(returned.smux, draft.smux);
}

#[test]
fn upsert_preserves_every_other_section_and_unknown_node_keys() {
    let commit = ProtocolCodecApplication::upsert_draft_into_profile(PROFILE, &ss_draft()).unwrap();

    assert!(commit.replaced_existing);
    assert_eq!(commit.node_count, 2);
    assert!(commit.is_structure_preserving(), "{:?}", commit.audit);
    assert!(commit.audit.lossless);
    assert_eq!(
        commit.audit.unknown_fields,
        vec![
            "custom-future-meta-flag".to_string(),
            "custom-numeric-parameter".to_string(),
        ]
    );

    // Every untouched section survives verbatim.
    let document: serde_yaml_ng::Value = serde_yaml_ng::from_str(&commit.profile_yaml).unwrap();
    assert_eq!(
        document.get("mixed-port").and_then(|v| v.as_u64()),
        Some(7890)
    );
    assert_eq!(document.get("mode").and_then(|v| v.as_str()), Some("rule"));
    assert_eq!(
        document
            .get("rules")
            .and_then(|v| v.as_sequence())
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        document
            .get("proxy-groups")
            .and_then(|v| v.as_sequence())
            .map(Vec::len),
        Some(1)
    );

    // The replaced node carries the draft's cipher and keeps its unknown keys.
    let nodes =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].name, "hk-01");
    assert_eq!(nodes[0].server, "3.3.3.3");
    assert_eq!(nodes[0].cipher.as_deref(), Some("2022-blake3-aes-256-gcm"));
    assert_eq!(
        nodes[0].extra.get("custom-future-meta-flag"),
        Some(&serde_yaml_ng::Value::Bool(true))
    );
    assert_eq!(
        nodes[0].extra.get("custom-numeric-parameter"),
        Some(&serde_yaml_ng::Value::Number(42.into()))
    );
}

#[test]
fn upsert_inserts_a_new_node_and_reports_preserved_sections() {
    let mut draft = ss_draft();
    draft.name = "new-node".into();
    let commit = ProtocolCodecApplication::upsert_draft_into_profile(PROFILE, &draft).unwrap();
    assert!(!commit.replaced_existing);
    assert_eq!(commit.node_count, 3);
    assert!(commit.is_structure_preserving());

    let nodes =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    assert_eq!(nodes[0].name, "new-node");
    assert_eq!(nodes[1].name, "hk-01");

    // A document without a `proxies:` key gains one without losing anything.
    let bare = "mode: rule\nrules:\n  - MATCH,DIRECT\n";
    let commit = ProtocolCodecApplication::upsert_draft_into_profile(bare, &draft).unwrap();
    assert_eq!(commit.node_count, 1);
    assert!(commit.is_structure_preserving(), "{:?}", commit.audit);
}

#[test]
fn upsert_refuses_a_draft_with_unresolved_protocol_issues() {
    let mut draft = ss_draft();
    draft.password = "AAAAAAAAAAAAAAAAAAAAAA==".into();
    let failure = ProtocolCodecApplication::upsert_draft_into_profile(PROFILE, &draft).unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert!(failure.message.contains("key size"), "{}", failure.message);

    let mut draft = ss_draft();
    draft.reality.public_key = "D".repeat(43);
    let failure = ProtocolCodecApplication::upsert_draft_into_profile(PROFILE, &draft).unwrap_err();
    assert!(
        failure.message.contains("reality.unsupported"),
        "{}",
        failure.message
    );
}

#[test]
fn yaml_round_trip_audit_is_honest_about_section_loss() {
    // The whole-document parse/export path is NOT section-lossless: this is
    // the exact regression the draft upsert was introduced to remove.
    let (_output, audit) = ProtocolCodecApplication::audit_conversion(
        PROFILE,
        NodeCodecFormat::ClashYaml,
        NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    assert_eq!(audit.node_count, 2);
    assert!(!audit.structure_preserved);
    assert!(!audit.lossless);

    // A document that only carries `proxies:` round-trips losslessly.
    let minimal = "proxies:\n  - name: a\n    type: ss\n    server: 1.1.1.1\n    port: 443\n    cipher: aes-128-gcm\n    password: pw\n    future-flag: 7\n";
    let (_output, audit) = ProtocolCodecApplication::audit_conversion(
        minimal,
        NodeCodecFormat::ClashYaml,
        NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    assert!(audit.structure_preserved);
    assert!(audit.lossless);
    assert_eq!(audit.unknown_fields, vec!["future-flag".to_string()]);
}

#[test]
fn studio_snapshot_is_published_for_both_surfaces() {
    clear_studio();
    assert!(studio_snapshot().is_none());

    let uri = format!(
        "vless://uuid-1@example.com:443?flow=xtls-rprx-vision&pbk={}#node",
        reality_pbk()
    );
    let draft = ProtocolCodecApplication::draft_from_uri(&uri).unwrap();
    let snapshot =
        ProtocolCodecApplication::publish_draft(draft.clone(), Some("vless://node".into()));
    assert!(snapshot.last_error.is_none());
    assert!(snapshot.uri_preview.is_some());
    assert!(!snapshot.has_blocking_issue());

    let live = studio_snapshot().unwrap();
    assert_eq!(
        live.draft.as_ref().map(|d| d.name.clone()),
        Some("node".into())
    );
    assert_eq!(
        live.report.as_ref().map(|r| r.family),
        Some(ProtocolFamily::Vless)
    );

    ProtocolCodecApplication::publish_error("share link could not be parsed");
    let live = studio_snapshot().unwrap();
    assert_eq!(
        live.last_error.as_deref(),
        Some("share link could not be parsed")
    );

    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    assert_eq!(commit.audit.source_format, NodeCodecFormat::Uri);
    assert_eq!(commit.audit.target_format, NodeCodecFormat::ClashYaml);
    clear_studio();
    assert!(studio_snapshot().is_none());
}

#[test]
fn draft_projection_keeps_weak_types_and_unknown_fields() {
    let yaml = r#"
proxies:
  - name: "weak"
    type: vless
    server: example.com
    port: 443
    uuid: uuid-2
    custom-field: keep-me
"#;
    let nodes = ProfileCodecFixture::parse(yaml, ProfileFormat::ClashYaml);
    let draft = ProtocolCodecApplication::draft_from_node(&nodes[0]);
    assert_eq!(draft.preserved_fields, vec!["custom-field".to_string()]);
    assert_eq!(draft.node_type, "vless");
    assert_eq!(draft.uuid, "uuid-2");
    assert!(!draft.tls);

    // The projection writes exactly the fields the draft owns.
    let item = ProtocolCodecApplication::node_from_draft(&draft);
    assert_eq!(item.uuid.as_deref(), Some("uuid-2"));
    assert!(item.password.is_none());
    assert!(item.smux.is_none());
    assert!(item.reality_opts.is_none());
}

/// Thin wrapper so the fixture above reads as an explicit profile parse.
struct ProfileCodecFixture;

impl ProfileCodecFixture {
    fn parse(yaml: &str, format: ProfileFormat) -> Vec<ProxyNodeItem> {
        ProtocolCodecApplication::parse_nodes(
            yaml,
            match format {
                ProfileFormat::ClashYaml => NodeCodecFormat::ClashYaml,
                ProfileFormat::RawJson => NodeCodecFormat::RawJson,
                ProfileFormat::ShadowrocketUriList => NodeCodecFormat::Uri,
                ProfileFormat::Base64Subscription => NodeCodecFormat::Base64Subscription,
            },
        )
        .unwrap()
    }
}
