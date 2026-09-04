//! Tests for the [`crate::proxy_nodes`] module, mounted via `#[cfg(test)]`
//! from the module root (kept out of the business-code line budget by
//! convention).

#[cfg(test)]
mod tests {
    use crate::proxy_nodes::{model::*, profile_yaml::*, validate::*};
    use serde_yaml_ng::Value;

    const VLESS_YAML: &str = r#"
proxies:
  - name: vless-reality-vision
    type: vless
    server: 203.0.113.10
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    udp: true
    tls: true
    skip-cert-verify: false
    flow: xtls-rprx-vision
    servername: www.microsoft.com
    client-fingerprint: chrome
    network: tcp
    reality-opts:
      public-key: SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc
      short-id: "6ba85179e30d4fc2"
      spider-x: "/spx"
    grpc-opts:
      grpc-service-name: grpc-svc
    ws-opts:
      path: /ws
      headers:
        Host: www.microsoft.com
    xhttp-opts:
      mode: stream-up
      path: /xhttp-path
    packet-encoding: packetaddr
    fake-field: 123
"#;

    const HYSTERIA2_YAML: &str = r#"
proxies:
  - name: hy2-full
    type: hysteria2
    server: 203.0.113.20
    port: 36712
    ports: "20000-30000,8443,443"
    password: hy2-pass
    obfs: salamander
    obfs-password: obfs-pass
    hop-interval: 30
    up: "100 Mbps"
    down: 200
    alpn:
      - h3
    cwnd: 1024
    recv-window-conn: 65536
    fake-field: hello
"#;

    const TUIC_YAML: &str = r#"
proxies:
  - name: tuic-full
    type: tuic
    server: 203.0.113.30
    port: 443
    uuid: 5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22
    password: tuic-pass
    congestion-controller: bbr
    udp-relay-mode: native
    alpn:
      - h3
    reduce-rtt: true
    heartbeat-interval: 10000
    request-timeout: 8000
    fake-field: 123
"#;

    const WIREGUARD_YAML: &str = r#"
proxies:
  - name: wg-full
    type: wireguard
    server: 203.0.113.40
    port: 51820
    udp: true
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    pre-shared-key: 31aIhAPwktDGpH4JDhA8GNvjFXEf/a6+UaQRyOAiyfM=
    reserved: [1, 2, 3]
    mtu: 1420
    ip: 172.16.0.2
    ipv6: fd01:5ca1:ab1e:80fa:ab85:6eea:213f:f4a5
    remote-dns-resolve: true
    dns:
      - 1.1.1.1
      - 8.8.8.8
    amnezia-opts:
      jc: 5
      jmin: 40
      jmax: 70
      s1: 15
      s2: 20
      h1: 123456
      h2: 234567
      h3: 345678
      h4: 456789
    fake-field: 123
"#;

    const WIREGUARD_RESERVED_BASE64_YAML: &str = r#"
proxies:
  - name: wg-base64
    type: wireguard
    server: 203.0.113.41
    port: 51820
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    reserved: AQID
    ip: 172.16.0.3
    fake-field: keep-me
"#;

    const SHADOWSOCKS_2022_YAML: &str = r#"
proxies:
  - name: ss-2022-node
    type: ss
    server: 203.0.113.50
    port: 8388
    cipher: 2022-blake3-aes-128-gcm
    password: eCtXsJZ27+4PbhDkHnB92w==
    udp-over-tcp: true
    uot-version: 2
    plugin: v2ray-plugin
    plugin-opts:
      mode: websocket
      host: ss.example.com
      path: /ws
      tls: true
    fake-field: 7
"#;

    const ANYTLS_YAML: &str = r#"
proxies:
  - name: anytls-node
    type: anytls
    server: 203.0.113.60
    port: 443
    password: anytls-secret-token
    padding-range: 100-1000
    idle-timeout: 60
    client-fingerprint: chrome
    sni: anytls.example.com
    alpn:
      - h2
      - http/1.1
    fake-field: 99
"#;

    const TROJAN_YAML: &str = r#"
proxies:
  - name: trojan-node
    type: trojan
    server: 203.0.113.70
    port: 443
    password: trojan-secret-pass
    sni: trojan.example.com
    alpn:
      - h2
      - http/1.1
    network: ws
    ws-opts:
      path: /trojan-ws
    fake-field: 42
"#;

    const VMESS_YAML: &str = r#"
proxies:
  - name: vmess-node
    type: vmess
    server: 203.0.113.80
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    alter-id: 0
    cipher: auto
    servername: vmess.example.com
    network: ws
    fake-field: 88
"#;

    /// Profile containing all typed protocols plus a custom fallback node.
    const PROFILE_YAML: &str = r#"
mixed-port: 7890
mode: rule
log-level: info

dns:
  enable: true
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16

proxies:
  - name: vless-reality-vision
    type: vless
    server: 203.0.113.10
    port: 443
    uuid: b831381d-6324-4d53-ad4f-8cda48b30811
    flow: xtls-rprx-vision
    reality-opts:
      public-key: SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc
      short-id: "6ba85179e30d4fc2"
    fake-field: 123
  - name: hy2-full
    type: hysteria2
    server: 203.0.113.20
    port: 36712
    password: hy2-pass
    obfs: salamander
    obfs-password: obfs-pass
    up: "100 Mbps"
    down: 200
    fake-field: hello
  - name: tuic-full
    type: tuic
    server: 203.0.113.30
    port: 443
    uuid: 5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22
    password: tuic-pass
    congestion-controller: bbr
    fake-field: 123
  - name: wg-full
    type: wireguard
    server: 203.0.113.40
    port: 51820
    private-key: eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=
    public-key: Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=
    reserved: [1, 2, 3]
    ip: 172.16.0.2
    fake-field: 123
  - name: legacy-ss
    type: ss
    server: 203.0.113.50
    port: 8388
    cipher: 2022-blake3-aes-128-gcm
    password: eCtXsJZ27+4PbhDkHnB92w==
    plugin: v2ray-plugin
    fake-field: 7
  - name: anytls-node
    type: anytls
    server: 203.0.113.60
    port: 443
    password: anytls-secret-token
  - name: custom-future-node
    type: quantum-tunnel-v9
    server: 203.0.113.99
    port: 9000
    secret-key: test

rules:
  - DOMAIN-SUFFIX,example.com,DIRECT
  - MATCH,PROXY
"#;

    fn parse_single(text: &str) -> RawNode {
        let nodes = parse_profile_yaml(text).expect("parse profile yaml");
        assert_eq!(nodes.len(), 1, "expected exactly one node");
        nodes.into_iter().next().expect("node")
    }

    /// typed -> YAML -> typed must be a fixed point.
    fn assert_roundtrip_fixed_point(nodes: &[RawNode]) {
        let yaml = nodes_to_profile_yaml(nodes).expect("serialize nodes");
        let reparsed = parse_profile_yaml(&yaml).expect("re-parse serialized nodes");
        assert_eq!(nodes, &reparsed, "typed -> YAML -> typed must be lossless");
    }

    /// The `proxies:` section must be semantically equivalent (same
    /// `serde_yaml_ng::Value`) before and after the roundtrip.
    fn assert_proxies_semantic_equivalence(input: &str) {
        let nodes = parse_profile_yaml(input).expect("parse profile");
        let output = nodes_to_profile_yaml(&nodes).expect("serialize nodes");
        assert_eq!(
            proxies_value(input),
            proxies_value(&output),
            "proxies section changed across the YAML roundtrip\n--- re-serialized: ---\n{output}"
        );
    }

    fn proxies_value(text: &str) -> Value {
        let doc: Value = serde_yaml_ng::from_str(text).expect("yaml doc");
        doc.get("proxies").cloned().expect("proxies section")
    }

    #[test]
    fn test_vless_full_roundtrip() {
        let node = parse_single(VLESS_YAML);
        let ProxyNode::Vless(vless) = &node else {
            panic!("vless node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "vless");
        assert_eq!(vless.common.name, "vless-reality-vision");
        assert_eq!(vless.common.server, "203.0.113.10");
        assert_eq!(vless.common.port, 443);
        assert_eq!(vless.common.udp, Some(true));
        assert_eq!(vless.common.tls, Some(true));
        assert_eq!(vless.common.skip_cert_verify, Some(false));
        assert_eq!(vless.flow.as_deref(), Some("xtls-rprx-vision"));
        assert_eq!(vless.client_fingerprint.as_deref(), Some("chrome"));
        assert_eq!(vless.network.as_deref(), Some("tcp"));
        assert_eq!(vless.packet_encoding.as_deref(), Some("packetaddr"));

        let reality = vless.reality_opts.as_ref().expect("reality-opts");
        assert_eq!(
            reality.public_key.as_deref(),
            Some("SbVKOEMjK0sIlbwg4akyBg5mL5KZwwB-ed4eEE7YnRc")
        );
        assert_eq!(reality.short_id.as_deref(), Some("6ba85179e30d4fc2"));
        assert_eq!(reality.spider_x.as_deref(), Some("/spx"));

        let grpc = vless.grpc_opts.as_ref().expect("grpc-opts");
        assert_eq!(
            grpc.get("grpc-service-name"),
            Some(&Value::String("grpc-svc".to_string()))
        );
        let ws = vless.ws_opts.as_ref().expect("ws-opts");
        assert_eq!(ws.get("path"), Some(&Value::String("/ws".to_string())));

        let xhttp = vless.xhttp_opts.as_ref().expect("xhttp-opts");
        assert_eq!(xhttp.mode.as_deref(), Some("stream-up"));
        assert_eq!(xhttp.path.as_deref(), Some("/xhttp-path"));

        // Unknown keys must be captured by the flatten extra map.
        assert_eq!(
            vless.uuid.as_deref(),
            Some("b831381d-6324-4d53-ad4f-8cda48b30811")
        );
        assert_eq!(vless.servername.as_deref(), Some("www.microsoft.com"));
        assert!(
            matches!(vless.extra.get("fake-field"), Some(Value::Number(_))),
            "fake-field kept in extra"
        );

        assert_proxies_semantic_equivalence(VLESS_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_hysteria2_full_roundtrip() {
        let node = parse_single(HYSTERIA2_YAML);
        let ProxyNode::Hysteria2(hy2) = &node else {
            panic!("hysteria2 node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "hysteria2");
        assert_eq!(hy2.common.port, 36712);
        assert_eq!(hy2.ports.as_deref(), Some("20000-30000,8443,443"));
        assert_eq!(hy2.password.as_deref(), Some("hy2-pass"));
        assert_eq!(hy2.obfs.as_deref(), Some("salamander"));
        assert_eq!(hy2.obfs_password.as_deref(), Some("obfs-pass"));
        assert_eq!(hy2.hop_interval, Some(30));
        assert_eq!(hy2.up, Some(Bandwidth::Text("100 Mbps".to_string())));
        assert_eq!(hy2.down, Some(Bandwidth::U64(200)));
        assert_eq!(hy2.alpn, Some(vec!["h3".to_string()]));
        assert_eq!(hy2.cwnd, Some(1024));
        assert_eq!(hy2.recv_window_conn, Some(65536));
        assert_eq!(
            hy2.extra.get("fake-field"),
            Some(&Value::String("hello".to_string()))
        );

        assert_proxies_semantic_equivalence(HYSTERIA2_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_tuic_full_roundtrip() {
        let node = parse_single(TUIC_YAML);
        let ProxyNode::Tuic(tuic) = &node else {
            panic!("tuic node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "tuic");
        assert_eq!(
            tuic.uuid.as_deref(),
            Some("5c1eee1f-1f0b-4e11-9a2e-f1d3aa09ab22")
        );
        assert_eq!(tuic.password.as_deref(), Some("tuic-pass"));
        assert_eq!(tuic.congestion_controller.as_deref(), Some("bbr"));
        assert_eq!(tuic.udp_relay_mode.as_deref(), Some("native"));
        assert_eq!(tuic.alpn, Some(vec!["h3".to_string()]));
        assert_eq!(tuic.reduce_rtt, Some(true));
        assert_eq!(tuic.heartbeat_interval, Some(10000));
        assert_eq!(tuic.request_timeout, Some(8000));
        assert_eq!(
            tuic.extra.get("fake-field"),
            Some(&Value::Number(123.into()))
        );

        assert_proxies_semantic_equivalence(TUIC_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_wireguard_full_roundtrip() {
        let node = parse_single(WIREGUARD_YAML);
        let ProxyNode::WireGuard(wg) = &node else {
            panic!("wireguard node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "wireguard");
        assert_eq!(
            wg.private_key.as_deref(),
            Some("eCtXsJZ27+4PbhDkHnB923tkUn2Gj59wZw5wFA75MnU=")
        );
        assert_eq!(
            wg.public_key.as_deref(),
            Some("Cr8hWlKvtDt7nrvf+f0brNQQzabAqrjfBvas9pmowjo=")
        );
        assert_eq!(
            wg.pre_shared_key.as_deref(),
            Some("31aIhAPwktDGpH4JDhA8GNvjFXEf/a6+UaQRyOAiyfM=")
        );
        // List form must be modeled as Reserved::Array and preserved.
        assert_eq!(wg.reserved, Some(Reserved::Array(vec![1, 2, 3])));
        assert_eq!(wg.mtu, Some(1420));
        assert_eq!(wg.ip.as_deref(), Some("172.16.0.2"));
        assert_eq!(
            wg.ipv6.as_deref(),
            Some("fd01:5ca1:ab1e:80fa:ab85:6eea:213f:f4a5")
        );
        assert_eq!(wg.remote_dns_resolve, Some(true));
        assert_eq!(
            wg.dns,
            Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );

        let awg = wg.amnezia_opts.as_ref().expect("amnezia-opts");
        assert_eq!(awg.jc, Some(5));
        assert_eq!(awg.jmin, Some(40));
        assert_eq!(awg.jmax, Some(70));
        assert_eq!(awg.s1, Some(15));
        assert_eq!(awg.s2, Some(20));
        assert_eq!(awg.h1, Some(123456));

        assert_eq!(wg.extra.get("fake-field"), Some(&Value::Number(123.into())));

        assert_proxies_semantic_equivalence(WIREGUARD_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_wireguard_reserved_base64_roundtrip() {
        let node = parse_single(WIREGUARD_RESERVED_BASE64_YAML);
        let ProxyNode::WireGuard(wg) = &node else {
            panic!("wireguard node degraded to Other: {node:?}");
        };
        // Base64 string form must stay a string (保形), not be decoded.
        assert_eq!(wg.reserved, Some(Reserved::Base64("AQID".to_string())));

        assert_proxies_semantic_equivalence(WIREGUARD_RESERVED_BASE64_YAML);
        let yaml = nodes_to_profile_yaml(std::slice::from_ref(&node)).expect("serialize");
        assert!(
            yaml.contains("reserved: AQID"),
            "base64 reserved shape must survive serialization: {yaml}"
        );
        assert!(yaml.contains("fake-field: keep-me"));
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
    }

    #[test]
    fn test_shadowsocks_2022_roundtrip() {
        let node = parse_single(SHADOWSOCKS_2022_YAML);
        let ProxyNode::Shadowsocks(ss) = &node else {
            panic!("shadowsocks node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "ss");
        assert_eq!(ss.cipher.as_deref(), Some("2022-blake3-aes-128-gcm"));
        assert_eq!(ss.password.as_deref(), Some("eCtXsJZ27+4PbhDkHnB92w=="));
        assert_eq!(ss.udp_over_tcp, Some(true));
        assert_eq!(ss.uot_version, Some(2));
        assert_eq!(ss.plugin.as_deref(), Some("v2ray-plugin"));

        assert_proxies_semantic_equivalence(SHADOWSOCKS_2022_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_anytls_roundtrip() {
        let node = parse_single(ANYTLS_YAML);
        let ProxyNode::Anytls(anytls) = &node else {
            panic!("anytls node degraded to Other: {node:?}");
        };
        assert_eq!(node.type_name(), "anytls");
        assert_eq!(anytls.password.as_deref(), Some("anytls-secret-token"));
        assert_eq!(anytls.padding_range.as_deref(), Some("100-1000"));
        assert_eq!(anytls.idle_timeout, Some(60));
        assert_eq!(anytls.client_fingerprint.as_deref(), Some("chrome"));
        assert_eq!(anytls.sni.as_deref(), Some("anytls.example.com"));

        assert_proxies_semantic_equivalence(ANYTLS_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
        assert!(validate(&node).is_empty());
    }

    #[test]
    fn test_trojan_and_vmess_roundtrip() {
        let trojan_node = parse_single(TROJAN_YAML);
        let ProxyNode::Trojan(trojan) = &trojan_node else {
            panic!("trojan node degraded to Other: {trojan_node:?}");
        };
        assert_eq!(trojan_node.type_name(), "trojan");
        assert_eq!(trojan.password.as_deref(), Some("trojan-secret-pass"));
        assert_eq!(trojan.network.as_deref(), Some("ws"));
        assert_proxies_semantic_equivalence(TROJAN_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&trojan_node));
        assert!(validate(&trojan_node).is_empty());

        let vmess_node = parse_single(VMESS_YAML);
        let ProxyNode::Vmess(vmess) = &vmess_node else {
            panic!("vmess node degraded to Other: {vmess_node:?}");
        };
        assert_eq!(vmess_node.type_name(), "vmess");
        assert_eq!(
            vmess.uuid.as_deref(),
            Some("b831381d-6324-4d53-ad4f-8cda48b30811")
        );
        assert_eq!(vmess.alter_id, Some(0));
        assert_eq!(vmess.cipher.as_deref(), Some("auto"));
        assert_proxies_semantic_equivalence(VMESS_YAML);
        assert_roundtrip_fixed_point(std::slice::from_ref(&vmess_node));
        assert!(validate(&vmess_node).is_empty());
    }

    #[test]
    fn test_port_hopping_parser() {
        let hopping = PortHopping::parse("20000-30000, 8443, 443, 50000-50005").unwrap();
        assert_eq!(hopping.specs.len(), 4);
        assert!(hopping.contains(8443));
        assert!(hopping.contains(25000));
        assert!(hopping.contains(50003));
        assert!(!hopping.contains(80));
        assert_eq!(hopping.total_ports(), 10001 + 1 + 1 + 6);
        assert_eq!(
            hopping.to_canonical_string(),
            "20000-30000,8443,443,50000-50005"
        );

        assert!(PortHopping::parse("30000-20000").is_err());
        assert!(PortHopping::parse("0").is_err());
        assert!(PortHopping::parse("").is_err());
    }

    #[test]
    fn test_bandwidth_conversion() {
        assert_eq!(
            Bandwidth::Text("100 Mbps".to_string()).to_bps(),
            Some(100_000_000)
        );
        assert_eq!(
            Bandwidth::Text("1 Gbps".to_string()).to_bps(),
            Some(1_000_000_000)
        );
        assert_eq!(
            Bandwidth::Text("500 kbps".to_string()).to_bps(),
            Some(500_000)
        );
        assert_eq!(Bandwidth::U64(1024).to_bps(), Some(1024));
    }

    #[test]
    fn test_malformed_values_degrade_to_other_losslessly() {
        let text = r#"
proxies:
  - name: wg-bad-reserved
    type: wireguard
    server: 10.0.0.1
    port: 51820
    private-key: k
    public-key: k2
    reserved: [300, 2, 3]
  - name: hy2-bad-port
    type: hysteria2
    server: 10.0.0.2
    port: not-a-port
    password: p
"#;
        let nodes = parse_profile_yaml(text).expect("parse");
        assert_eq!(nodes.len(), 2);

        // reserved [300, ...] does not fit u8 -> the typed variant fails and
        // the node degrades to Other instead of dropping data.
        let ProxyNode::Other(wg) = &nodes[0] else {
            panic!("expected lossless Other fallback, got {:?}", nodes[0]);
        };
        assert_eq!(wg.type_name, "wireguard");
        assert_eq!(
            wg.fields.get("reserved"),
            Some(&serde_yaml_ng::from_str::<Value>("[300, 2, 3]").expect("reserved value"))
        );
        assert!(wg.fields.contains_key("private-key"));

        let ProxyNode::Other(hy2) = &nodes[1] else {
            panic!("expected lossless Other fallback, got {:?}", nodes[1]);
        };
        assert_eq!(hy2.type_name, "hysteria2");
        assert_eq!(
            hy2.fields.get("port"),
            Some(&Value::String("not-a-port".to_string()))
        );

        assert_proxies_semantic_equivalence(text);
        assert_roundtrip_fixed_point(&nodes);
    }

    #[test]
    fn test_future_unknown_type_falls_back_to_other() {
        let text = r#"
proxies:
  - name: future-node
    type: quantum-tunnel-v9
    server: 203.0.113.99
    port: 9000
    secret-handshake: open-sesame
"#;
        let node = parse_single(text);
        let ProxyNode::Other(other) = &node else {
            panic!("future protocol must degrade to Other, got {node:?}");
        };
        assert_eq!(other.type_name, "quantum-tunnel-v9");
        assert_eq!(
            other.fields.get("secret-handshake"),
            Some(&Value::String("open-sesame".to_string()))
        );
        assert_proxies_semantic_equivalence(text);
        assert_roundtrip_fixed_point(std::slice::from_ref(&node));
    }

    #[test]
    fn test_full_profile_roundtrip_all_protocols() {
        let nodes = parse_profile_yaml(PROFILE_YAML).expect("parse profile");
        assert_eq!(nodes.len(), 7);
        let type_names: Vec<&str> = nodes.iter().map(ProxyNode::type_name).collect();
        assert_eq!(
            type_names,
            ["vless", "hysteria2", "tuic", "wireguard", "ss", "anytls", "quantum-tunnel-v9"]
        );
        // Exactly 6 nodes are strongly typed; the 7th falls back to Other.
        assert_eq!(nodes.iter().filter(|n| n.is_typed()).count(), 6);

        // typed -> YAML -> typed stability for the whole list.
        assert_roundtrip_fixed_point(&nodes);
        assert_proxies_semantic_equivalence(PROFILE_YAML);

        // Writing nodes back into the original profile must not touch the other sections.
        let updated = replace_proxies_in_profile(PROFILE_YAML, &nodes).expect("write back");
        let doc: Value = serde_yaml_ng::from_str(&updated).expect("updated doc");
        assert_eq!(doc.get("mixed-port").and_then(Value::as_i64), Some(7890));
        let dns = doc.get("dns").expect("dns section kept");
        assert_eq!(
            dns.get("fake-ip-range"),
            Some(&Value::String("198.18.0.1/16".to_string()))
        );
        let rules = doc
            .get("rules")
            .and_then(Value::as_sequence)
            .expect("rules section kept");
        assert_eq!(rules.len(), 2);
        assert_eq!(doc.get("proxies"), Some(&proxies_value(PROFILE_YAML)));

        // A minimal profile built from nodes parses back to the same nodes.
        let minimal = nodes_to_profile_yaml(&nodes).expect("serialize");
        assert!(parse_profile_yaml(&minimal).expect("reparse minimal") == nodes);
    }

    #[test]
    fn test_unknown_field_survives_repeated_roundtrips() {
        let nodes1 = parse_profile_yaml(TUIC_YAML).expect("parse");
        let yaml1 = nodes_to_profile_yaml(&nodes1).expect("serialize 1");
        assert!(yaml1.contains("fake-field: 123"));

        let nodes2 = parse_profile_yaml(&yaml1).expect("re-parse 1");
        let yaml2 = nodes_to_profile_yaml(&nodes2).expect("serialize 2");
        assert_eq!(yaml1, yaml2, "serialization must be a fixed point");
        assert_eq!(nodes1, nodes2);

        let extra = nodes2[0].extra();
        assert!(
            extra.contains_key("fake-field"),
            "unknown field must still be captured after roundtrips"
        );
    }

    #[test]
    fn test_parse_rejects_malformed_profiles() {
        // Top-level sequence is not a profile document.
        let err = parse_profile_yaml("- a\n- b\n").expect_err("must reject");
        assert!(err.to_string().contains("mapping"), "{err}");

        // `proxies` that is not a list.
        let err = parse_profile_yaml("proxies: oops\n").expect_err("must reject");
        assert!(err.to_string().contains("proxies"), "{err}");

        // An entry without a string `type` key.
        let err = parse_profile_yaml("proxies:\n  - name: n0\n    server: 1.2.3.4\n    port: 1\n")
            .expect_err("must reject");
        assert!(err.to_string().contains("proxies[0]"), "{err}");

        // A scalar entry.
        let err = parse_profile_yaml("proxies:\n  - just-a-string\n").expect_err("must reject");
        assert!(err.to_string().contains("proxies[0]"), "{err}");
    }

    #[test]
    fn test_validate_flags_missing_fields() {
        let text = r#"
proxies:
  - name: vless-broken
    type: vless
    server: 10.4.4.4
    port: 443
  - name: hy2-broken
    type: hysteria2
    server: 10.2.2.2
    port: 443
    obfs: salamander
  - name: tuic-broken
    type: tuic
    server: 10.1.1.1
    port: 443
    congestion-controller: reno
    udp-relay-mode: tcp
  - name: wg-broken
    type: wireguard
    server: 10.3.3.3
    port: 51820
    private-key: k
    public-key: k2
    ip: not-an-ip
  - name: mystery
    type: quantum-tunnel
    port: 7000
"#;
        let nodes = parse_profile_yaml(text).expect("parse");
        assert_eq!(nodes.len(), 5);

        let issues = validate(&nodes[0]);
        assert!(
            issues.iter().any(|m| m.contains("uuid")),
            "vless without uuid must be reported: {issues:?}"
        );

        let issues = validate(&nodes[1]);
        assert!(
            issues.iter().any(|m| m.contains("password"))
                && issues.iter().any(|m| m.contains("obfs-password")),
            "hysteria2 salamander without obfs-password must be reported: {issues:?}"
        );

        let issues = validate(&nodes[2]);
        assert!(
            issues.iter().any(|m| m.contains("uuid"))
                && issues.iter().any(|m| m.contains("password"))
                && issues.iter().any(|m| m.contains("congestion-controller"))
                && issues.iter().any(|m| m.contains("udp-relay-mode")),
            "tuic problems must be reported: {issues:?}"
        );

        let issues = validate(&nodes[3]);
        assert!(
            issues.iter().any(|m| m.contains("not a valid IP")),
            "wireguard address problems must be reported: {issues:?}"
        );

        let issues = validate(&nodes[4]);
        assert!(
            issues.iter().any(|m| m.contains("server")),
            "untyped node without server must be reported: {issues:?}"
        );
    }
}
