use super::*;
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_contract::protocol_params_ext::PluginOptValue;
use infiltrator_domain::profile_converter::ProxyNodeItem;
use infiltrator_domain::protocol_draft_codec::{draft_from_uri, uri_from_draft};
use infiltrator_domain::protocol_node_params::node_from_draft;
use infiltrator_domain::protocol_node_projection::draft_from_node;

fn valid_vless() -> ProtocolDraft {
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "node".to_owned();
    draft.server = "example.com".to_owned();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    draft
}

fn node_round_trip(draft: &ProtocolDraft) -> ProtocolDraft {
    let node = node_from_draft(draft);
    draft_from_node(&node)
}

#[test]
fn uri_draft_round_trip_keeps_the_2022_cipher_family() {
    let uri = "ss://MjAyMi1ibGFrZTMtYWVzLTI1Ni1nY206QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQT0=@ss.example.com:8388#SS-2022";
    let draft = draft_from_uri(uri).unwrap();
    assert_eq!(draft.family(), ProtocolFamily::Shadowsocks);
    assert_eq!(draft.cipher, "2022-blake3-aes-256-gcm");
    assert_eq!(draft.password.len(), 44);
    assert_eq!(
        draft_from_uri(&uri_from_draft(&draft).unwrap())
            .unwrap()
            .cipher,
        draft.cipher
    );
}

#[test]
fn vless_reality_vision_report_and_domain_mapping_keep_typed_values() {
    let mut draft = valid_vless();
    draft.flow = "xtls-rprx-vision".to_owned();
    draft.tls = true;
    draft.reality.public_key = format!("PubKey1234567890{}", "A".repeat(27));
    draft.reality.short_id = "abcd1234".to_owned();
    draft.reality.spider_x = "/spider".to_owned();
    draft.reality.fingerprint = "chrome".to_owned();

    let report = draft.report();
    assert!(report.is_valid(), "{:?}", report.issues);
    assert!(report.flow_chip.is_some());
    assert!(report.reality_chips.contains(&"sid:abcd1234".to_owned()));
    assert_eq!(node_round_trip(&draft), draft);
}

#[test]
fn typed_protocol_parameter_blocks_round_trip_through_domain_node_mapping() {
    let mut wireguard = ProtocolDraft::new("wireguard");
    wireguard.name = "wg".to_owned();
    wireguard.server = "wg.example.com".to_owned();
    wireguard.port = 51820;
    wireguard.params.wireguard.private_key = format!("{}=", "A".repeat(43));
    wireguard.params.wireguard.public_key = format!("{}=", "B".repeat(43));
    wireguard.params.wireguard.pre_shared_key = format!("{}=", "C".repeat(43));
    wireguard.params.wireguard.reserved = "1,2,3".to_owned();
    wireguard.params.wireguard.allowed_ips = vec!["0.0.0.0/0".to_owned()];
    wireguard.params.wireguard.amnezia.jc = Some(4);
    assert_eq!(
        node_round_trip(&wireguard).params.wireguard,
        wireguard.params.wireguard
    );

    let mut tuic = ProtocolDraft::new("tuic");
    tuic.name = "tuic".to_owned();
    tuic.server = "tuic.example.com".to_owned();
    tuic.port = 443;
    tuic.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    tuic.password = "pw".to_owned();
    tuic.params.tuic.congestion_controller = "bbr".to_owned();
    tuic.params.tuic.udp_relay_mode = "native".to_owned();
    tuic.params.tuic.heartbeat_interval = 10;
    assert_eq!(node_round_trip(&tuic).params.tuic, tuic.params.tuic);

    let mut hysteria = ProtocolDraft::new("hysteria2");
    hysteria.name = "hy2".to_owned();
    hysteria.server = "hy2.example.com".to_owned();
    hysteria.port = 443;
    hysteria.password = "pw".to_owned();
    hysteria.params.hysteria2.ports = "20000-30000,8443".to_owned();
    hysteria.params.hysteria2.obfs = "salamander".to_owned();
    hysteria.params.hysteria2.obfs_password = "obfs".to_owned();
    assert_eq!(
        node_round_trip(&hysteria).params.hysteria2,
        hysteria.params.hysteria2
    );
}

#[test]
fn transport_plugin_tls_and_ech_blocks_round_trip_as_typed_values() {
    let mut draft = valid_vless();
    draft.params.transport.network = "ws".to_owned();
    draft.params.transport.ws.path = "/ws".to_owned();
    draft.params.transport.ws.max_early_data = 2048;
    draft.params.transport.grpc.service_name = "svc".to_owned();
    draft.params.ech.enabled = true;
    draft.params.ech.config = "AQIDBA==".to_owned();
    draft.params.tls_trust.ca_path = "/etc/ssl/ca.pem".to_owned();
    draft.params.tls_trust.ca_str =
        "-----BEGIN CERTIFICATE-----\ninline\n-----END CERTIFICATE-----".to_owned();
    draft.params.tls_trust.fingerprint = "00".repeat(32);
    let round_trip = node_round_trip(&draft);
    assert_eq!(round_trip.params.transport, draft.params.transport);
    assert_eq!(round_trip.params.ech, draft.params.ech);
    assert_eq!(round_trip.params.tls_trust, draft.params.tls_trust);

    let mut plugin = ProtocolDraft::new("ss");
    plugin.name = "plugin".to_owned();
    plugin.server = "example.com".to_owned();
    plugin.port = 8388;
    plugin.cipher = "aes-128-gcm".to_owned();
    plugin.password = "pw".to_owned();
    plugin.params.plugin.name = "obfs".to_owned();
    plugin.params.plugin.opts.insert(
        "mode".to_owned(),
        PluginOptValue::Text("websocket".to_owned()),
    );
    assert_eq!(node_round_trip(&plugin).params.plugin, plugin.params.plugin);
}

#[test]
fn domain_mapping_leaves_engine_field_ownership_to_the_selected_codec() {
    let mut node = ProxyNodeItem::new("future", "ss", "1.1.1.1", 8388);
    node.extra
        .insert("future-field".to_owned(), serde_yaml_ng::Value::Bool(true));
    let draft = draft_from_node(&node);
    assert!(draft.preserved_fields.is_empty());
}

#[test]
fn studio_publication_and_field_edit_keep_the_shared_typed_draft() {
    clear_studio();
    let draft = valid_vless();
    let published = ProtocolCodecApplication::publish_draft(draft.clone(), None);
    assert_eq!(published.draft.as_ref(), Some(&draft));
    assert!(ProtocolCodecApplication::update_draft_field("dialer-proxy", "gateway").is_ok());
    assert_eq!(
        studio_snapshot()
            .expect("published studio snapshot")
            .draft
            .as_ref()
            .unwrap()
            .dialer_proxy,
        "gateway"
    );
    assert!(ProtocolCodecApplication::update_draft_field("unknown", "x").is_err());
}
