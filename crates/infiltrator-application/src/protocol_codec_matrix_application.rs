//! DUAL-05-15: shared protocol draft and engine-codec regression matrix.
//!
//! The application passes opaque profile bytes and typed draft operations to
//! the selected codec. It does not parse or patch YAML itself.

use crate::certificate_authority_application::CertificateAuthorityApplication;
use crate::dialer_chain_application::DialerChainApplication;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_matrix::{
    ProtocolCodecMatrixReport, ProtocolCodecMatrixScenario,
};
use infiltrator_contract::protocol_params_ext::PluginOptValue;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_domain::protocol_draft_codec::uri_fidelity_gaps;
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationMutation, ProfileConfigurationReadout, ProfileConfigurationUpdate,
};
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSourceId,
};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

#[cfg(test)]
#[path = "protocol_codec_matrix_application_test.rs"]
mod protocol_codec_matrix_application_test;

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

fn draft(family: &str, name: &str) -> ProtocolDraft {
    let mut draft = ProtocolDraft::new(family);
    draft.name = name.to_owned();
    draft.server = "example.com".to_owned();
    draft.port = 443;
    draft.password = "pw".to_owned();
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    draft
}

fn matrix_identity(codec: &dyn ProfileConfigurationCodecPort) -> ProfileDocumentIdentity {
    let scope = codec.scope();
    ProfileDocumentIdentity::new(
        ProfileId::new("protocol-matrix").expect("matrix profile ID"),
        scope.engine_id,
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("protocol-matrix-source").expect("matrix source ID"),
        ),
        ProfileDocumentRevision::new("protocol-matrix-revision").expect("matrix revision"),
        scope.format_id,
        scope.schema_version,
    )
}

fn apply_protocol(
    codec: &dyn ProfileConfigurationCodecPort,
    identity: &ProfileDocumentIdentity,
    raw_bytes: &[u8],
    draft: ProtocolDraft,
) -> Result<ProfileConfigurationMutation, PortError> {
    codec.apply_configuration(
        identity,
        raw_bytes,
        ProfileConfigurationUpdate::UpsertProtocolNode(draft),
    )
}

fn round_trip(
    codec: &dyn ProfileConfigurationCodecPort,
    draft: &ProtocolDraft,
) -> Result<ProtocolDraft, String> {
    if !draft.report().is_valid() {
        return Err("draft is invalid".to_owned());
    }
    let identity = matrix_identity(codec);
    let mutation = apply_protocol(codec, &identity, b"proxies: []\n", draft.clone())
        .map_err(|error| error.to_string())?;
    let ProfileConfigurationReadout::ProtocolNodeMutation(readback) = mutation.readback else {
        return Err("codec returned the wrong protocol mutation readback".to_owned());
    };
    readback
        .nodes
        .into_iter()
        .find(|node| node.name.trim() == draft.name.trim())
        .ok_or_else(|| "codec did not return the edited node".to_owned())
}

fn round_trip_check<F>(
    codec: &dyn ProfileConfigurationCodecPort,
    draft: &ProtocolDraft,
    check: F,
) -> Check
where
    F: FnOnce(&ProtocolDraft) -> bool,
{
    match round_trip(codec, draft) {
        Ok(returned) => {
            let passed = check(&returned);
            (passed, format!("typed-readback={passed}"))
        }
        Err(error) => (false, format!("typed-readback failed: {error}")),
    }
}

/// One deterministic matrix invocation for the selected engine adapter.
pub struct ProtocolCodecMatrixApplication;

impl ProtocolCodecMatrixApplication {
    pub fn run_deterministic_matrix(
        codec: Arc<dyn ProfileConfigurationCodecPort>,
    ) -> ProtocolCodecMatrixReport {
        let codec: &dyn ProfileConfigurationCodecPort = codec.as_ref();
        let identity = matrix_identity(codec);
        let mut scenarios = vec![
            closed(
                "DUAL-05-01",
                "Shadowsocks cipher family and PSK validation",
                check_cipher(codec),
            ),
            closed(
                "DUAL-05-02",
                "VLESS REALITY and Vision typed readback",
                check_reality(codec),
            ),
            closed(
                "DUAL-05-03",
                "TUIC and Hysteria2 typed parameters",
                check_tuic_hysteria(codec),
            ),
            closed(
                "DUAL-05-04",
                "WireGuard and AmneziaWG typed parameters",
                check_wireguard(codec),
            ),
            closed(
                "DUAL-05-05",
                "Transport typed readback and fidelity notes",
                check_transports(codec),
            ),
            closed(
                "DUAL-05-06",
                "SIP003 plugin typed readback",
                check_plugin(codec),
            ),
            closed(
                "DUAL-05-07",
                "SSH typed readback and validation",
                check_ssh(codec),
            ),
            closed(
                "DUAL-05-08",
                "AnyTLS and Trojan typed readback",
                check_anytls_trojan(codec),
            ),
            closed(
                "DUAL-05-09",
                "Dialer hop typed readback",
                check_dialer_hop(codec),
            ),
            closed(
                "DUAL-05-10",
                "Dialer chain and cycle analysis",
                check_dialer_cycles(),
            ),
            closed(
                "DUAL-05-11",
                "Multiplexing typed readback",
                check_smux(codec),
            ),
            closed(
                "DUAL-05-12",
                "ECH and ALPN typed readback",
                check_ech(codec),
            ),
            closed(
                "DUAL-05-13",
                "CA path carrier and inline capability distinction",
                check_custom_ca(codec, &identity),
            ),
            closed(
                "DUAL-05-14",
                "Protocol upsert preserves unknown fields and exact sections",
                check_lossless_upsert(codec, &identity),
            ),
        ];
        let interim = ProtocolCodecMatrixReport {
            scenarios: scenarios.clone(),
        };
        scenarios.push(closed(
            "DUAL-05-15",
            "Matrix self-coverage",
            check_matrix_self(&interim),
        ));
        ProtocolCodecMatrixReport { scenarios }
    }
}

fn check_cipher(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("ss", "matrix-ss");
    node.cipher = "2022-blake3-aes-128-gcm".to_owned();
    node.password = "AAAAAAAAAAAAAAAAAAAAAA==".to_owned();
    let report = node.report();
    let family_ok =
        report.cipher_is_2022 && report.cipher_key_bytes == Some(16) && report.is_valid();
    let returned = round_trip_check(codec, &node, |returned| {
        returned.cipher == node.cipher && returned.password == node.password
    });
    let mut broken = node;
    broken.password = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned();
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "password" && issue.message.contains("key size"));
    (
        family_ok && returned.0 && refused,
        format!(
            "family={family_ok} readback={} psk-refused={refused}",
            returned.0
        ),
    )
}

fn check_reality(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("vless", "matrix-vless");
    node.flow = "xtls-rprx-vision".to_owned();
    node.tls = true;
    node.reality.public_key = format!("PubKey1234567890{}", "A".repeat(27));
    node.reality.short_id = "abcd1234".to_owned();
    node.reality.spider_x = "/spider".to_owned();
    node.reality.fingerprint = "chrome".to_owned();
    let report = node.report();
    let chips_ok = report.flow_chip.is_some()
        && report.reality_chips.contains(&"Reality".to_owned())
        && report.reality_chips.contains(&"sid:abcd1234".to_owned());
    let returned = round_trip_check(codec, &node, |returned| {
        returned.flow == node.flow && returned.reality == node.reality
    });
    let mut broken = node;
    broken.reality.short_id = "xyz".to_owned();
    let refused = broken
        .validate()
        .iter()
        .any(|issue| issue.field == "reality.short-id");
    (
        chips_ok && returned.0 && refused,
        format!(
            "chips={chips_ok} readback={} invalid-refused={refused}",
            returned.0
        ),
    )
}

fn check_tuic_hysteria(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut tuic = draft("tuic", "matrix-tuic");
    tuic.params.tuic.congestion_controller = "bbr".to_owned();
    tuic.params.tuic.udp_relay_mode = "native".to_owned();
    tuic.params.tuic.heartbeat_interval = 10;
    let tuic_readback = round_trip_check(codec, &tuic, |returned| {
        returned.params.tuic == tuic.params.tuic
    });

    let mut hy2 = draft("hysteria2", "matrix-hy2");
    hy2.params.hysteria2.ports = "20000-30000,8443".to_owned();
    hy2.params.hysteria2.hop_interval = 30;
    hy2.params.hysteria2.obfs = "salamander".to_owned();
    hy2.params.hysteria2.obfs_password = "obfs".to_owned();
    let hy2_readback = round_trip_check(codec, &hy2, |returned| {
        returned.params.hysteria2 == hy2.params.hysteria2
    });
    (
        tuic_readback.0 && hy2_readback.0,
        format!("tuic={} hysteria2={}", tuic_readback.0, hy2_readback.0),
    )
}

fn check_wireguard(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("wireguard", "matrix-wg");
    node.params.wireguard.private_key = format!("{}=", "A".repeat(43));
    node.params.wireguard.public_key = format!("{}=", "B".repeat(43));
    node.params.wireguard.pre_shared_key = format!("{}=", "C".repeat(43));
    node.params.wireguard.reserved = "1,2,3".to_owned();
    node.params.wireguard.ip = "10.0.0.2/32".to_owned();
    node.params.wireguard.allowed_ips = vec!["0.0.0.0/0".to_owned()];
    node.params.wireguard.amnezia.jc = Some(4);
    let roundtrip = match round_trip(codec, &node) {
        Ok(returned) => {
            let passed = returned.params.wireguard == node.params.wireguard;
            let detail = if passed {
                "typed-readback=true".to_owned()
            } else {
                format!(
                    "typed-readback=false expected={:?} actual={:?}",
                    node.params.wireguard, returned.params.wireguard
                )
            };
            (passed, detail)
        }
        Err(error) => (false, format!("typed-readback failed: {error}")),
    };
    let mut invalid = node;
    invalid.params.wireguard.private_key = "short".to_owned();
    let refused = invalid
        .validate()
        .iter()
        .any(|issue| issue.field == "wireguard.private-key");
    (
        roundtrip.0 && refused,
        format!("{} invalid-refused={refused}", roundtrip.1),
    )
}

fn check_transports(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("vless", "matrix-transport");
    node.params.transport.network = "ws".to_owned();
    node.params.transport.ws.path = "/ws".to_owned();
    node.params.transport.ws.max_early_data = 2048;
    node.params.transport.grpc.service_name = "svc".to_owned();
    let readback = round_trip_check(codec, &node, |returned| {
        returned.params.transport == node.params.transport
    });
    node.params.transport.xhttp.mode = "stream-up".to_owned();
    node.params.transport.xhttp.path = "/x".to_owned();
    let xhttp_note = node
        .report()
        .params
        .notes
        .iter()
        .any(|note| note.contains("v1.19.18"));
    node.params.transport.network = "quic".to_owned();
    let quic_note = node
        .report()
        .params
        .notes
        .iter()
        .any(|note| note.contains("TCP"));
    (
        readback.0 && xhttp_note && quic_note,
        format!(
            "readback={} xhttp-note={xhttp_note} quic-note={quic_note}",
            readback.0
        ),
    )
}

fn check_plugin(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("ss", "matrix-plugin");
    node.cipher = "aes-128-gcm".to_owned();
    node.params.plugin.name = "obfs".to_owned();
    node.params
        .plugin
        .opts
        .insert("mode".to_owned(), PluginOptValue::Text("tls".to_owned()));
    round_trip_check(codec, &node, |returned| {
        returned.params.plugin == node.params.plugin
    })
}

fn check_ssh(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("ssh", "matrix-ssh");
    node.params.ssh.username = "root".to_owned();
    node.params.ssh.private_key = "key-material".to_owned();
    node.params.ssh.passphrase = "phrase".to_owned();
    node.params.ssh.host_key_algorithms = vec!["ssh-ed25519".to_owned()];
    round_trip_check(codec, &node, |returned| {
        returned.params.ssh == node.params.ssh
    })
}

fn check_anytls_trojan(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut anytls = draft("anytls", "matrix-anytls");
    anytls.params.anytls.idle_session_timeout = 30_000;
    anytls.params.anytls.min_idle_session = 2;
    let anytls_roundtrip = round_trip_check(codec, &anytls, |returned| {
        returned.params.anytls == anytls.params.anytls
    });
    let mut trojan = draft("trojan", "matrix-trojan");
    trojan.params.trojan_ss.enabled = true;
    trojan.params.trojan_ss.method = "chacha20-ietf-poly1305".to_owned();
    trojan.params.trojan_ss.password = "pw".to_owned();
    let trojan_roundtrip = round_trip_check(codec, &trojan, |returned| {
        returned.params.trojan_ss == trojan.params.trojan_ss
    });
    (
        anytls_roundtrip.0 && trojan_roundtrip.0,
        format!(
            "anytls={} trojan={}",
            anytls_roundtrip.0, trojan_roundtrip.0
        ),
    )
}

fn check_dialer_hop(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("vless", "matrix-hop");
    node.dialer_proxy = "gateway".to_owned();
    let roundtrip = round_trip_check(codec, &node, |returned| returned.dialer_proxy == "gateway");
    let uri_gap = uri_fidelity_gaps(&node).contains(&"dialer-proxy".to_owned());
    (
        roundtrip.0 && uri_gap,
        format!("readback={} uri-gap={uri_gap}", roundtrip.0),
    )
}

fn check_dialer_cycles() -> Check {
    let profile = "proxies:\n  - {name: a, type: ss, server: 1.1.1.1, port: 443, cipher: aes-128-gcm, password: pw, dialer-proxy: b}\n  - {name: b, type: ss, server: 2.2.2.2, port: 443, cipher: aes-128-gcm, password: pw, dialer-proxy: a}\n";
    let report = DialerChainApplication::analyze_profile(profile).expect("dialer report");
    let loop_found = !report.loops.is_empty();
    (loop_found, format!("cycle-detected={loop_found}"))
}

fn check_smux(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("ss", "matrix-smux");
    node.cipher = "aes-128-gcm".to_owned();
    node.smux.enabled = true;
    node.smux.protocol = "yamux".to_owned();
    node.smux.max_connections = 8;
    node.smux.min_streams = 2;
    node.smux.max_streams = 16;
    node.smux.padding = true;
    let roundtrip = round_trip_check(codec, &node, |returned| returned.smux == node.smux);
    let uri_gap = uri_fidelity_gaps(&node).contains(&"smux".to_owned());
    (
        roundtrip.0 && uri_gap,
        format!("readback={} uri-gap={uri_gap}", roundtrip.0),
    )
}

fn check_ech(codec: &dyn ProfileConfigurationCodecPort) -> Check {
    let mut node = draft("vless", "matrix-ech");
    node.params.ech.enabled = true;
    node.params.ech.config = "AQIDBA==".to_owned();
    node.alpn = vec!["h2".to_owned(), "http/1.1".to_owned()];
    let report = node.report();
    let chips = report.params.ech_chip.as_deref() == Some("ech:inline")
        && report.params.alpn_chips.len() == 2;
    let roundtrip = round_trip_check(codec, &node, |returned| {
        returned.params.ech == node.params.ech && returned.alpn == node.alpn
    });
    (
        chips && roundtrip.0,
        format!("chips={chips} readback={}", roundtrip.0),
    )
}

fn check_custom_ca(
    codec: &dyn ProfileConfigurationCodecPort,
    identity: &ProfileDocumentIdentity,
) -> Check {
    let path_params = TlsTrustParams {
        ca_path: "/etc/ssl/private/custom-ca.pem".to_owned(),
        ..TlsTrustParams::default()
    };
    let path_result = codec.apply_configuration(
        identity,
        b"mode: rule\ntls:\n  certificate: /etc/ssl/cert.pem\nrules:\n  - MATCH,DIRECT\n",
        ProfileConfigurationUpdate::TrustAnchors(path_params),
    );
    let carrier_ok = path_result.is_ok_and(|mutation| {
        matches!(mutation.readback, ProfileConfigurationReadout::TrustAnchors(ref readback)
            if readback.ca_path == "/etc/ssl/private/custom-ca.pem")
            && String::from_utf8(mutation.document_bytes).is_ok_and(|yaml| {
                yaml.contains("custom-certifactes")
                    && yaml.contains("certificate: /etc/ssl/cert.pem")
                    && yaml.contains("MATCH,DIRECT")
            })
    });
    let inline = TlsTrustParams {
        ca_str: "-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----".to_owned(),
        ..TlsTrustParams::default()
    };
    let inline_unsupported = matches!(
        codec.apply_configuration(
            identity,
            b"mode: rule\n",
            ProfileConfigurationUpdate::TrustAnchors(inline.clone()),
        ),
        Err(PortError::Unsupported { .. })
    );
    let inline_report = CertificateAuthorityApplication::resolve(&inline, None);
    let inline_report_is_honest = inline_report
        .notes
        .iter()
        .any(|note| note.contains("no `ca-str` carrier"));
    let mut fingerprint = TlsTrustParams::default();
    fingerprint.fingerprint = "00".repeat(32);
    let fingerprint_is_not_anchor = codec
        .apply_configuration(
            identity,
            b"mode: rule\n",
            ProfileConfigurationUpdate::TrustAnchors(fingerprint),
        )
        .is_err();
    (
        carrier_ok && inline_unsupported && inline_report_is_honest && fingerprint_is_not_anchor,
        format!(
            "path-carrier={carrier_ok} inline-unsupported={inline_unsupported} inline-report={inline_report_is_honest} fingerprint-separated={fingerprint_is_not_anchor}"
        ),
    )
}

fn check_lossless_upsert(
    codec: &dyn ProfileConfigurationCodecPort,
    identity: &ProfileDocumentIdentity,
) -> Check {
    let profile = b"mode: rule\nproxies:\n  - name: keep\n    type: ss\n    server: 1.1.1.1\n    port: 8388\n    cipher: aes-128-gcm\n    password: old\n    future-flag: 7\nproxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [keep]\nrules:\n  - MATCH,PROXY\n";
    let mut node = draft("ss", "keep");
    node.cipher = "aes-128-gcm".to_owned();
    let mutation = apply_protocol(codec, identity, profile, node);
    let (audit_ok, sections_ok) = match mutation {
        Ok(mutation) => {
            let audit_ok = matches!(
                &mutation.readback,
                ProfileConfigurationReadout::ProtocolNodeMutation(readback)
                    if readback.audit.structure_preserved
                        && readback.audit.unknown_fields == vec!["future-flag".to_owned()]
            );
            let sections_ok = String::from_utf8(mutation.document_bytes)
                .is_ok_and(|yaml| yaml.contains("proxy-groups:") && yaml.contains("MATCH,PROXY"));
            (audit_ok, sections_ok)
        }
        Err(_) => (false, false),
    };
    let invalid_rejected = {
        let mut invalid = draft("ss", "invalid");
        invalid.password.clear();
        apply_protocol(codec, identity, profile, invalid).is_err()
    };
    (
        audit_ok && sections_ok && invalid_rejected,
        format!("audit={audit_ok} sections={sections_ok} invalid-rejected={invalid_rejected}"),
    )
}

fn check_matrix_self(report: &ProtocolCodecMatrixReport) -> Check {
    let covered: Vec<&str> = report
        .scenarios
        .iter()
        .filter(|row| row.covered)
        .map(|row| row.id.as_str())
        .collect();
    let expected = [
        "DUAL-05-01",
        "DUAL-05-02",
        "DUAL-05-03",
        "DUAL-05-04",
        "DUAL-05-05",
        "DUAL-05-06",
        "DUAL-05-07",
        "DUAL-05-08",
        "DUAL-05-09",
        "DUAL-05-10",
        "DUAL-05-11",
        "DUAL-05-12",
        "DUAL-05-13",
        "DUAL-05-14",
    ];
    let passed = covered == expected;
    (
        passed,
        format!("covered={}/{}", covered.len(), expected.len()),
    )
}
