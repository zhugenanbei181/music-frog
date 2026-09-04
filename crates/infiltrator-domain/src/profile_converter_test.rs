use super::*;
use base64::Engine;
use serde_json::Value;

#[test]
fn test_parse_yaml() {
    let yaml = r#"
proxies:
  - name: "ss-node"
    type: ss
    server: 1.2.3.4
    port: 8388
    cipher: 2022-blake3-aes-128-gcm
    password: "password123"
  - name: "vmess-node"
    type: vmess
    server: 1.1.1.1
    port: 443
    uuid: "b831381d-6324-4d53-ad4f-8cda48b30811"
    tls: true
  - name: "vless-reality"
    type: vless
    server: 2.2.2.2
    port: 443
    uuid: "b831381d-6324-4d53-ad4f-8cda48b30812"
    tls: true
    flow: xtls-rprx-vision
    servername: example.com
    client-fingerprint: chrome
    reality-opts:
      public-key: "pbk-test-key-12345"
      short-id: "sid1234"
      spider-x: "/spx"
  - name: "hy2-node"
    type: hysteria2
    server: 3.3.3.3
    port: 8443
    ports: "10000-20000,8443"
    password: "hy2pass"
    obfs: salamander
    obfs-password: "obfspass"
    up: "100 Mbps"
    down: "200 Mbps"
"#;
    let nodes = ProfileConverter::parse_nodes(yaml, ProfileFormat::ClashYaml).unwrap();
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0].name, "ss-node");
    assert_eq!(nodes[0].node_type, "ss");
    assert_eq!(nodes[0].cipher.as_deref(), Some("2022-blake3-aes-128-gcm"));

    assert_eq!(nodes[1].name, "vmess-node");
    assert_eq!(nodes[1].node_type, "vmess");
    assert!(nodes[1].tls);

    assert_eq!(nodes[2].name, "vless-reality");
    assert_eq!(nodes[2].node_type, "vless");
    assert_eq!(
        nodes[2].get_effective_public_key(),
        Some("pbk-test-key-12345")
    );
    assert_eq!(nodes[2].get_effective_short_id(), Some("sid1234"));
    assert_eq!(nodes[2].flow.as_deref(), Some("xtls-rprx-vision"));

    assert_eq!(nodes[3].name, "hy2-node");
    assert_eq!(nodes[3].node_type, "hysteria2");
    assert_eq!(nodes[3].ports.as_deref(), Some("10000-20000,8443"));
    assert_eq!(nodes[3].obfs.as_deref(), Some("salamander"));
}

#[test]
fn test_roundtrip_json() {
    let mut node = ProxyNodeItem::new("test-trojan", "trojan", "example.com", 443);
    node.password = Some("secret".to_string());
    node.tls = true;
    node.servername = Some("sni.example.com".to_string());
    node.alpn = Some(vec!["h2".to_string(), "http/1.1".to_string()]);
    node.network = Some("ws".to_string());

    let json = ProfileConverter::export_nodes(std::slice::from_ref(&node), ProfileFormat::RawJson)
        .unwrap();
    let parsed = ProfileConverter::parse_nodes(&json, ProfileFormat::RawJson).unwrap();

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, node.name);
    assert_eq!(parsed[0].server, node.server);
    assert_eq!(parsed[0].port, node.port);
    assert_eq!(parsed[0].password, node.password);
    assert_eq!(parsed[0].servername, node.servername);
    assert_eq!(parsed[0].alpn, node.alpn);
    assert_eq!(parsed[0].network, node.network);
}

#[test]
fn test_roundtrip_uri_base64() {
    let mut node = ProxyNodeItem::new("vmess-test", "vmess", "v.example.com", 443);
    node.uuid = Some("b831381d-6324-4d53-ad4f-8cda48b30811".to_string());
    node.tls = true;

    let b64 = ProfileConverter::export_nodes(
        std::slice::from_ref(&node),
        ProfileFormat::Base64Subscription,
    )
    .unwrap();
    let parsed = ProfileConverter::parse_nodes(&b64, ProfileFormat::Base64Subscription).unwrap();

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, node.name);
    assert_eq!(parsed[0].server, node.server);
    assert_eq!(parsed[0].port, node.port);
    assert_eq!(parsed[0].uuid, node.uuid);
    assert!(parsed[0].tls);
}

#[test]
fn test_parse_vless_reality() {
    let uri = "vless://b831381d-6324-4d53-ad4f-8cda48b30811@us.reality.example.com:443?security=reality&encryption=none&pbk=PubKey1234567890&sid=abcd1234&spx=%2Fspider&fp=chrome&type=grpc&serviceName=vless-grpc&sni=reality.example.com&flow=xtls-rprx-vision#US-Reality-Node";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "US-Reality-Node");
    assert_eq!(node.server, "us.reality.example.com");
    assert_eq!(node.port, 443);
    assert_eq!(node.node_type, "vless");
    assert_eq!(
        node.uuid.as_deref(),
        Some("b831381d-6324-4d53-ad4f-8cda48b30811")
    );
    assert_eq!(node.public_key.as_deref(), Some("PubKey1234567890"));
    assert_eq!(node.short_id.as_deref(), Some("abcd1234"));
    assert_eq!(node.spider_x.as_deref(), Some("/spider"));
    assert_eq!(node.client_fingerprint.as_deref(), Some("chrome"));
    assert_eq!(node.flow.as_deref(), Some("xtls-rprx-vision"));
    assert_eq!(node.network.as_deref(), Some("grpc"));
    assert_eq!(node.servername.as_deref(), Some("reality.example.com"));
    assert_eq!(node.get_grpc_service_name(), Some("vless-grpc"));
    assert!(node.tls);

    // Re-export and parse again
    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.name, node.name);
    assert_eq!(re_parsed.server, node.server);
    assert_eq!(re_parsed.port, node.port);
    assert_eq!(re_parsed.public_key, node.public_key);
    assert_eq!(re_parsed.short_id, node.short_id);
    assert_eq!(re_parsed.flow, node.flow);
}

#[test]
fn test_parse_hysteria2() {
    let uri = "hysteria2://sec-pass@hy2.example.com:8443?insecure=1&sni=hy2.example.com&obfs=salamander&obfs-password=obfspass&ports=10000-20000%2C8443&up=100%20Mbps&down=200%20Mbps&alpn=h3#HK-HY2";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "HK-HY2");
    assert_eq!(node.server, "hy2.example.com");
    assert_eq!(node.port, 8443);
    assert_eq!(node.node_type, "hysteria2");
    assert_eq!(node.password.as_deref(), Some("sec-pass"));
    assert_eq!(node.servername.as_deref(), Some("hy2.example.com"));
    assert_eq!(node.skip_cert_verify, Some(true));
    assert_eq!(node.obfs.as_deref(), Some("salamander"));
    assert_eq!(node.obfs_password.as_deref(), Some("obfspass"));
    assert_eq!(node.ports.as_deref(), Some("10000-20000,8443"));
    assert_eq!(node.up.as_deref(), Some("100 Mbps"));
    assert_eq!(node.down.as_deref(), Some("200 Mbps"));
    assert_eq!(node.alpn, Some(vec!["h3".to_string()]));
    assert!(node.tls);

    // Re-export roundtrip
    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.name, node.name);
    assert_eq!(re_parsed.password, node.password);
    assert_eq!(re_parsed.ports, node.ports);
    assert_eq!(re_parsed.obfs, node.obfs);
}

#[test]
fn test_parse_tuic() {
    let uri = "tuic://uuid-user:pass-secret@tuic.example.com:8443?congestion_controller=bbr&udp_relay_mode=native&alpn=h3&sni=tuic.example.com&reduce_rtt=1&allowInsecure=1#TUIC-Node";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "TUIC-Node");
    assert_eq!(node.server, "tuic.example.com");
    assert_eq!(node.port, 8443);
    assert_eq!(node.node_type, "tuic");
    assert_eq!(node.uuid.as_deref(), Some("uuid-user"));
    assert_eq!(node.password.as_deref(), Some("pass-secret"));
    assert_eq!(node.congestion_controller.as_deref(), Some("bbr"));
    assert_eq!(node.udp_relay_mode.as_deref(), Some("native"));
    assert_eq!(node.alpn, Some(vec!["h3".to_string()]));
    assert_eq!(node.servername.as_deref(), Some("tuic.example.com"));
    assert_eq!(node.reduce_rtt, Some(true));
    assert_eq!(node.skip_cert_verify, Some(true));
    assert!(node.tls);

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.uuid, node.uuid);
    assert_eq!(re_parsed.password, node.password);
    assert_eq!(re_parsed.congestion_controller, node.congestion_controller);
}

#[test]
fn test_parse_shadowsocks_sip002_and_sip003() {
    // 2022 cipher with v2ray-plugin
    let uri = "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206cGFzc3dvcmQxMjM=@ss.example.com:8388?plugin=v2ray-plugin%3Bmode%3Dwebsocket%3Bhost%3Dss.example.com%3Bpath%3D%2Fws%3Btls#SS-2022";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "SS-2022");
    assert_eq!(node.server, "ss.example.com");
    assert_eq!(node.port, 8388);
    assert_eq!(node.node_type, "ss");
    assert_eq!(node.cipher.as_deref(), Some("2022-blake3-aes-128-gcm"));
    assert_eq!(node.password.as_deref(), Some("password123"));
    assert_eq!(node.plugin.as_deref(), Some("v2ray-plugin"));

    assert!(node.plugin_opts.is_some());
    let opts = node.plugin_opts.as_ref().unwrap();
    assert_eq!(opts.get("mode").and_then(Value::as_str), Some("websocket"));
    assert_eq!(
        opts.get("host").and_then(Value::as_str),
        Some("ss.example.com")
    );
    assert_eq!(opts.get("path").and_then(Value::as_str), Some("/ws"));
    assert_eq!(opts.get("tls").and_then(Value::as_bool), Some(true));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.cipher, node.cipher);
    assert_eq!(re_parsed.password, node.password);
    assert_eq!(re_parsed.plugin, node.plugin);
}

#[test]
fn test_parse_trojan() {
    let uri = "trojan://trojanpass@trojan.example.com:443?sni=trojan.example.com&alpn=h2%2Chttp%2F1.1&type=ws&path=%2Ftrojan-ws&host=trojan.example.com&allowInsecure=1#Trojan-Node";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "Trojan-Node");
    assert_eq!(node.server, "trojan.example.com");
    assert_eq!(node.port, 443);
    assert_eq!(node.node_type, "trojan");
    assert_eq!(node.password.as_deref(), Some("trojanpass"));
    assert_eq!(node.servername.as_deref(), Some("trojan.example.com"));
    assert_eq!(node.network.as_deref(), Some("ws"));
    assert_eq!(node.get_ws_path(), Some("/trojan-ws"));
    assert_eq!(node.get_ws_host(), Some("trojan.example.com"));
    assert_eq!(node.skip_cert_verify, Some(true));
    assert!(node.tls);

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.name, node.name);
    assert_eq!(re_parsed.password, node.password);
    assert_eq!(re_parsed.servername, node.servername);
}

#[test]
fn test_parse_wireguard() {
    let uri = "wireguard://c2VjcmV0cHJpdmF0ZWtleQ==@wg.example.com:51820?public_key=c2VydmVycHVibGlja2V5&preshared_key=cHJlc2hhcmVka2V5&ip=10.0.0.2&ipv6=fd00::2&mtu=1420&reserved=1,2,3#WG-Node";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "WG-Node");
    assert_eq!(node.server, "wg.example.com");
    assert_eq!(node.port, 51820);
    assert_eq!(node.node_type, "wireguard");
    assert_eq!(
        node.private_key.as_deref(),
        Some("c2VjcmV0cHJpdmF0ZWtleQ==")
    );
    assert_eq!(node.public_key.as_deref(), Some("c2VydmVycHVibGlja2V5"));
    assert_eq!(node.preshared_key.as_deref(), Some("cHJlc2hhcmVka2V5"));
    assert_eq!(node.ip.as_deref(), Some("10.0.0.2"));
    assert_eq!(node.ipv6.as_deref(), Some("fd00::2"));
    assert_eq!(node.mtu, Some(1420));
    assert_eq!(node.reserved, Some(vec![1, 2, 3]));
    assert_eq!(node.udp, Some(true));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.private_key, node.private_key);
    assert_eq!(re_parsed.public_key, node.public_key);
    assert_eq!(re_parsed.reserved, node.reserved);
}

#[test]
fn test_parse_ssh() {
    let uri = "ssh://root:sshpassword@ssh.example.com:22?private_key=priv-key-data&passphrase=pp-secret&host_key_algorithms=ssh-ed25519,rsa-sha2-512#SSH-Node";

    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "SSH-Node");
    assert_eq!(node.server, "ssh.example.com");
    assert_eq!(node.port, 22);
    assert_eq!(node.node_type, "ssh");
    assert_eq!(node.username.as_deref(), Some("root"));
    assert_eq!(node.password.as_deref(), Some("sshpassword"));
    assert_eq!(node.private_key.as_deref(), Some("priv-key-data"));
    assert_eq!(node.passphrase.as_deref(), Some("pp-secret"));
    assert_eq!(
        node.host_key_algorithms,
        Some(vec!["ssh-ed25519".to_string(), "rsa-sha2-512".to_string()])
    );

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let re_parsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(re_parsed.username, node.username);
    assert_eq!(re_parsed.password, node.password);
}

#[test]
fn test_aggregate_profiles() {
    let p1 = r#"
proxies:
  - name: "node-hk"
    type: hysteria2
    server: 1.1.1.1
    port: 8443
    password: pass
"#;
    let p2 = r#"
proxies:
  - name: "node-hk"
    type: tuic
    server: 2.2.2.2
    port: 8443
    uuid: test-uuid
"#;

    let merged = ProfileConverter::aggregate_profiles(&[p1, p2]).unwrap();
    assert!(merged.contains("node-hk"));
    assert!(merged.contains("node-hk (2)"));

    let parsed = ProfileConverter::parse_nodes(&merged, ProfileFormat::ClashYaml).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].name, "node-hk");
    assert_eq!(parsed[0].node_type, "hysteria2");
    assert_eq!(parsed[1].name, "node-hk (2)");
    assert_eq!(parsed[1].node_type, "tuic");
}

#[test]
fn test_detect_and_convert() {
    // 1. Raw JSON to Clash YAML
    let json_input = r#"[
        {
            "name": "json-node",
            "type": "ss",
            "server": "1.2.3.4",
            "port": 8388,
            "cipher": "aes-256-gcm",
            "password": "pass"
        }
    ]"#;
    let yaml_out = ProfileConverter::detect_and_convert(json_input).unwrap();
    assert!(yaml_out.contains("json-node"));
    assert!(yaml_out.contains("proxies:"));

    // 2. URI string to Clash YAML
    let uri_input = "hysteria2://secret@hy2.com:8443#Hy2-Test";
    let yaml_out2 = ProfileConverter::detect_and_convert(uri_input).unwrap();
    assert!(yaml_out2.contains("Hy2-Test"));
    assert!(yaml_out2.contains("hysteria2"));

    // 3. Base64 subscription to Clash YAML
    let b64_input = base64::engine::general_purpose::STANDARD.encode(
        "trojan://pass@trojan.com:443#Trojan-Sub
",
    );
    let yaml_out3 = ProfileConverter::detect_and_convert(&b64_input).unwrap();
    assert!(yaml_out3.contains("Trojan-Sub"));
    assert!(yaml_out3.contains("trojan"));
}

#[test]
fn test_convert_format() {
    let yaml = r#"
proxies:
  - name: "ss1"
    type: ss
    server: 1.1.1.1
    port: 1080
    cipher: aes-256-gcm
    password: pass
"#;
    let json =
        ProfileConverter::convert(yaml, ProfileFormat::ClashYaml, ProfileFormat::RawJson).unwrap();
    assert!(json.contains("ss1"));
    assert!(json.contains("1.1.1.1"));

    let nodes = ProfileConverter::parse_nodes(&json, ProfileFormat::RawJson).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "ss1");
}

#[test]
fn test_unknown_fields_flatten_lossless() {
    let yaml = r#"
proxies:
  - name: "custom-node"
    type: vless
    server: 1.2.3.4
    port: 443
    uuid: "b831381d-6324-4d53-ad4f-8cda48b30811"
    custom-future-meta-flag: true
    custom-numeric-parameter: 42
"#;
    let nodes = ProfileConverter::parse_nodes(yaml, ProfileFormat::ClashYaml).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0].extra.get("custom-future-meta-flag"),
        Some(&serde_yaml_ng::Value::Bool(true))
    );
    assert_eq!(
        nodes[0].extra.get("custom-numeric-parameter"),
        Some(&serde_yaml_ng::Value::Number(42.into()))
    );

    let exported = ProfileConverter::export_nodes(&nodes, ProfileFormat::ClashYaml).unwrap();
    assert!(exported.contains("custom-future-meta-flag: true"));
    assert!(exported.contains("custom-numeric-parameter: 42"));
}

#[test]
fn test_multi_subscription_aggregator_with_proxy_groups() {
    let sub1 = SourceSubscription::with_prefix(
        "Airport-A",
        r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇯🇵 Tokyo 01"
    type: ss
    server: 2.2.2.2
    port: 443
    password: pass
"#,
        "AirportA",
    );

    let sub2 = SourceSubscription::with_prefix(
        "Airport-B",
        r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
    server: 1.1.1.1
    port: 443
    password: pass
  - name: "🇺🇸 US West"
    type: trojan
    server: 3.3.3.3
    port: 443
    password: pass
"#,
        "AirportB",
    );

    let opts = AggregationOptions {
        content_dedup: ContentDedupStrategy::KeepFirst,
        normalize_country_code: true,
        remove_emojis: true,
        sort_by: NodeSortOrder::CountryCode,
        generate_proxy_groups: true,
        ..Default::default()
    };

    let result = MultiSubscriptionAggregator::aggregate(&[sub1, sub2], &opts).unwrap();
    assert!(result.contains("proxies:"));
    assert!(result.contains("proxy-groups:"));
    assert!(result.contains("🚀 节点选择"));
    assert!(result.contains("♻️ 自动选择"));
    assert!(result.contains("HK 节点"));
    assert!(result.contains("JP 节点"));
    assert!(result.contains("US 节点"));
    assert!(result.contains("[AirportA]"));
    // Sub2's duplicate 1.1.1.1 should have been deduplicated by content fingerprint
    assert!(!result.contains("[AirportB] [HK] 香港 01"));
}

#[test]
fn test_anytls_uri_roundtrip() {
    let uri = "anytls://token123@anytls.example.com:443?sni=anytls.example.com&alpn=h2%2Chttp%2F1.1&fp=chrome&padding=100-1000&idle_timeout=60#AnyTLS-Test";
    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "AnyTLS-Test");
    assert_eq!(node.server, "anytls.example.com");
    assert_eq!(node.port, 443);
    assert_eq!(node.node_type, "anytls");
    assert_eq!(node.password.as_deref(), Some("token123"));
    assert_eq!(node.sni.as_deref(), Some("anytls.example.com"));
    assert_eq!(node.client_fingerprint.as_deref(), Some("chrome"));
    assert_eq!(node.padding_range.as_deref(), Some("100-1000"));
    assert_eq!(node.idle_timeout, Some(60));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let reparsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(reparsed.name, node.name);
    assert_eq!(reparsed.password, node.password);
    assert_eq!(reparsed.padding_range, node.padding_range);
    assert_eq!(reparsed.idle_timeout, node.idle_timeout);
}

#[test]
fn test_amnezia_wireguard_uri_roundtrip() {
    let uri = "awg://c2VjcmV0cHJpdmF0ZWtleQ==@awg.example.com:51820?public_key=c2VydmVycHVibGlja2V5&preshared_key=cHJlc2hhcmVka2V5&ip=10.0.0.5&jc=5&jmin=40&jmax=70&s1=15&s2=20&h1=123456&h2=234567&h3=345678&h4=456789#AWG-Node";
    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "AWG-Node");
    assert_eq!(node.node_type, "wireguard");
    assert_eq!(node.ip.as_deref(), Some("10.0.0.5"));
    assert!(node.amnezia_opts.is_some());

    let awg = node.amnezia_opts.as_ref().unwrap();
    assert_eq!(awg.get("jc").and_then(Value::as_u64), Some(5));
    assert_eq!(awg.get("jmin").and_then(Value::as_u64), Some(40));
    assert_eq!(awg.get("jmax").and_then(Value::as_u64), Some(70));
    assert_eq!(awg.get("h1").and_then(Value::as_u64), Some(123456));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    assert!(exported.starts_with("awg://"));
    let reparsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(reparsed.name, node.name);
    assert_eq!(reparsed.amnezia_opts, node.amnezia_opts);
}

#[test]
fn test_vless_xhttp_transport_roundtrip() {
    let uri = "vless://uuid-vless-123@vless.example.com:443?type=xhttp&mode=stream-up&path=%2Fsplithttp-path&host=vless.example.com&packetEncoding=packetaddr&security=tls&sni=vless.example.com#VLESS-XHTTP";
    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "VLESS-XHTTP");
    assert_eq!(node.node_type, "vless");
    assert_eq!(node.network.as_deref(), Some("xhttp"));
    assert_eq!(node.get_xhttp_mode(), Some("stream-up"));
    assert_eq!(node.get_xhttp_path(), Some("/splithttp-path"));
    assert_eq!(node.packet_encoding.as_deref(), Some("packetaddr"));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    let reparsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(reparsed.network, node.network);
    assert_eq!(reparsed.get_xhttp_mode(), node.get_xhttp_mode());
    assert_eq!(reparsed.get_xhttp_path(), node.get_xhttp_path());
    assert_eq!(reparsed.packet_encoding, node.packet_encoding);
}

#[test]
fn test_shadowsocks_2022_uot_roundtrip() {
    let uri = "ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206cGFzc3dvcmQxMjM=@ss2022.example.com:8388?uot=2#SS2022-UoT";
    let node = ProfileConverter::parse_uri(uri).unwrap();
    assert_eq!(node.name, "SS2022-UoT");
    assert_eq!(node.cipher.as_deref(), Some("2022-blake3-aes-128-gcm"));
    assert_eq!(node.uot_version, Some(2));
    assert_eq!(node.udp_over_tcp, Some(true));

    let exported = ProfileConverter::export_uri(&node).unwrap();
    assert!(exported.contains("uot=2"));
    let reparsed = ProfileConverter::parse_uri(&exported).unwrap();
    assert_eq!(reparsed.uot_version, Some(2));
}
