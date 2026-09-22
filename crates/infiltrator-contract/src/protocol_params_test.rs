//! DUAL-05 typed parameter-block tests (05-03…05-08, 05-12).

use crate::protocol_fidelity::ProtocolDraft;
use crate::protocol_params::{
    EchParams, Hysteria2Params, ProtocolParams, ProtocolParamsReport, TuicParams,
};
use crate::protocol_params_ext::{
    AnyTlsParams, PluginOptValue, Sip003Plugin, SshParams, TransportParams, TrojanSsParams,
    WireGuardParams, WsOptsParams,
};

fn issue_fields(issues: &[crate::protocol_fidelity::ProtocolIssue]) -> Vec<&str> {
    issues.iter().map(|issue| issue.field.as_str()).collect()
}

fn vless() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "n".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.uuid = "uuid-value".into();
    draft
}

#[test]
fn ech_is_typed_validated_and_honest_about_dns_resolution() {
    let params = EchParams {
        enabled: true,
        config: String::new(),
    };
    assert_eq!(params.chips(), vec!["ech:dns".to_string()]);
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");

    // 3 bytes base64 = "AQID" padding-free; the validator accepts any base64
    // blob because the pinned core decodes the list itself.
    let params = EchParams {
        enabled: true,
        config: "AQIDBA==".into(),
    };
    assert_eq!(params.chips(), vec!["ech:inline".to_string()]);
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");

    let broken = EchParams {
        enabled: true,
        config: "not base64!!".into(),
    };
    let mut issues = Vec::new();
    broken.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["ech.config"]);

    // A config without `enable: true` is ignored by the core; the shared
    // report says so instead of pretending it is active.
    let off = EchParams {
        enabled: false,
        config: "AQIDBA==".into(),
    };
    let mut issues = Vec::new();
    off.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["ech.enable"]);
    assert!(off.chips().is_empty());
}

#[test]
fn ech_gating_and_chips_reach_the_draft_report() {
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "s".into();
    draft.server = "1.1.1.1".into();
    draft.port = 8388;
    draft.password = "pw".into();
    draft.params.ech.enabled = true;
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "ech.unsupported"),
        "{:?}",
        report.issues
    );
    assert!(report.params.ech_chip.is_none());

    let mut draft = vless();
    draft.params.ech.enabled = true;
    draft.params.ech.config = "AQIDBA==".into();
    let report = draft.report();
    assert!(report.is_valid(), "{:?}", report.issues);
    assert_eq!(report.params.ech_chip.as_deref(), Some("ech:inline"));
}

#[test]
fn tuic_congestion_parameters_are_typed_and_chipped() {
    let params = TuicParams {
        congestion_controller: "bbr".into(),
        udp_relay_mode: "native".into(),
        reduce_rtt: true,
        heartbeat_interval: 10000,
        request_timeout: 8000,
        recv_window_conn: 1024,
        recv_window: 4096,
        disable_sni: false,
    };
    assert!(params.congestion_controller_known());
    assert!(params.udp_relay_mode_known());
    assert_eq!(
        params.chips(),
        vec![
            "cc:bbr".to_string(),
            "udp:native".to_string(),
            "reduce-rtt".to_string(),
            "hb:10000ms".to_string(),
            "recv:4096".to_string(),
        ]
    );

    let broken = TuicParams {
        recv_window_conn: 9000,
        recv_window: 4096,
        ..TuicParams::default()
    };
    let mut issues = Vec::new();
    broken.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["tuic.recv-window"]);

    // Unknown values are preserved with an honest note (mihomo silently falls
    // back to `native`), never silently rewritten in the draft.
    let unknown = TuicParams {
        congestion_controller: "vegas".into(),
        udp_relay_mode: "udp".into(),
        ..TuicParams::default()
    };
    let mut issues = Vec::new();
    unknown.validate(&mut issues);
    assert!(issues.is_empty());
    let mut notes = Vec::new();
    unknown.notes(&mut notes);
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(notes[0].contains("vegas"));
    assert!(notes[1].contains("native"));
}

#[test]
fn hysteria2_port_hopping_and_obfs_are_validated() {
    let params = Hysteria2Params {
        ports: "20000-30000,8443".into(),
        hop_interval: 30,
        obfs: "salamander".into(),
        obfs_password: "pw".into(),
        up: "100 Mbps".into(),
        down: "200".into(),
        cwnd: 32,
        udp_mtu: 0,
    };
    assert!(params.ports_valid());
    assert_eq!(params.port_count(), 10002);
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let chips = params.chips();
    assert!(chips.contains(&"ports:20000-30000,8443".to_string()));
    assert!(chips.contains(&"hop:30s".to_string()));
    assert!(chips.contains(&"obfs:salamander".to_string()));
    assert!(chips.contains(&"cwnd:32".to_string()));

    let broken = Hysteria2Params {
        ports: "30000-20000".into(),
        obfs: "quantum".into(),
        obfs_password: String::new(),
        up: "fast".into(),
        ..Hysteria2Params::default()
    };
    let mut issues = Vec::new();
    broken.validate(&mut issues);
    let names = issue_fields(&issues);
    assert!(names.contains(&"hysteria2.ports"), "{names:?}");
    assert!(names.contains(&"hysteria2.obfs"), "{names:?}");
    assert!(names.contains(&"hysteria2.obfs-password"), "{names:?}");
    assert!(names.contains(&"hysteria2.up"), "{names:?}");

    // hop-interval without ports and the core's silent clamp both surface.
    let no_ports = Hysteria2Params {
        hop_interval: 3,
        ..Hysteria2Params::default()
    };
    let mut issues = Vec::new();
    no_ports.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["hysteria2.hop-interval"]);
    let mut notes = Vec::new();
    no_ports.notes(&mut notes);
    assert!(notes[0].contains("5s"), "{notes:?}");

    // Ping-pong: a non-hysteria2 draft refuses the block.
    let mut draft = vless();
    draft.params.hysteria2.ports = "1000-2000".into();
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "hysteria2.unsupported")
    );
}

#[test]
fn wireguard_keys_reserved_and_amnezia_are_typed() {
    // 32-byte base64 key.
    let key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let params = WireGuardParams {
        private_key: key.into(),
        public_key: key.into(),
        pre_shared_key: String::new(),
        reserved: "1,2,3".into(),
        reserved_is_base64: false,
        ip: "172.16.0.2/32".into(),
        ipv6: String::new(),
        mtu: 1420,
        dns: vec!["1.1.1.1".into()],
        workers: 2,
        persistent_keepalive: 25,
        allowed_ips: vec!["0.0.0.0/0".into()],
        remote_dns_resolve: true,
        amnezia: crate::protocol_params_ext::AmneziaWgParams {
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
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let chips = params.chips();
    assert!(chips.contains(&"wg:key".to_string()));
    assert!(chips.contains(&"wg:reserved(list)".to_string()));
    assert!(chips.contains(&"mtu:1420".to_string()));
    assert!(chips.contains(&"awg:jc=4".to_string()));

    let bad = WireGuardParams {
        private_key: "short-key".into(),
        reserved: "1,2".into(),
        amnezia: crate::protocol_params_ext::AmneziaWgParams {
            jmin: Some(90),
            jmax: Some(10),
            h1: Some(7),
            h2: Some(7),
            ..crate::protocol_params_ext::AmneziaWgParams::default()
        },
        ..WireGuardParams::default()
    };
    let mut issues = Vec::new();
    bad.validate(&mut issues);
    let names = issue_fields(&issues);
    assert!(names.contains(&"wireguard.private-key"), "{names:?}");
    assert!(names.contains(&"wireguard.reserved"), "{names:?}");
    assert!(names.contains(&"wireguard.jmin"), "{names:?}");
    assert!(names.contains(&"wireguard.h1"), "{names:?}");

    // The base64 reserved shape is preserved as authored.
    let base64_form = WireGuardParams {
        reserved: "AQID".into(),
        reserved_is_base64: true,
        ip: "172.16.0.3/32".into(),
        ..WireGuardParams::default()
    };
    let mut issues = Vec::new();
    base64_form.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    assert!(
        base64_form
            .chips()
            .contains(&"wg:reserved(base64)".to_string())
    );

    // A WireGuard node without an interface address is refused.
    let mut issues = Vec::new();
    WireGuardParams::default().validate(&mut issues);
    assert!(
        issue_fields(&issues).contains(&"wireguard.ip"),
        "{issues:?}"
    );
}

#[test]
fn transports_cover_ws_early_data_grpc_xhttp_and_quic_with_notes() {
    let params = TransportParams {
        network: "ws".into(),
        packet_encoding: "xudp".into(),
        ws: WsOptsParams {
            path: "/ws".into(),
            headers: [("Host".to_string(), "cdn.example.com".to_string())]
                .into_iter()
                .collect(),
            max_early_data: 2048,
            early_data_header_name: "Sec-WebSocket-Protocol".into(),
            ..WsOptsParams::default()
        },
        grpc: crate::protocol_params_ext::GrpcOptsParams {
            service_name: "svc".into(),
        },
        ..TransportParams::default()
    };
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let chips = params.chips();
    assert!(chips.contains(&"net:ws".to_string()));
    assert!(chips.contains(&"ws-0rtt:2048".to_string()));
    assert!(chips.contains(&"grpc:svc".to_string()));
    assert!(chips.contains(&"packet:xudp".to_string()));

    let early_without_path = TransportParams {
        network: "ws".into(),
        ws: WsOptsParams {
            max_early_data: 512,
            ..WsOptsParams::default()
        },
        ..TransportParams::default()
    };
    let mut issues = Vec::new();
    early_without_path.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["transport.ws.max-early-data"]);

    let xhttp = TransportParams {
        network: "xhttp".into(),
        xhttp: crate::protocol_params_ext::XhttpOptsParams {
            mode: "stream-up".into(),
            path: "/x".into(),
            host: String::new(),
        },
        ..TransportParams::default()
    };
    let mut issues = Vec::new();
    xhttp.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let mut notes = Vec::new();
    xhttp.notes(&mut notes);
    assert!(
        notes.iter().any(|note| note.contains("v1.19.18")),
        "{notes:?}"
    );

    // QUIC and unknown networks are preserved but honestly flagged: the pinned
    // core silently runs them over TCP.
    let quic = TransportParams {
        network: "quic".into(),
        ..TransportParams::default()
    };
    assert!(!quic.network_known());
    let mut issues = Vec::new();
    quic.validate(&mut issues);
    assert!(issues.is_empty());
    let mut notes = Vec::new();
    quic.notes(&mut notes);
    assert!(notes[0].contains("TCP"), "{notes:?}");

    let bad_xhttp = TransportParams {
        network: "xhttp".into(),
        xhttp: crate::protocol_params_ext::XhttpOptsParams {
            mode: "turbo".into(),
            ..crate::protocol_params_ext::XhttpOptsParams::default()
        },
        ..TransportParams::default()
    };
    let mut issues = Vec::new();
    bad_xhttp.validate(&mut issues);
    assert!(
        issue_fields(&issues).contains(&"transport.xhttp.mode"),
        "{issues:?}"
    );

    let network_but_no_opts = TransportParams {
        network: "xhttp".into(),
        ..TransportParams::default()
    };
    let mut issues = Vec::new();
    network_but_no_opts.validate(&mut issues);
    assert!(
        issue_fields(&issues).contains(&"transport.xhttp"),
        "{issues:?}"
    );

    let mut draft = ProtocolDraft::new("ss");
    draft.name = "s".into();
    draft.server = "1.1.1.1".into();
    draft.port = 8388;
    draft.password = "pw".into();
    draft.params.transport.network = "ws".into();
    assert!(
        draft
            .report()
            .issues
            .iter()
            .any(|issue| issue.field == "transport.unsupported")
    );
}

#[test]
fn sip003_plugins_enforce_their_required_options() {
    let shadow_tls = Sip003Plugin {
        name: "shadow-tls".into(),
        opts: [
            ("host".to_string(), PluginOptValue::Text("bing.com".into())),
            ("password".to_string(), PluginOptValue::Text("pw".into())),
            ("version".to_string(), PluginOptValue::Number(3)),
        ]
        .into_iter()
        .collect(),
    };
    let mut issues = Vec::new();
    shadow_tls.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let chips = shadow_tls.chips();
    assert!(chips.contains(&"plugin:shadow-tls".to_string()));
    assert!(chips.contains(&"plugin-opt:version=3".to_string()));

    let missing = Sip003Plugin {
        name: "shadow-tls".into(),
        opts: Default::default(),
    };
    let mut issues = Vec::new();
    missing.validate(&mut issues);
    let names = issue_fields(&issues);
    assert!(names.contains(&"plugin.host"), "{names:?}");
    assert!(names.contains(&"plugin.password"), "{names:?}");

    let bad_obfs = Sip003Plugin {
        name: "obfs".into(),
        opts: [("mode".to_string(), PluginOptValue::Text("ftp".into()))]
            .into_iter()
            .collect(),
    };
    let mut issues = Vec::new();
    bad_obfs.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["plugin.mode"]);

    let no_name = Sip003Plugin {
        name: String::new(),
        opts: [("mode".to_string(), PluginOptValue::Bool(true))]
            .into_iter()
            .collect(),
    };
    let mut issues = Vec::new();
    no_name.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["plugin.name"]);

    let unknown = Sip003Plugin {
        name: "future-plugin".into(),
        opts: Default::default(),
    };
    let mut issues = Vec::new();
    unknown.validate(&mut issues);
    assert!(issues.is_empty());
    let mut notes = Vec::new();
    unknown.notes(&mut notes);
    assert!(notes[0].contains("future-plugin"), "{notes:?}");

    // Plugins are a Shadowsocks-only schema.
    let mut draft = vless();
    draft.params.plugin.name = "shadow-tls".into();
    assert!(
        draft
            .report()
            .issues
            .iter()
            .any(|issue| issue.field == "plugin.unsupported")
    );
}

#[test]
fn ssh_parameters_require_an_identity() {
    let params = SshParams {
        username: "root".into(),
        private_key: "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
        passphrase: "pass".into(),
        host_key_algorithms: vec!["ssh-ed25519".into()],
    };
    let mut issues = Vec::new();
    params.validate("", &mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let chips = params.chips();
    assert!(chips.contains(&"ssh:root".to_string()));
    assert!(chips.contains(&"ssh:key".to_string()));
    assert!(chips.contains(&"ssh-algs:1".to_string()));

    let no_auth = SshParams {
        username: "root".into(),
        ..SshParams::default()
    };
    let mut issues = Vec::new();
    no_auth.validate("", &mut issues);
    let names = issue_fields(&issues);
    assert!(names.contains(&"ssh.auth"), "{names:?}");

    let passphrase_only = SshParams {
        username: "root".into(),
        passphrase: "pass".into(),
        ..SshParams::default()
    };
    let mut issues = Vec::new();
    passphrase_only.validate("pw", &mut issues);
    assert!(
        issue_fields(&issues).contains(&"ssh.passphrase"),
        "{:?}",
        issue_fields(&issues)
    );

    // A password-only SSH draft is valid and draws the username-required rule.
    let mut draft = ProtocolDraft::new("ssh");
    draft.name = "s".into();
    draft.server = "ssh.example.com".into();
    draft.port = 22;
    draft.password = "pw".into();
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "ssh.username"),
        "{:?}",
        report.issues
    );
    draft.params.ssh.username = "root".into();
    assert!(draft.report().is_valid(), "{:?}", draft.report().issues);
    assert_eq!(draft.required_credentials(), &[] as &[&str]);

    // WireGuard has a required private key slot.
    let mut wg = ProtocolDraft::new("wireguard");
    wg.name = "wg".into();
    wg.server = "wg.example.com".into();
    wg.port = 51820;
    assert_eq!(wg.required_credentials(), &["private-key"]);
    assert!(!wg.has_required_credentials());
}

#[test]
fn anytls_and_trojan_ss_opts_are_typed() {
    let anytls = AnyTlsParams {
        idle_session_timeout: 30000,
        idle_session_check_interval: 30000,
        min_idle_session: 3,
    };
    assert!(anytls.is_present());
    assert_eq!(
        anytls.chips(),
        vec![
            "idle:30000ms".to_string(),
            "idle-check:30000ms".to_string(),
            "min-idle:3".to_string(),
        ]
    );

    let ss = TrojanSsParams {
        enabled: true,
        method: "aes-128-gcm".into(),
        password: "pw".into(),
    };
    let mut issues = Vec::new();
    ss.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        ss.chips(),
        vec![
            "trojan-go:ss".to_string(),
            "ss-opts:aes-128-gcm".to_string()
        ]
    );

    let broken = TrojanSsParams {
        enabled: true,
        method: "rot13".into(),
        password: String::new(),
    };
    let mut issues = Vec::new();
    broken.validate(&mut issues);
    let names = issue_fields(&issues);
    assert!(names.contains(&"trojan.ss-opts.method"), "{names:?}");
    assert!(names.contains(&"trojan.ss-opts.password"), "{names:?}");

    let off = TrojanSsParams {
        enabled: false,
        method: "aes-128-gcm".into(),
        password: "pw".into(),
    };
    let mut issues = Vec::new();
    off.validate(&mut issues);
    assert_eq!(issue_fields(&issues), vec!["trojan.ss-opts"]);
    assert!(off.chips().is_empty());

    // Trojan drafts surface the ss-opts chips through the shared report.
    let mut draft = ProtocolDraft::new("trojan");
    draft.name = "t".into();
    draft.server = "t.example.com".into();
    draft.port = 443;
    draft.password = "pw".into();
    draft.params.trojan_ss.enabled = true;
    draft.params.trojan_ss.method = "aes-128-gcm".into();
    draft.params.trojan_ss.password = "pw".into();
    let report = draft.report();
    assert!(report.is_valid(), "{:?}", report.issues);
    assert!(
        report
            .params
            .trojan_chips
            .contains(&"trojan-go:ss".to_string())
    );
}

#[test]
fn alpn_order_is_the_negotiation_model() {
    let mut draft = vless();
    draft.alpn = vec!["h2".into(), "http/1.1".into()];
    let report = draft.report();
    assert_eq!(
        report.params.alpn_chips,
        vec!["alpn[0]:h2".to_string(), "alpn[1]:http/1.1".to_string()]
    );
    assert!(report.is_valid(), "{:?}", report.issues);

    draft.alpn = vec!["h2".into(), "http/1.1".into(), "h2".into()];
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "alpn" && issue.message.contains("unique")),
        "{:?}",
        report.issues
    );
}

#[test]
fn params_report_collects_every_chip_for_the_surfaces() {
    let mut draft = vless();
    draft.params.ech.enabled = true;
    draft.alpn = vec!["h2".into()];
    draft.params.transport.network = "ws".into();
    draft.params.transport.ws.path = "/ws".into();
    draft.params.transport.ws.max_early_data = 1024;
    let report = ProtocolParamsReport::from_draft(&draft);
    let chips = report.chips();
    assert!(chips.contains(&"ech:dns".to_string()));
    assert!(chips.contains(&"alpn[0]:h2".to_string()));
    assert!(chips.contains(&"net:ws".to_string()));
    assert!(chips.contains(&"ws-0rtt:1024".to_string()));

    // Gating: a block set on the wrong family never leaks chips.
    let mut ss = ProtocolDraft::new("ss");
    ss.name = "s".into();
    ss.server = "1.1.1.1".into();
    ss.port = 8388;
    ss.password = "pw".into();
    ss.params.wireguard.mtu = 1280;
    let report = ProtocolParamsReport::from_draft(&ss);
    assert!(report.wireguard_chips.is_empty());
    assert!(report.chips().is_empty());
    assert!(ProtocolParams::default().notes(ss.family()).is_empty());
}
