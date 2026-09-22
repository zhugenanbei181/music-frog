//! DUAL-05 contract tests: cipher family, REALITY/Vision, smux, draft report.

use super::protocol_fidelity::{
    NodeCodecFormat, ProtocolDraft, ProtocolFamily, ProtocolFidelityReport, RealityParams,
    ShadowsocksCipher, SmuxParams, VlessFlow,
};

#[test]
fn shadowsocks_2022_family_is_complete_and_typed() {
    let mut parsed_2022 = 0;
    for cipher in ShadowsocksCipher::ALL_2022 {
        let parsed = ShadowsocksCipher::parse(cipher.wire_name()).unwrap();
        assert_eq!(parsed, cipher);
        assert!(parsed.is_2022());
        assert!(!parsed.is_stream_cipher());
        assert!(parsed.key_bytes().is_some());
        parsed_2022 += 1;
    }
    assert_eq!(parsed_2022, 4);

    assert_eq!(
        ShadowsocksCipher::parse("2022-blake3-aes-128-gcm")
            .unwrap()
            .key_bytes(),
        Some(16)
    );
    assert_eq!(
        ShadowsocksCipher::parse("2022-blake3-chacha20-poly1305")
            .unwrap()
            .key_bytes(),
        Some(32)
    );
    assert_eq!(ShadowsocksCipher::parse("aes-256-GCM"), None);
    assert_eq!(ShadowsocksCipher::parse("not-a-cipher"), None);
    assert!(
        ShadowsocksCipher::parse("rc4-md5")
            .unwrap()
            .is_stream_cipher()
    );
}

#[test]
fn ss2022_psk_length_is_validated_against_the_cipher() {
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "hk".into();
    draft.server = "1.2.3.4".into();
    draft.port = 8388;
    draft.cipher = "2022-blake3-aes-128-gcm".into();
    // 16 zero bytes -> 24 base64 chars ("AAAAAAAAAAAAAAAAAAAAAA==").
    draft.password = "AAAAAAAAAAAAAAAAAAAAAA==".into();
    assert!(draft.validate().is_empty(), "{:?}", draft.validate());

    draft.password = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into();
    let issues = draft.validate();
    assert!(
        issues
            .iter()
            .any(|issue| issue.field == "password" && issue.message.contains("key size")),
        "{issues:?}"
    );

    // The multi-user form validates every component.
    draft.password = "AAAAAAAAAAAAAAAAAAAAAA==:AAAAAAAAAAAAAAAAAAAAAA==".into();
    assert!(draft.validate().is_empty(), "{:?}", draft.validate());
    draft.password = "AAAAAAAAAAAAAAAAAAAAAA==:nope".into();
    assert!(!draft.validate().is_empty());

    // An unknown cipher is preserved verbatim and reported, never rewritten.
    draft.cipher = "future-cipher-99".into();
    draft.password = "plain-password".into();
    let issues = draft.validate();
    assert!(issues.iter().any(|issue| issue.field == "cipher"));
    assert_eq!(draft.cipher_family(), None);
}

#[test]
fn vless_flow_and_reality_chips_are_shared_vocabulary() {
    assert_eq!(
        VlessFlow::parse("xtls-rprx-vision"),
        Some(VlessFlow::Vision)
    );
    assert_eq!(VlessFlow::Vision.wire_name(), "xtls-rprx-vision");
    assert_eq!(
        VlessFlow::parse("xtls-rprx-vision-udp443"),
        Some(VlessFlow::VisionUdp443)
    );
    assert_eq!(VlessFlow::parse("xtls-rprx-direct"), None);

    let reality = RealityParams {
        public_key: "A".repeat(43),
        short_id: "abcd1234".into(),
        spider_x: "/spider".into(),
        fingerprint: "chrome".into(),
    };
    assert!(reality.is_complete());
    assert!(reality.is_present());
    assert!(reality.fingerprint_known());
    let mut issues = Vec::new();
    reality.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        reality.chips(),
        vec![
            "Reality".to_string(),
            "sid:abcd1234".to_string(),
            "spx:/spider".to_string(),
            "fp:chrome".to_string(),
        ]
    );

    let broken = RealityParams {
        public_key: "short".into(),
        short_id: "xyz".into(),
        spider_x: "spider".into(),
        fingerprint: "netscape".into(),
    };
    let mut issues = Vec::new();
    broken.validate(&mut issues);
    let fields: Vec<&str> = issues.iter().map(|issue| issue.field.as_str()).collect();
    assert!(fields.contains(&"reality.public-key"));
    assert!(fields.contains(&"reality.short-id"));
    assert!(fields.contains(&"reality.spider-x"));
    assert!(fields.contains(&"reality.fingerprint"));
}

#[test]
fn reality_short_id_without_public_key_is_a_shared_issue() {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "reality".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.uuid = "uuid-value".into();
    draft.flow = "xtls-rprx-vision".into();
    draft.reality.short_id = "abcd".into();
    let report = draft.report();
    assert!(report.reality_chips.contains(&"sid:abcd".to_string()));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "reality.public-key")
    );
    assert!(!report.reality_chips.contains(&"Reality".to_string()));

    draft.reality.public_key = "B".repeat(43);
    let report = draft.report();
    assert!(report.is_valid(), "{:?}", report.issues);
    assert!(report.reality_chips.contains(&"Reality".to_string()));
}

#[test]
fn flow_on_a_non_vless_family_is_rejected() {
    let mut draft = ProtocolDraft::new("trojan");
    draft.name = "t".into();
    draft.server = "example.com".into();
    draft.port = 443;
    draft.password = "pw".into();
    draft.flow = "xtls-rprx-vision".into();
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "flow" && issue.message.contains("VLESS"))
    );
}

#[test]
fn smux_parameters_are_typed_and_validated() {
    let defaults = SmuxParams::default();
    assert_eq!(defaults.protocol, "smux");
    assert_eq!(defaults.max_connections, 4);
    assert!(!defaults.has_overrides());
    assert!(defaults.chips().is_empty());

    let mut params = SmuxParams {
        enabled: true,
        protocol: "yamux".into(),
        max_connections: 8,
        min_streams: 2,
        max_streams: 16,
        padding: true,
        ..SmuxParams::default()
    };
    assert!(params.has_overrides());
    let mut issues = Vec::new();
    params.validate(&mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        params.chips(),
        vec![
            "mux:yamux".to_string(),
            "mc:8".to_string(),
            "ms:16".to_string(),
            "min:2".to_string(),
            "padding".to_string(),
        ]
    );
    assert!(params.summary_zh().contains("yamux"));

    params.protocol = "quic-mux".into();
    params.max_connections = 0;
    params.min_streams = 99;
    let mut issues = Vec::new();
    params.validate(&mut issues);
    let fields: Vec<&str> = issues.iter().map(|issue| issue.field.as_str()).collect();
    assert!(fields.contains(&"smux.protocol"));
    assert!(fields.contains(&"smux.max-connections"));
    assert!(fields.contains(&"smux.min-streams"));

    let h2 = SmuxParams {
        enabled: true,
        protocol: "h2mux".into(),
        padding: true,
        ..SmuxParams::default()
    };
    let mut issues = Vec::new();
    h2.validate(&mut issues);
    assert!(issues.iter().any(|issue| issue.field == "smux.padding"));
}

#[test]
fn unsupported_reality_and_smux_are_reported_not_silently_dropped() {
    // REALITY on a family without a reality block.
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "hk".into();
    draft.server = "1.2.3.4".into();
    draft.port = 443;
    draft.password = "pw".into();
    draft.reality.public_key = "C".repeat(43);
    let report = draft.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "reality.unsupported"),
        "{:?}",
        report.issues
    );

    // smux on a family whose schema has no smux block.
    let mut tuic = ProtocolDraft::new("tuic");
    tuic.name = "t".into();
    tuic.server = "example.com".into();
    tuic.port = 443;
    tuic.uuid = "uuid-value".into();
    tuic.password = "pw".into();
    tuic.smux.enabled = true;
    tuic.smux.max_connections = 8;
    let report = tuic.report();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.field == "smux.unsupported"),
        "{:?}",
        report.issues
    );
    assert!(report.smux_chips.is_empty());
}

#[test]
fn draft_report_never_invents_families_or_fields() {
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "hk".into();
    draft.server = "1.2.3.4".into();
    draft.port = 443;
    draft.password = "pw".into();
    draft.smux.enabled = true;
    draft.smux.protocol = "smux".into();

    let report = ProtocolFidelityReport::from_draft(&draft);
    assert_eq!(report.family, ProtocolFamily::Shadowsocks);
    assert!(report.cipher_chip.is_none());
    assert!(!report.cipher_is_2022);
    assert!(report.reality_chips.is_empty());
    assert!(report.smux_chips.contains(&"mux:smux".to_string()));
    assert!(report.is_valid(), "{:?}", report.issues);
    assert!(report.issue_lines().is_empty());

    let unknown = ProtocolDraft::new("shadowsocks-2048");
    assert_eq!(unknown.family(), ProtocolFamily::Unknown);
    assert!(!ProtocolFamily::Unknown.supports_reality());
    assert!(!ProtocolFamily::Unknown.supports_smux());
    assert_eq!(
        ProtocolFamily::from_type_str("awg"),
        ProtocolFamily::WireGuard
    );
    assert_eq!(
        ProtocolFamily::from_type_str("hy2"),
        ProtocolFamily::Hysteria2
    );
}

#[test]
fn tuic_keeps_both_credential_slots_distinct() {
    let mut draft = ProtocolDraft::new("tuic");
    draft.name = "t".into();
    draft.server = "example.com".into();
    draft.port = 443;
    assert_eq!(draft.required_credentials(), &["uuid", "password"]);
    draft.uuid = "uuid-half".into();
    assert!(!draft.has_required_credentials());
    let issues = draft.validate();
    assert!(
        issues.iter().any(|issue| issue.field == "password"),
        "{issues:?}"
    );
    // The single-field projection never mirrors one value into both slots.
    assert_eq!(draft.password_or_uuid(), "uuid-half");
    draft.password = "pw".into();
    assert!(draft.has_required_credentials());
    assert_eq!(draft.validate(), Vec::new());
}

#[test]
fn codec_format_vocabulary_is_stable() {
    assert_eq!(NodeCodecFormat::ALL.len(), 4);
    assert_eq!(
        NodeCodecFormat::ClashYaml.label_zh(),
        NodeCodecFormat::ClashYaml.label_en()
    );
    assert_eq!(
        serde_json::to_string(&NodeCodecFormat::Base64Subscription).unwrap(),
        "\"base64_subscription\""
    );
    assert_eq!(
        serde_json::to_string(&ShadowsocksCipher::Blake3Aes256Gcm).unwrap(),
        "\"2022-blake3-aes-256-gcm\""
    );
    assert_eq!(
        serde_json::to_string(&ShadowsocksCipher::Aes128Gcm).unwrap(),
        "\"aes-128-gcm\""
    );
}
