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

/// 32-byte base64 WireGuard key.
fn wg_key() -> String {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()
}

fn round_trip(draft: &ProtocolDraft) -> ProtocolDraft {
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", draft).unwrap();
    let nodes =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    ProtocolCodecApplication::draft_from_node(&nodes[0])
}

#[test]
fn wireguard_parameters_round_trip_with_mihomo_key_spellings() {
    let mut draft = ProtocolDraft::new("wireguard");
    draft.name = "wg-01".into();
    draft.server = "wg.example.com".into();
    draft.port = 51820;
    draft.params.wireguard = infiltrator_contract::protocol_params_ext::WireGuardParams {
        private_key: wg_key(),
        public_key: wg_key(),
        pre_shared_key: wg_key(),
        reserved: "AQID".into(),
        reserved_is_base64: true,
        ip: "172.16.0.2/32".into(),
        ipv6: "fd00::2/128".into(),
        mtu: 1420,
        dns: vec!["1.1.1.1".into()],
        workers: 2,
        persistent_keepalive: 25,
        allowed_ips: vec!["0.0.0.0/0".into()],
        remote_dns_resolve: true,
        amnezia: infiltrator_contract::protocol_params_ext::AmneziaWgParams {
            jc: Some(4),
            jmin: Some(40),
            jmax: Some(70),
            s1: Some(10),
            s2: Some(20),
            h1: Some(1),
            h2: Some(2),
            h3: Some(3),
            h4: Some(4),
        },
    };
    assert!(draft.report().is_valid(), "{:?}", draft.report().issues);
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    // The mihomo v1.19.18 wire keys, not the old repo spellings.
    assert!(
        commit.profile_yaml.contains("pre-shared-key:"),
        "{}",
        commit.profile_yaml
    );
    assert!(
        commit.profile_yaml.contains("amnezia-wg-option:"),
        "{}",
        commit.profile_yaml
    );
    assert!(
        commit.profile_yaml.contains("reserved: AQID"),
        "the base64 reserved shape is preserved: {}",
        commit.profile_yaml
    );
    let returned = round_trip(&draft);
    assert_eq!(returned.params.wireguard, draft.params.wireguard);
    assert_eq!(returned.params, draft.params);
}

#[test]
fn tuic_and_hysteria2_parameters_reach_the_profile_and_back() {
    let mut tuic = ProtocolDraft::new("tuic");
    tuic.name = "tuic-01".into();
    tuic.server = "tuic.example.com".into();
    tuic.port = 443;
    tuic.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".into();
    tuic.password = "pw".into();
    tuic.params.tuic = infiltrator_contract::protocol_params::TuicParams {
        congestion_controller: "bbr".into(),
        udp_relay_mode: "quic".into(),
        reduce_rtt: true,
        heartbeat_interval: 10000,
        request_timeout: 8000,
        recv_window_conn: 1024,
        recv_window: 4096,
        disable_sni: false,
    };
    assert!(tuic.report().is_valid(), "{:?}", tuic.report().issues);
    let returned = round_trip(&tuic);
    assert_eq!(returned.params.tuic, tuic.params.tuic);
    let report = ProtocolCodecApplication::report(&returned);
    assert!(
        report
            .params
            .congestion_chips
            .contains(&"cc:bbr".to_string())
    );

    let mut hy2 = ProtocolDraft::new("hysteria2");
    hy2.name = "hy2-01".into();
    hy2.server = "hy2.example.com".into();
    hy2.port = 443;
    hy2.password = "pw".into();
    hy2.params.hysteria2 = infiltrator_contract::protocol_params::Hysteria2Params {
        ports: "20000-30000,8443".into(),
        hop_interval: 30,
        obfs: "salamander".into(),
        obfs_password: "obfs-pw".into(),
        up: "100 Mbps".into(),
        down: "200".into(),
        cwnd: 32,
        udp_mtu: 1200,
    };
    assert!(hy2.report().is_valid(), "{:?}", hy2.report().issues);
    let returned = round_trip(&hy2);
    assert_eq!(returned.params.hysteria2, hy2.params.hysteria2);
    let report = ProtocolCodecApplication::report(&returned);
    assert!(
        report
            .params
            .congestion_chips
            .contains(&"ports:20000-30000,8443".to_string())
    );
}

#[test]
fn ssh_and_anytls_parameters_project_through_the_flat_node() {
    let mut ssh = ProtocolDraft::new("ssh");
    ssh.name = "ssh-01".into();
    ssh.server = "ssh.example.com".into();
    ssh.port = 22;
    ssh.params.ssh = infiltrator_contract::protocol_params_ext::SshParams {
        username: "root".into(),
        private_key: "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
        passphrase: "phrase".into(),
        host_key_algorithms: vec!["ssh-ed25519".into()],
    };
    assert!(ssh.report().is_valid(), "{:?}", ssh.report().issues);
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &ssh).unwrap();
    assert!(
        commit.profile_yaml.contains("type: ssh"),
        "{}",
        commit.profile_yaml
    );
    assert!(
        commit.profile_yaml.contains("private-key-passphrase:"),
        "{}",
        commit.profile_yaml
    );
    let returned = round_trip(&ssh);
    assert_eq!(returned.params.ssh, ssh.params.ssh);

    let mut anytls = ProtocolDraft::new("anytls");
    anytls.name = "anytls-01".into();
    anytls.server = "anytls.example.com".into();
    anytls.port = 443;
    anytls.password = "pw".into();
    anytls.params.anytls = infiltrator_contract::protocol_params_ext::AnyTlsParams {
        idle_session_timeout: 30000,
        idle_session_check_interval: 15000,
        min_idle_session: 2,
    };
    assert!(anytls.report().is_valid(), "{:?}", anytls.report().issues);
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &anytls).unwrap();
    assert!(
        commit.profile_yaml.contains("idle-session-timeout: 30000"),
        "{}",
        commit.profile_yaml
    );
    let returned = round_trip(&anytls);
    assert_eq!(returned.params.anytls, anytls.params.anytls);
}

#[test]
fn legacy_idle_timeout_and_amnezia_aliases_still_parse() {
    let yaml = r#"
proxies:
  - name: legacy
    type: anytls
    server: anytls.example.com
    port: 443
    password: pw
    idle-timeout: 60000
"#;
    let nodes = ProtocolCodecApplication::parse_nodes(yaml, NodeCodecFormat::ClashYaml).unwrap();
    let draft = ProtocolCodecApplication::draft_from_node(&nodes[0]);
    assert_eq!(draft.params.anytls.idle_session_timeout, 60000);

    let yaml = r#"
proxies:
  - name: legacy-wg
    type: wireguard
    server: wg.example.com
    port: 51820
    private-key: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
    public-key: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
    preshared-key: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
    amnezia-opts:
      jc: 4
"#;
    let nodes = ProtocolCodecApplication::parse_nodes(yaml, NodeCodecFormat::ClashYaml).unwrap();
    let draft = ProtocolCodecApplication::draft_from_node(&nodes[0]);
    assert_eq!(draft.params.wireguard.pre_shared_key, wg_key());
    assert_eq!(draft.params.wireguard.amnezia.jc, Some(4));
}

#[test]
fn nestable_owned_maps_keep_unknown_sub_keys_during_the_splice() {
    let profile = r#"
mode: rule
proxies:
  - name: ws-node
    type: vless
    server: old.example.com
    port: 443
    uuid: uuid-1
    network: ws
    ws-opts:
      path: /old
      headers:
        Host: old.example.com
        User-Agent: keep-me
      future-ws-key: 7
"#;
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "ws-node".into();
    draft.server = "new.example.com".into();
    draft.port = 443;
    draft.uuid = "uuid-1".into();
    draft.tls = true;
    draft.params.transport.network = "ws".into();
    draft.params.transport.ws.path = "/new".into();
    draft.params.transport.ws.set_host("cdn.example.com");

    let commit = ProtocolCodecApplication::upsert_draft_into_profile(profile, &draft).unwrap();
    let nodes =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    let ws = nodes[0].ws_opts.as_ref().expect("ws-opts").clone();
    assert_eq!(ws["path"], "/new");
    assert_eq!(ws["headers"]["Host"], "cdn.example.com");
    assert_eq!(ws["headers"]["User-Agent"], "keep-me");
    assert_eq!(ws["future-ws-key"], 7);
    assert!(
        commit
            .audit
            .unknown_fields
            .iter()
            .any(|field| field == "ws-opts.headers.User-Agent" || field == "ws-opts.future-ws-key"),
        "{:?}",
        commit.audit.unknown_fields
    );
    assert!(commit.is_structure_preserving());
}

#[test]
fn typed_parameter_blocks_are_measured_as_uri_gaps() {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "n".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.uuid = "uuid-1".into();
    draft.alpn = vec!["h2".into(), "http/1.1".into()];
    draft.params.ech.enabled = true;
    draft.params.transport.network = "ws".into();
    draft.params.transport.ws.path = "/ws".into();
    draft.params.transport.ws.max_early_data = 1024;

    let gaps = ProtocolCodecApplication::uri_fidelity_gaps(&draft);
    assert!(gaps.contains(&"ech".to_string()), "{gaps:?}");
    assert!(gaps.contains(&"transport".to_string()), "{gaps:?}");
    // The vless exporter writes no `alpn` parameter, so the measured round
    // trip loses it: the gap list reports that fact instead of hiding it.
    assert!(gaps.contains(&"alpn".to_string()), "{gaps:?}");
    assert!(!gaps.contains(&"type".to_string()), "{gaps:?}");

    // trojan links do carry alpn, and the measurement must reflect that.
    let mut trojan = ProtocolDraft::new("trojan");
    trojan.name = "t".into();
    trojan.server = "t.example.com".into();
    trojan.port = 443;
    trojan.password = "pw".into();
    trojan.alpn = vec!["h2".into(), "http/1.1".into()];
    let gaps = ProtocolCodecApplication::uri_fidelity_gaps(&trojan);
    assert!(!gaps.contains(&"alpn".to_string()), "{gaps:?}");

    // SIP002 share links *do* carry the SIP003 plugin + opts; the measurement
    // proves it instead of assuming a gap.
    let mut ss = ProtocolDraft::new("ss");
    ss.name = "s".into();
    ss.server = "1.1.1.1".into();
    ss.port = 8388;
    ss.cipher = "aes-128-gcm".into();
    ss.password = "pw".into();
    ss.params.plugin.name = "shadow-tls".into();
    ss.params.plugin.opts.insert(
        "host".into(),
        infiltrator_contract::protocol_params_ext::PluginOptValue::Text("bing.com".into()),
    );
    ss.params.plugin.opts.insert(
        "password".into(),
        infiltrator_contract::protocol_params_ext::PluginOptValue::Text("pw".into()),
    );
    let gaps = ProtocolCodecApplication::uri_fidelity_gaps(&ss);
    assert!(!gaps.contains(&"plugin".to_string()), "{gaps:?}");
    let returned = ProtocolCodecApplication::draft_from_uri(
        &ProtocolCodecApplication::uri_from_draft(&ss).unwrap(),
    )
    .unwrap();
    assert_eq!(returned.params.plugin.name, "shadow-tls");
    assert_eq!(
        returned.params.plugin.opt("host").as_deref(),
        Some("bing.com")
    );
}

#[test]
fn unknown_field_audit_does_not_call_typed_keys_unknown() {
    let profile = r#"
proxies:
  - name: typed
    type: vless
    server: example.com
    port: 443
    uuid: uuid-1
    ech-opts:
      enable: true
    future-key: keep-me
"#;
    let (_output, audit) = ProtocolCodecApplication::audit_conversion(
        profile,
        NodeCodecFormat::ClashYaml,
        NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    assert_eq!(audit.unknown_fields, vec!["future-key".to_string()]);
}
