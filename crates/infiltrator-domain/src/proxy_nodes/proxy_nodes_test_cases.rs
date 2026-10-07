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

fn parse_single(text: &str) -> ProxyNode {
    let nodes = parse_profile_yaml(text).expect("parse profile yaml");
    assert_eq!(nodes.len(), 1, "expected exactly one node");
    nodes.into_iter().next().expect("node")
}

/// typed -> YAML -> typed must be a fixed point.
fn assert_roundtrip_fixed_point(nodes: &[ProxyNode]) {
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

const SSH_YAML: &str = r#"
proxies:
  - name: ssh-node
    type: ssh
    server: 203.0.113.90
    port: 22
    username: root
    private-key: "-----BEGIN OPENSSH PRIVATE KEY-----"
    private-key-passphrase: phrase
    host-key-algorithms:
      - ssh-ed25519
    dialer-proxy: hop
    fake-field: 5
"#;

const TROJAN_SS_OPTS_YAML: &str = r#"
proxies:
  - name: trojan-go-node
    type: trojan
    server: 203.0.113.93
    port: 443
    password: pw
    network: ws
    ws-opts:
      path: /trojan
    ss-opts:
      enabled: true
      method: aes-128-gcm
      password: ss-pw
    fake-field: keep
"#;

#[path = "proxy_nodes_test_cases/anytls.rs"]
mod anytls;
#[path = "proxy_nodes_test_cases/bandwidth.rs"]
mod bandwidth;
#[path = "proxy_nodes_test_cases/full.rs"]
mod full;
#[path = "proxy_nodes_test_cases/future.rs"]
mod future;
#[path = "proxy_nodes_test_cases/hysteria2.rs"]
mod hysteria2;
#[path = "proxy_nodes_test_cases/malformed.rs"]
mod malformed;
#[path = "proxy_nodes_test_cases/parse.rs"]
mod parse;
#[path = "proxy_nodes_test_cases/port.rs"]
mod port;
#[path = "proxy_nodes_test_cases/shadowsocks.rs"]
mod shadowsocks;
#[path = "proxy_nodes_test_cases/ssh.rs"]
mod ssh;
#[path = "proxy_nodes_test_cases/trojan.rs"]
mod trojan;
#[path = "proxy_nodes_test_cases/tuic.rs"]
mod tuic;
#[path = "proxy_nodes_test_cases/unknown.rs"]
mod unknown;
#[path = "proxy_nodes_test_cases/validate.rs"]
mod validate;
#[path = "proxy_nodes_test_cases/vless.rs"]
mod vless;
#[path = "proxy_nodes_test_cases/wireguard.rs"]
mod wireguard;
