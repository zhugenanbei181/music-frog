//! URI parsing implementations for proxy protocols.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use url::Url;

use crate::profile_converter::{ProxyNodeItem, uri_parse_aux};

/// Parses any supported proxy URI string into a strongly-typed [`ProxyNodeItem`].
pub fn parse_uri(raw_uri: &str) -> Result<ProxyNodeItem> {
    let uri = raw_uri.trim();
    if uri.is_empty() {
        return Err(anyhow!("Empty URI string"));
    }

    if let Some(rest) = uri.strip_prefix("vmess://") {
        return uri_parse_aux::parse_vmess_uri(rest);
    }

    let parsed = Url::parse(uri).map_err(|e| anyhow!("Invalid URL format: {e}"))?;
    let scheme = parsed.scheme();

    match scheme {
        "ss" => uri_parse_aux::parse_shadowsocks(&parsed),
        "trojan" => uri_parse_aux::parse_trojan(&parsed),
        "vless" => parse_vless(&parsed),
        "hysteria2" | "hy2" => parse_hysteria2(&parsed),
        "tuic" => parse_tuic(&parsed),
        "wireguard" | "wg" | "awg" | "amnezia-wg" => parse_wireguard(&parsed),
        "anytls" => parse_anytls(&parsed),
        "ssh" => uri_parse_aux::parse_ssh(&parsed),
        _ => Err(anyhow!("Unsupported proxy scheme: {scheme}")),
    }
}

fn parse_vless(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "VLESS Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(443);
    let uuid = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };

    let mut flow = None;
    let mut client_fingerprint = None;
    let mut sni = None;
    let mut alpn = None;
    let mut network = None;
    let mut public_key = None;
    let mut short_id = None;
    let mut spider_x = None;
    let mut security = None;
    let mut service_name = None;
    let mut path = None;
    let mut host = None;
    let mut xhttp_mode = None;
    let mut packet_encoding = None;
    let mut skip_cert_verify = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "flow" => flow = Some(v.to_string()),
            "fp" => client_fingerprint = Some(v.to_string()),
            "sni" | "peer" => sni = Some(v.to_string()),
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).collect()),
            "type" => network = Some(v.to_string()),
            "pbk" => public_key = Some(v.to_string()),
            "sid" => short_id = Some(v.to_string()),
            "spx" => spider_x = Some(v.to_string()),
            "security" => security = Some(v.to_string()),
            "serviceName" | "service_name" | "service-name" => service_name = Some(v.to_string()),
            "path" => path = Some(v.to_string()),
            "host" => host = Some(v.to_string()),
            "mode" => xhttp_mode = Some(v.to_string()),
            "packetEncoding" | "packet-encoding" | "packet_encoding" => {
                packet_encoding = Some(v.to_string())
            }
            "insecure" | "allowInsecure" => {
                skip_cert_verify = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            _ => {}
        }
    }

    let tls = security.as_deref() == Some("tls")
        || security.as_deref() == Some("reality")
        || public_key.is_some();

    let reality_opts = if public_key.is_some() || short_id.is_some() || spider_x.is_some() {
        let mut r_map = serde_json::Map::new();
        if let Some(ref pk) = public_key {
            r_map.insert("public-key".to_string(), Value::String(pk.clone()));
        }
        if let Some(ref sid) = short_id {
            r_map.insert("short-id".to_string(), Value::String(sid.clone()));
        }
        if let Some(ref spx) = spider_x {
            r_map.insert("spider-x".to_string(), Value::String(spx.clone()));
        }
        Some(Value::Object(r_map))
    } else {
        None
    };

    let grpc_opts = service_name.map(|sn| json!({ "grpc-service-name": sn }));
    let ws_opts = if path.is_some() || host.is_some() {
        let mut w = serde_json::Map::new();
        if let Some(ref p) = path {
            w.insert("path".to_string(), Value::String(p.clone()));
        }
        if let Some(ref h) = host {
            let mut headers = serde_json::Map::new();
            headers.insert("Host".to_string(), Value::String(h.clone()));
            w.insert("headers".to_string(), Value::Object(headers));
        }
        Some(Value::Object(w))
    } else {
        None
    };

    let xhttp_opts = if network.as_deref() == Some("xhttp")
        || network.as_deref() == Some("splithttp")
        || xhttp_mode.is_some()
    {
        let mut x = serde_json::Map::new();
        if let Some(ref m) = xhttp_mode {
            x.insert("mode".to_string(), Value::String(m.clone()));
        }
        if let Some(ref p) = path {
            x.insert("path".to_string(), Value::String(p.clone()));
        }
        if let Some(ref h) = host {
            x.insert("host".to_string(), Value::String(h.clone()));
        }
        Some(Value::Object(x))
    } else {
        None
    };

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "vless".to_string(),
        password: None,
        uuid,
        cipher: None,
        tls,
        flow,
        client_fingerprint,
        servername: sni.clone(),
        sni,
        alpn,
        skip_cert_verify,
        packet_encoding,
        network,
        public_key,
        short_id,
        spider_x,
        reality_opts,
        ports: None,
        hop_interval: None,
        obfs: None,
        obfs_password: None,
        auth: None,
        up: None,
        down: None,
        cwnd: None,
        recv_window_conn: None,
        recv_window: None,
        congestion_controller: None,
        udp_relay_mode: None,
        reduce_rtt: None,
        heartbeat_interval: None,
        request_timeout: None,
        fast_open: None,
        disable_sni: None,
        version: None,
        padding_range: None,
        idle_timeout: None,
        private_key: None,
        preshared_key: None,
        reserved: None,
        ip: None,
        ipv6: None,
        mtu: None,
        remote_dns_resolve: None,
        workers: None,
        persistent_keepalive: None,
        allowed_ips: None,
        amnezia_opts: None,
        peers: None,
        plugin: None,
        plugin_opts: None,
        udp_over_tcp: None,
        uot_version: None,
        username: None,
        passphrase: None,
        host_key_algorithms: None,
        dialer_proxy: None,
        smux: None,
        tfo: None,
        mptcp: None,
        udp: Some(true),
        ws_opts,
        grpc_opts,
        h2_opts: None,
        http_opts: None,
        xhttp_opts,
        extra: std::collections::BTreeMap::new(),
    })
}

fn parse_hysteria2(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "Hysteria2 Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(443);
    let auth = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };

    let mut sni = None;
    let mut alpn = None;
    let mut obfs = None;
    let mut obfs_password = None;
    let mut ports = None;
    let mut skip_cert_verify = None;
    let mut up = None;
    let mut down = None;
    let mut cwnd = None;
    let mut recv_window_conn = None;
    let mut recv_window = None;
    let mut hop_interval = None;
    let mut fast_open = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" => sni = Some(v.to_string()),
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).collect()),
            "obfs" => obfs = Some(v.to_string()),
            "obfs-password" | "obfs_password" => obfs_password = Some(v.to_string()),
            "mport" | "ports" => ports = Some(v.to_string()),
            "insecure" | "allowInsecure" => {
                skip_cert_verify = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            "up" | "upmbps" => up = Some(v.to_string()),
            "down" | "downmbps" => down = Some(v.to_string()),
            "cwnd" => cwnd = v.parse::<u64>().ok(),
            "recv_window_conn" | "recv-window-conn" => recv_window_conn = v.parse::<u64>().ok(),
            "recv_window" | "recv-window" => recv_window = v.parse::<u64>().ok(),
            "hop_interval" | "hop-interval" => hop_interval = v.parse::<u64>().ok(),
            "fast_open" | "fast-open" => {
                fast_open = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            _ => {}
        }
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "hysteria2".to_string(),
        password: auth.clone(),
        uuid: None,
        cipher: None,
        tls: true,
        flow: None,
        client_fingerprint: None,
        servername: sni.clone(),
        sni,
        alpn,
        skip_cert_verify,
        packet_encoding: None,
        network: None,
        public_key: None,
        short_id: None,
        spider_x: None,
        reality_opts: None,
        ports,
        hop_interval,
        obfs,
        obfs_password,
        auth,
        up,
        down,
        cwnd,
        recv_window_conn,
        recv_window,
        congestion_controller: None,
        udp_relay_mode: None,
        reduce_rtt: None,
        heartbeat_interval: None,
        request_timeout: None,
        fast_open,
        disable_sni: None,
        version: None,
        padding_range: None,
        idle_timeout: None,
        private_key: None,
        preshared_key: None,
        reserved: None,
        ip: None,
        ipv6: None,
        mtu: None,
        remote_dns_resolve: None,
        workers: None,
        persistent_keepalive: None,
        allowed_ips: None,
        amnezia_opts: None,
        peers: None,
        plugin: None,
        plugin_opts: None,
        udp_over_tcp: None,
        uot_version: None,
        username: None,
        passphrase: None,
        host_key_algorithms: None,
        dialer_proxy: None,
        smux: None,
        tfo: None,
        mptcp: None,
        udp: Some(true),
        ws_opts: None,
        grpc_opts: None,
        h2_opts: None,
        http_opts: None,
        xhttp_opts: None,
        extra: std::collections::BTreeMap::new(),
    })
}

fn parse_tuic(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "TUIC Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(443);
    let uuid = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };
    let password = parsed
        .password()
        .map(|p| urlencoding::decode(p).unwrap_or_default().to_string());

    let mut congestion_controller = None;
    let mut udp_relay_mode = None;
    let mut alpn = None;
    let mut sni = None;
    let mut skip_cert_verify = None;
    let mut reduce_rtt = None;
    let mut heartbeat_interval = None;
    let mut request_timeout = None;
    let mut fast_open = None;
    let mut disable_sni = None;
    let mut ip = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "congestion_controller" | "congestion_control" | "cc" => {
                congestion_controller = Some(v.to_string())
            }
            "udp_relay_mode" => udp_relay_mode = Some(v.to_string()),
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).collect()),
            "sni" => sni = Some(v.to_string()),
            "insecure" | "allowInsecure" => {
                skip_cert_verify = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            "reduce_rtt" => reduce_rtt = Some(v == "1" || v.eq_ignore_ascii_case("true")),
            "heartbeat_interval" | "heartbeat-interval" => {
                heartbeat_interval = v.parse::<u64>().ok()
            }
            "request_timeout" | "request-timeout" => request_timeout = v.parse::<u64>().ok(),
            "fast_open" | "fast-open" => {
                fast_open = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            "disable_sni" | "disable-sni" => {
                disable_sni = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            "ip" => ip = Some(v.to_string()),
            _ => {}
        }
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "tuic".to_string(),
        password,
        uuid,
        cipher: None,
        tls: true,
        flow: None,
        client_fingerprint: None,
        servername: sni.clone(),
        sni,
        alpn,
        skip_cert_verify,
        packet_encoding: None,
        network: None,
        public_key: None,
        short_id: None,
        spider_x: None,
        reality_opts: None,
        ports: None,
        obfs: None,
        obfs_password: None,
        auth: None,
        up: None,
        down: None,
        cwnd: None,
        recv_window_conn: None,
        recv_window: None,
        congestion_controller,
        udp_relay_mode,
        reduce_rtt: reduce_rtt.or(Some(true)),
        heartbeat_interval,
        request_timeout,
        fast_open,
        disable_sni,
        version: Some(5),
        padding_range: None,
        idle_timeout: None,
        private_key: None,
        preshared_key: None,
        reserved: None,
        ip,
        ipv6: None,
        mtu: None,
        remote_dns_resolve: None,
        workers: None,
        persistent_keepalive: None,
        allowed_ips: None,
        amnezia_opts: None,
        peers: None,
        plugin: None,
        plugin_opts: None,
        udp_over_tcp: None,
        uot_version: None,
        username: None,
        passphrase: None,
        host_key_algorithms: None,
        dialer_proxy: None,
        smux: None,
        tfo: None,
        mptcp: None,
        udp: Some(true),
        ws_opts: None,
        grpc_opts: None,
        h2_opts: None,
        http_opts: None,
        xhttp_opts: None,
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}

fn parse_wireguard(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "WireGuard Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(51820);
    let private_key = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };

    let mut public_key = None;
    let mut preshared_key = None;
    let mut ip = None;
    let mut ipv6 = None;
    let mut mtu = None;
    let mut reserved = None;
    let mut workers = None;
    let mut persistent_keepalive = None;

    let mut awg_jc = None;
    let mut awg_jmin = None;
    let mut awg_jmax = None;
    let mut awg_s1 = None;
    let mut awg_s2 = None;
    let mut awg_h1 = None;
    let mut awg_h2 = None;
    let mut awg_h3 = None;
    let mut awg_h4 = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "public_key" | "publickey" | "pubkey" => public_key = Some(v.to_string()),
            "preshared_key" | "presharedkey" | "psk" => preshared_key = Some(v.to_string()),
            "ip" | "address" => ip = Some(v.to_string()),
            "ipv6" => ipv6 = Some(v.to_string()),
            "mtu" => mtu = v.parse::<u16>().ok(),
            "workers" => workers = v.parse::<u32>().ok(),
            "persistent_keepalive" | "keepalive" => persistent_keepalive = v.parse::<u32>().ok(),
            "reserved" => {
                let bytes: Vec<u8> = v
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u8>().ok())
                    .collect();
                if !bytes.is_empty() {
                    reserved = Some(bytes);
                }
            }
            "jc" => awg_jc = v.parse::<u8>().ok(),
            "jmin" => awg_jmin = v.parse::<u16>().ok(),
            "jmax" => awg_jmax = v.parse::<u16>().ok(),
            "s1" => awg_s1 = v.parse::<u16>().ok(),
            "s2" => awg_s2 = v.parse::<u16>().ok(),
            "h1" => awg_h1 = v.parse::<u32>().ok(),
            "h2" => awg_h2 = v.parse::<u32>().ok(),
            "h3" => awg_h3 = v.parse::<u32>().ok(),
            "h4" => awg_h4 = v.parse::<u32>().ok(),
            _ => {}
        }
    }

    let amnezia_opts = if awg_jc.is_some()
        || awg_jmin.is_some()
        || awg_jmax.is_some()
        || awg_s1.is_some()
        || awg_s2.is_some()
        || awg_h1.is_some()
        || awg_h2.is_some()
        || awg_h3.is_some()
        || awg_h4.is_some()
    {
        let mut map = serde_json::Map::new();
        if let Some(v) = awg_jc {
            map.insert("jc".to_string(), json!(v));
        }
        if let Some(v) = awg_jmin {
            map.insert("jmin".to_string(), json!(v));
        }
        if let Some(v) = awg_jmax {
            map.insert("jmax".to_string(), json!(v));
        }
        if let Some(v) = awg_s1 {
            map.insert("s1".to_string(), json!(v));
        }
        if let Some(v) = awg_s2 {
            map.insert("s2".to_string(), json!(v));
        }
        if let Some(v) = awg_h1 {
            map.insert("h1".to_string(), json!(v));
        }
        if let Some(v) = awg_h2 {
            map.insert("h2".to_string(), json!(v));
        }
        if let Some(v) = awg_h3 {
            map.insert("h3".to_string(), json!(v));
        }
        if let Some(v) = awg_h4 {
            map.insert("h4".to_string(), json!(v));
        }
        Some(Value::Object(map))
    } else {
        None
    };

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "wireguard".to_string(),
        password: None,
        uuid: None,
        cipher: None,
        tls: false,
        flow: None,
        client_fingerprint: None,
        servername: None,
        sni: None,
        alpn: None,
        skip_cert_verify: None,
        packet_encoding: None,
        network: None,
        public_key,
        short_id: None,
        spider_x: None,
        reality_opts: None,
        ports: None,
        obfs: None,
        obfs_password: None,
        auth: None,
        up: None,
        down: None,
        cwnd: None,
        recv_window_conn: None,
        recv_window: None,
        congestion_controller: None,
        udp_relay_mode: None,
        reduce_rtt: None,
        heartbeat_interval: None,
        request_timeout: None,
        fast_open: None,
        disable_sni: None,
        version: None,
        padding_range: None,
        idle_timeout: None,
        private_key,
        preshared_key,
        reserved,
        ip,
        ipv6,
        mtu,
        remote_dns_resolve: Some(true),
        workers,
        persistent_keepalive,
        allowed_ips: None,
        amnezia_opts,
        peers: None,
        plugin: None,
        plugin_opts: None,
        udp_over_tcp: None,
        uot_version: None,
        username: None,
        passphrase: None,
        host_key_algorithms: None,
        dialer_proxy: None,
        smux: None,
        tfo: None,
        mptcp: None,
        udp: Some(true),
        ws_opts: None,
        grpc_opts: None,
        h2_opts: None,
        http_opts: None,
        xhttp_opts: None,
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}

fn parse_anytls(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "AnyTLS Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(443);
    let auth = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };

    let mut sni = None;
    let mut alpn = None;
    let mut client_fingerprint = None;
    let mut padding_range = None;
    let mut idle_timeout = None;
    let mut skip_cert_verify = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" => sni = Some(v.to_string()),
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).collect()),
            "fp" => client_fingerprint = Some(v.to_string()),
            "padding" | "padding_range" | "padding-range" => padding_range = Some(v.to_string()),
            "idle_timeout" | "idle-timeout" => idle_timeout = v.parse::<u64>().ok(),
            "insecure" | "allowInsecure" => {
                skip_cert_verify = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            _ => {}
        }
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "anytls".to_string(),
        password: auth.clone(),
        uuid: None,
        cipher: None,
        tls: true,
        flow: None,
        client_fingerprint,
        servername: sni.clone(),
        sni,
        alpn,
        skip_cert_verify,
        packet_encoding: None,
        network: None,
        public_key: None,
        short_id: None,
        spider_x: None,
        reality_opts: None,
        ports: None,
        obfs: None,
        obfs_password: None,
        auth,
        up: None,
        down: None,
        cwnd: None,
        recv_window_conn: None,
        recv_window: None,
        congestion_controller: None,
        udp_relay_mode: None,
        reduce_rtt: None,
        heartbeat_interval: None,
        request_timeout: None,
        fast_open: None,
        disable_sni: None,
        version: None,
        padding_range,
        idle_timeout,
        private_key: None,
        preshared_key: None,
        reserved: None,
        ip: None,
        ipv6: None,
        mtu: None,
        remote_dns_resolve: None,
        workers: None,
        persistent_keepalive: None,
        allowed_ips: None,
        amnezia_opts: None,
        peers: None,
        plugin: None,
        plugin_opts: None,
        udp_over_tcp: None,
        uot_version: None,
        username: None,
        passphrase: None,
        host_key_algorithms: None,
        dialer_proxy: None,
        smux: None,
        tfo: None,
        mptcp: None,
        udp: Some(true),
        ws_opts: None,
        grpc_opts: None,
        h2_opts: None,
        http_opts: None,
        xhttp_opts: None,
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}
