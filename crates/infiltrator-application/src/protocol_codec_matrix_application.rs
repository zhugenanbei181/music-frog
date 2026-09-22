//! DUAL-05-15: the shared protocol-codec regression matrix execution.
//!
//! Every row runs the real domain codec + shared application and reports what
//! it measured. The `planned` group 05 items (09 dialer chains, 10 dialer
//! cycle detection, 13 custom CA) are registered uncovered with their real
//! reason; nothing here is hard-coded to `true`.

use infiltrator_contract::protocol_fidelity::{NodeCodecFormat, ProtocolDraft};
use infiltrator_contract::protocol_matrix::{
    ProtocolCodecMatrixReport, ProtocolCodecMatrixScenario,
};

use crate::protocol_codec_application::ProtocolCodecApplication;

#[cfg(test)]
#[path = "protocol_codec_matrix_application_test.rs"]
mod protocol_codec_matrix_application_test;

/// The one shared executor both surfaces call.
pub struct ProtocolCodecMatrixApplication;

type Check = (bool, String);

fn closed(id: &str, item: &str, (passed, detail): Check) -> ProtocolCodecMatrixScenario {
    ProtocolCodecMatrixScenario {
        id: id.to_string(),
        item: item.to_string(),
        covered: true,
        passed,
        detail,
    }
}

fn planned(id: &str, item: &str, reason: &str) -> ProtocolCodecMatrixScenario {
    ProtocolCodecMatrixScenario {
        id: id.to_string(),
        item: item.to_string(),
        covered: false,
        passed: false,
        detail: reason.to_string(),
    }
}

fn wg_key() -> String {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string()
}

fn vless_draft() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "matrix-vless".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".into();
    draft
}

fn ss_draft() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "matrix-ss".into();
    draft.server = "1.1.1.1".into();
    draft.port = 8388;
    draft.cipher = "2022-blake3-aes-128-gcm".into();
    draft.password = "AAAAAAAAAAAAAAAAAAAAAA==".into();
    draft
}

/// Real profile round trip: upsert into an empty profile, parse it back and
/// project the node into a draft again.
fn profile_round_trip(draft: &ProtocolDraft) -> ProtocolDraft {
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", draft).unwrap();
    let nodes =
        ProtocolCodecApplication::parse_nodes(&commit.profile_yaml, NodeCodecFormat::ClashYaml)
            .unwrap();
    ProtocolCodecApplication::draft_from_node(&nodes[0])
}

impl ProtocolCodecMatrixApplication {
    /// Run every scenario deterministically (no time, no I/O, no randomness).
    pub fn run_deterministic_matrix() -> ProtocolCodecMatrixReport {
        let mut report = ProtocolCodecMatrixReport {
            scenarios: vec![
                closed(
                    "DUAL-05-01",
                    "Shadowsocks 2022 cipher family + PSK length",
                    check_cipher_family(),
                ),
                closed(
                    "DUAL-05-02",
                    "VLESS REALITY / Vision flow",
                    check_vless_reality(),
                ),
                closed(
                    "DUAL-05-03",
                    "TUIC congestion + Hysteria2 port hopping / obfs",
                    check_congestion_and_hopping(),
                ),
                closed(
                    "DUAL-05-04",
                    "WireGuard / AmneziaWG parameters",
                    check_wireguard(),
                ),
                closed(
                    "DUAL-05-05",
                    "Transports: WS 0-RTT / gRPC / H2 / HTTP / XHTTP",
                    check_transports(),
                ),
                closed("DUAL-05-06", "SIP003 plugin chain", check_plugin_chain()),
                closed("DUAL-05-07", "Native SSH SOCKS node", check_ssh()),
                closed(
                    "DUAL-05-08",
                    "AnyTLS + trojan-go ss-opts",
                    check_anytls_and_trojan_go(),
                ),
                planned(
                    "DUAL-05-09",
                    "dialer-proxy hop chain",
                    "no chain topology exists: `dialer-proxy` is a typed field with no resolver, projection or surface",
                ),
                planned(
                    "DUAL-05-10",
                    "dialer dependency cycle detection",
                    "`detect_group_cycles` covers proxy-group references only; no dialer graph is built anywhere",
                ),
                closed(
                    "DUAL-05-11",
                    "Smux / Yamux / H2Mux multiplexing",
                    check_smux(),
                ),
                closed(
                    "DUAL-05-12",
                    "TLS ECH + multi-version ALPN order",
                    check_ech_and_alpn(),
                ),
                planned(
                    "DUAL-05-13",
                    "custom CA / certificate whitelist",
                    "no custom-CA field, import port or fingerprint check exists in the workspace",
                ),
                closed(
                    "DUAL-05-14",
                    "lossless URI/JSON/Clash-YAML conversion",
                    check_lossless_upsert(),
                ),
            ],
        };
        let self_check = check_matrix_self(&report);
        report.scenarios.push(closed(
            "DUAL-05-15",
            "full parse/serialise regression matrix",
            self_check,
        ));
        report
    }
}

fn check_cipher_family() -> Check {
    let mut draft = ss_draft();
    let report = ProtocolCodecApplication::report(&draft);
    let family_ok =
        report.cipher_is_2022 && report.cipher_key_bytes == Some(16) && report.is_valid();
    let returned = profile_round_trip(&draft);
    let round_trip_ok = returned.cipher == draft.cipher && returned.password == draft.password;

    // A 32-byte PSK on a 16-byte cipher is refused by the shared validator.
    draft.password = wg_key();
    let refused = draft
        .validate()
        .iter()
        .any(|issue| issue.field == "password" && issue.message.contains("key size"));

    (
        family_ok && round_trip_ok && refused,
        format!("family={family_ok} round-trip={round_trip_ok} psk-refused={refused}"),
    )
}

fn check_vless_reality() -> Check {
    let mut draft = vless_draft();
    draft.flow = "xtls-rprx-vision".into();
    draft.tls = true;
    draft.reality.public_key = format!("PubKey1234567890{}", "A".repeat(27));
    draft.reality.short_id = "abcd1234".into();
    draft.reality.spider_x = "/spider".into();
    draft.reality.fingerprint = "chrome".into();
    let report = ProtocolCodecApplication::report(&draft);
    let chips_ok = report.flow_chip.is_some()
        && report.reality_chips.contains(&"Reality".to_string())
        && report.reality_chips.contains(&"sid:abcd1234".to_string());
    let returned = profile_round_trip(&draft);
    let round_trip_ok = returned.flow == draft.flow && returned.reality == draft.reality;

    let mut broken = draft.clone();
    broken.reality.short_id = "xyz".into();
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "reality.short-id");

    (
        chips_ok && round_trip_ok && refused,
        format!("chips={chips_ok} round-trip={round_trip_ok} sid-refused={refused}"),
    )
}

fn check_congestion_and_hopping() -> Check {
    let mut tuic = ProtocolDraft::new("tuic");
    tuic.name = "matrix-tuic".into();
    tuic.server = "tuic.example.com".into();
    tuic.port = 443;
    tuic.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".into();
    tuic.password = "pw".into();
    tuic.params.tuic.congestion_controller = "bbr".into();
    tuic.params.tuic.udp_relay_mode = "quic".into();
    tuic.params.tuic.reduce_rtt = true;
    tuic.params.tuic.recv_window = 4096;
    tuic.params.tuic.recv_window_conn = 1024;
    let tuic_report = ProtocolCodecApplication::report(&tuic);
    let tuic_ok = tuic_report.is_valid()
        && tuic_report
            .params
            .congestion_chips
            .contains(&"cc:bbr".to_string())
        && profile_round_trip(&tuic).params.tuic == tuic.params.tuic;

    let mut hy2 = ProtocolDraft::new("hysteria2");
    hy2.name = "matrix-hy2".into();
    hy2.server = "hy2.example.com".into();
    hy2.port = 443;
    hy2.password = "pw".into();
    hy2.params.hysteria2.ports = "20000-30000,8443".into();
    hy2.params.hysteria2.hop_interval = 30;
    hy2.params.hysteria2.obfs = "salamander".into();
    hy2.params.hysteria2.obfs_password = "obfs".into();
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &hy2).unwrap();
    let hops_ok = commit.profile_yaml.contains("ports: 20000-30000,8443")
        && commit.profile_yaml.contains("obfs: salamander");
    let round_trip_ok = profile_round_trip(&hy2).params.hysteria2 == hy2.params.hysteria2;

    // Honest masquerade fact: the pinned client-node schema has no such key, so
    // the studio must never fabricate one.
    let no_masquerade = !commit.profile_yaml.contains("masquerade");

    (
        tuic_ok && hops_ok && round_trip_ok && no_masquerade,
        format!(
            "tuic={tuic_ok} hopping={hops_ok} round-trip={round_trip_ok} no-masquerade={no_masquerade}"
        ),
    )
}

fn check_wireguard() -> Check {
    let mut draft = ProtocolDraft::new("wireguard");
    draft.name = "matrix-wg".into();
    draft.server = "wg.example.com".into();
    draft.port = 51820;
    draft.params.wireguard.private_key = wg_key();
    draft.params.wireguard.public_key = wg_key();
    draft.params.wireguard.pre_shared_key = wg_key();
    draft.params.wireguard.reserved = "AQID".into();
    draft.params.wireguard.reserved_is_base64 = true;
    draft.params.wireguard.ip = "172.16.0.2/32".into();
    draft.params.wireguard.mtu = 1420;
    draft.params.wireguard.persistent_keepalive = 25;
    draft.params.wireguard.workers = 2;
    draft.params.wireguard.amnezia.jc = Some(4);
    draft.params.wireguard.amnezia.jmin = Some(40);
    draft.params.wireguard.amnezia.jmax = Some(70);
    draft.params.wireguard.amnezia.h1 = Some(1);
    draft.params.wireguard.amnezia.h2 = Some(2);
    draft.params.wireguard.amnezia.h3 = Some(3);
    draft.params.wireguard.amnezia.h4 = Some(4);
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    let spellings_ok = commit.profile_yaml.contains("pre-shared-key:")
        && commit.profile_yaml.contains("amnezia-wg-option:")
        && commit.profile_yaml.contains("reserved: AQID");
    let round_trip_ok = profile_round_trip(&draft).params.wireguard == draft.params.wireguard;

    let mut broken = draft.clone();
    broken.params.wireguard.private_key = "short".into();
    broken.params.wireguard.amnezia.h1 = Some(2);
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "wireguard.private-key")
        && broken
            .validate()
            .iter()
            .any(|issue| issue.field == "wireguard.h1");

    (
        spellings_ok && round_trip_ok && refused,
        format!("keys={spellings_ok} round-trip={round_trip_ok} refused={refused}"),
    )
}

fn check_transports() -> Check {
    let mut draft = vless_draft();
    draft.tls = true;
    draft.params.transport.network = "ws".into();
    draft.params.transport.ws.path = "/ws".into();
    draft.params.transport.ws.set_host("cdn.example.com");
    draft.params.transport.ws.max_early_data = 2048;
    draft.params.transport.ws.early_data_header_name = "Sec-WebSocket-Protocol".into();
    draft.params.transport.grpc.service_name = "svc".into();
    draft.params.transport.packet_encoding = "xudp".into();
    let report = ProtocolCodecApplication::report(&draft);
    let chips_ok = report
        .params
        .transport_chips
        .contains(&"ws-0rtt:2048".to_string())
        && report
            .params
            .transport_chips
            .contains(&"grpc:svc".to_string());
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    let early_data_ok = commit.profile_yaml.contains("max-early-data: 2048");
    let round_trip_ok = profile_round_trip(&draft).params.transport == draft.params.transport;

    // XHTTP is typed + preserved, with an honest note that the pinned core has
    // no such transport; QUIC is likewise flagged as a TCP fallback.
    draft.params.transport.xhttp.mode = "stream-up".into();
    draft.params.transport.xhttp.path = "/x".into();
    let xhttp_note = ProtocolCodecApplication::report(&draft)
        .params
        .notes
        .iter()
        .any(|note| note.contains("v1.19.18"));
    draft.params.transport.network = "quic".into();
    let quic_note = ProtocolCodecApplication::report(&draft)
        .params
        .notes
        .iter()
        .any(|note| note.contains("TCP"));

    (
        chips_ok && early_data_ok && round_trip_ok && xhttp_note && quic_note,
        format!(
            "chips={chips_ok} 0rtt={early_data_ok} round-trip={round_trip_ok} notes={xhttp_note}/{quic_note}"
        ),
    )
}

fn check_plugin_chain() -> Check {
    let mut draft = ss_draft();
    draft.params.plugin.name = "shadow-tls".into();
    draft.params.plugin.opts.insert(
        "host".into(),
        infiltrator_contract::protocol_params_ext::PluginOptValue::Text("bing.com".into()),
    );
    draft.params.plugin.opts.insert(
        "password".into(),
        infiltrator_contract::protocol_params_ext::PluginOptValue::Text("pw".into()),
    );
    draft.params.plugin.opts.insert(
        "version".into(),
        infiltrator_contract::protocol_params_ext::PluginOptValue::Number(3),
    );
    let report = ProtocolCodecApplication::report(&draft);
    let chips_ok = report
        .params
        .plugin_chips
        .contains(&"plugin:shadow-tls".to_string());
    let round_trip_ok = profile_round_trip(&draft).params.plugin == draft.params.plugin;
    // SIP002 links carry the plugin *and* its options. The link is text-only,
    // so a numeric option returns as its exact text (`3` -> `"3"`); compare the
    // option text, not the enum variant, and say so.
    let uri_returned = ProtocolCodecApplication::draft_from_uri(
        &ProtocolCodecApplication::uri_from_draft(&draft).unwrap(),
    )
    .unwrap();
    let uri_ok = uri_returned.params.plugin.name == draft.params.plugin.name
        && uri_returned.params.plugin.opt("host") == draft.params.plugin.opt("host")
        && uri_returned.params.plugin.opt("password") == draft.params.plugin.opt("password")
        && uri_returned.params.plugin.opt("version") == draft.params.plugin.opt("version");

    // The interlock is real: shadow-tls without host/password is refused.
    let mut broken = draft.clone();
    broken.params.plugin.opts.clear();
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "plugin.host")
        && broken
            .validate()
            .iter()
            .any(|issue| issue.field == "plugin.password");

    (
        chips_ok && round_trip_ok && uri_ok && refused,
        format!("chips={chips_ok} yaml={round_trip_ok} uri={uri_ok} refused={refused}"),
    )
}

fn check_ssh() -> Check {
    let uri = "ssh://root:pw@ssh.example.com:22?private_key=key-material&passphrase=phrase&host_key_algorithms=ssh-ed25519,rsa-sha2-256#Matrix-SSH";
    let draft = ProtocolCodecApplication::draft_from_uri(uri).unwrap();
    let family_ok = draft.family() == infiltrator_contract::protocol_fidelity::ProtocolFamily::Ssh;
    let ssh_ok = draft.params.ssh.username == "root"
        && draft.params.ssh.private_key == "key-material"
        && draft.params.ssh.passphrase == "phrase"
        && draft.params.ssh.host_key_algorithms.len() == 2;
    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    let key_ok = commit
        .profile_yaml
        .contains("private-key-passphrase: phrase");
    let round_trip_ok = profile_round_trip(&draft).params.ssh == draft.params.ssh;

    let mut missing = draft.clone();
    missing.params.ssh.username = String::new();
    let refused = missing
        .validate()
        .iter()
        .any(|issue| issue.field == "ssh.username");

    (
        family_ok && ssh_ok && key_ok && round_trip_ok && refused,
        format!(
            "family={family_ok} fields={ssh_ok} key-name={key_ok} round-trip={round_trip_ok} refused={refused}"
        ),
    )
}

fn check_anytls_and_trojan_go() -> Check {
    let mut anytls = ProtocolDraft::new("anytls");
    anytls.name = "matrix-anytls".into();
    anytls.server = "anytls.example.com".into();
    anytls.port = 443;
    anytls.password = "pw".into();
    anytls.params.anytls.idle_session_timeout = 30000;
    anytls.params.anytls.idle_session_check_interval = 15000;
    anytls.params.anytls.min_idle_session = 2;
    let anytls_ok = profile_round_trip(&anytls).params.anytls == anytls.params.anytls;

    let mut trojan = ProtocolDraft::new("trojan");
    trojan.name = "matrix-trojan".into();
    trojan.server = "trojan.example.com".into();
    trojan.port = 443;
    trojan.password = "pw".into();
    trojan.params.trojan_ss.enabled = true;
    trojan.params.trojan_ss.method = "aes-128-gcm".into();
    trojan.params.trojan_ss.password = "ss-pw".into();
    let report = ProtocolCodecApplication::report(&trojan);
    let chips_ok = report
        .params
        .trojan_chips
        .contains(&"trojan-go:ss".to_string());
    let round_trip_ok = profile_round_trip(&trojan).params.trojan_ss == trojan.params.trojan_ss;

    let mut broken = trojan.clone();
    broken.params.trojan_ss.method = "rot13".into();
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "trojan.ss-opts.method");

    (
        anytls_ok && chips_ok && round_trip_ok && refused,
        format!("anytls={anytls_ok} chips={chips_ok} round-trip={round_trip_ok} refused={refused}"),
    )
}

fn check_smux() -> Check {
    let mut draft = vless_draft();
    draft.smux.enabled = true;
    draft.smux.protocol = "yamux".into();
    draft.smux.max_connections = 8;
    draft.smux.min_streams = 2;
    draft.smux.max_streams = 16;
    draft.smux.padding = true;
    let report = ProtocolCodecApplication::report(&draft);
    let chips_ok = report.smux_chips.contains(&"mux:yamux".to_string());
    let round_trip_ok = profile_round_trip(&draft).smux == draft.smux;
    let gap_ok = ProtocolCodecApplication::uri_fidelity_gaps(&draft).contains(&"smux".to_string());
    (
        chips_ok && round_trip_ok && gap_ok,
        format!("chips={chips_ok} round-trip={round_trip_ok} uri-gap={gap_ok}"),
    )
}

fn check_ech_and_alpn() -> Check {
    let mut draft = vless_draft();
    draft.tls = true;
    draft.alpn = vec!["h2".into(), "http/1.1".into()];
    draft.params.ech.enabled = true;
    draft.params.ech.config = "AQIDBA==".into();
    let report = ProtocolCodecApplication::report(&draft);
    let chips_ok = report.params.ech_chip.as_deref() == Some("ech:inline")
        && report.params.alpn_chips
            == vec!["alpn[0]:h2".to_string(), "alpn[1]:http/1.1".to_string()];

    let commit =
        ProtocolCodecApplication::upsert_draft_into_profile("proxies: []\n", &draft).unwrap();
    let ech_ok = commit.profile_yaml.contains("ech-opts:")
        && commit.profile_yaml.contains("config: AQIDBA==");
    let round_trip_ok = profile_round_trip(&draft).params.ech == draft.params.ech;

    let mut duplicate = draft.clone();
    duplicate.alpn = vec!["h2".into(), "h2".into()];
    let refused = duplicate
        .validate()
        .iter()
        .any(|issue| issue.field == "alpn");
    let gap_ok = ProtocolCodecApplication::uri_fidelity_gaps(&draft).contains(&"ech".to_string());

    (
        chips_ok && ech_ok && round_trip_ok && refused && gap_ok,
        format!(
            "chips={chips_ok} yaml={ech_ok} round-trip={round_trip_ok} refused={refused} uri-gap={gap_ok}"
        ),
    )
}

fn check_lossless_upsert() -> Check {
    let profile = r#"
mode: rule
proxies:
  - name: keep
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: aes-128-gcm
    password: pw
    future-flag: 7
proxy-groups:
  - name: PROXY
    type: select
    proxies: [keep]
rules:
  - MATCH,PROXY
"#;
    let mut draft = ss_draft();
    draft.name = "keep".into();
    let commit = ProtocolCodecApplication::upsert_draft_into_profile(profile, &draft).unwrap();
    let preserved_ok = commit.is_structure_preserving()
        && commit.audit.unknown_fields == vec!["future-flag".to_string()];
    let sections_ok = commit.profile_yaml.contains("proxy-groups:")
        && commit.profile_yaml.contains("MATCH,PROXY");

    // The legacy round trip really is lossy; the audit says so.
    let (_output, audit) = ProtocolCodecApplication::audit_conversion(
        profile,
        NodeCodecFormat::ClashYaml,
        NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    let audit_honest = !audit.structure_preserved && !audit.lossless;

    // A blocking draft can never be spliced.
    let mut invalid = draft.clone();
    invalid.password = String::new();
    let refused = ProtocolCodecApplication::upsert_draft_into_profile(profile, &invalid).is_err();

    (
        preserved_ok && sections_ok && audit_honest && refused,
        format!(
            "preserved={preserved_ok} sections={sections_ok} audit-honest={audit_honest} refused={refused}"
        ),
    )
}

/// DUAL-05-15 self-check: the matrix itself must cover every closed item and
/// name every planned one. Computed from the real report, not a constant.
fn check_matrix_self(report: &ProtocolCodecMatrixReport) -> Check {
    let covered: Vec<&str> = report
        .scenarios
        .iter()
        .filter(|row| row.covered)
        .map(|row| row.id.as_str())
        .collect();
    let expected_covered = [
        "DUAL-05-01",
        "DUAL-05-02",
        "DUAL-05-03",
        "DUAL-05-04",
        "DUAL-05-05",
        "DUAL-05-06",
        "DUAL-05-07",
        "DUAL-05-08",
        "DUAL-05-11",
        "DUAL-05-12",
        "DUAL-05-14",
    ];
    let missing: Vec<&str> = expected_covered
        .iter()
        .copied()
        .filter(|id| !covered.contains(id))
        .collect();
    let planned = report.not_covered_ids();
    let expected_planned = ["DUAL-05-09", "DUAL-05-10", "DUAL-05-13"];
    let planned_ok = expected_planned.iter().all(|id| planned.contains(id))
        && planned.len() == expected_planned.len();
    let no_failures = report.all_covered_passed();
    let details_ok = report
        .scenarios
        .iter()
        .all(|row| !row.detail.trim().is_empty());

    (
        missing.is_empty() && planned_ok && no_failures && details_ok,
        format!(
            "covered={}/{} missing={missing:?} planned={planned:?} failures={:?}",
            report.covered_passed_count(),
            report.covered_count(),
            report.failed_ids()
        ),
    )
}
