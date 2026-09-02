//! URI exporting implementations for proxy protocols.

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;

use crate::profile_converter::ProxyNodeItem;

/// Exports a [`ProxyNodeItem`] to a canonical URI string.
pub fn export_uri(node: &ProxyNodeItem) -> Result<String> {
    match node.node_type.as_str() {
        "ss" => export_shadowsocks(node),
        "trojan" => export_trojan(node),
        "vless" => export_vless(node),
        "vmess" => export_vmess(node),
        "hysteria2" | "hy2" => export_hysteria2(node),
        "tuic" => export_tuic(node),
        "wireguard" | "wg" | "awg" | "amnezia-wg" => export_wireguard(node),
        "anytls" => export_anytls(node),
        "ssh" => export_ssh(node),
        _ => Err(anyhow!(
            "Unsupported export scheme for type: {}",
            node.node_type
        )),
    }
}

fn export_shadowsocks(node: &ProxyNodeItem) -> Result<String> {
    let cipher = node.cipher.as_deref().unwrap_or("aes-128-gcm");
    let password = node.password.as_deref().unwrap_or_default();
    let userinfo = STANDARD.encode(format!("{cipher}:{password}"));

    let mut uri = format!("ss://{}@{}:{}", userinfo, node.server, node.port);

    let mut params = Vec::new();
    if let Some(ref plugin) = node.plugin {
        let mut plugin_val = plugin.clone();
        if let Some(ref opts) = node.plugin_opts
            && let Some(obj) = opts.as_object()
        {
            for (k, v) in obj {
                if let Some(b) = v.as_bool() {
                    if b {
                        plugin_val.push_str(&format!(";{}", k));
                    }
                } else if let Some(s) = v.as_str() {
                    plugin_val.push_str(&format!(";{}={}", k, s));
                }
            }
        }
        params.push(format!("plugin={}", urlencoding::encode(&plugin_val)));
    }

    if let Some(uot_v) = node.uot_version {
        params.push(format!("uot={uot_v}"));
    } else if let Some(uot) = node.udp_over_tcp
        && uot
    {
        params.push("uot=1".to_string());
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_trojan(node: &ProxyNodeItem) -> Result<String> {
    let password = node.password.as_deref().unwrap_or_default();
    let mut uri = format!(
        "trojan://{}@{}:{}",
        urlencoding::encode(password),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(sni) = node.sni.as_ref().or(node.servername.as_ref()) {
        params.push(format!("sni={}", urlencoding::encode(sni)));
    }
    if let Some(ref alpn) = node.alpn {
        params.push(format!("alpn={}", urlencoding::encode(&alpn.join(","))));
    }
    if let Some(ref fp) = node.client_fingerprint {
        params.push(format!("fp={}", urlencoding::encode(fp)));
    }
    if let Some(ref net) = node.network {
        params.push(format!("type={}", urlencoding::encode(net)));
    }
    if let Some(p) = node.get_ws_path() {
        params.push(format!("path={}", urlencoding::encode(p)));
    }
    if let Some(h) = node.get_ws_host() {
        params.push(format!("host={}", urlencoding::encode(h)));
    }
    if let Some(skip) = node.skip_cert_verify
        && skip
    {
        params.push("allowInsecure=1".to_string());
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_vless(node: &ProxyNodeItem) -> Result<String> {
    let uuid = node.uuid.as_deref().unwrap_or_default();
    let mut uri = format!(
        "vless://{}@{}:{}",
        urlencoding::encode(uuid),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(ref flow) = node.flow {
        params.push(format!("flow={}", urlencoding::encode(flow)));
    }
    if let Some(ref fp) = node.client_fingerprint {
        params.push(format!("fp={}", urlencoding::encode(fp)));
    }
    if let Some(sni) = node.sni.as_ref().or(node.servername.as_ref()) {
        params.push(format!("sni={}", urlencoding::encode(sni)));
    }
    if let Some(ref net) = node.network {
        params.push(format!("type={}", urlencoding::encode(net)));
    }
    if let Some(pk) = node.get_effective_public_key() {
        params.push(format!("pbk={}", urlencoding::encode(pk)));
        params.push("security=reality".to_string());
    } else if node.tls {
        params.push("security=tls".to_string());
    }
    if let Some(sid) = node.get_effective_short_id() {
        params.push(format!("sid={}", urlencoding::encode(sid)));
    }
    if let Some(ref spx) = node.spider_x {
        params.push(format!("spx={}", urlencoding::encode(spx)));
    }
    if let Some(ref pe) = node.packet_encoding {
        params.push(format!("packetEncoding={}", urlencoding::encode(pe)));
    }
    if let Some(sn) = node.get_grpc_service_name() {
        params.push(format!("serviceName={}", urlencoding::encode(sn)));
    }
    if let Some(p) = node.get_ws_path() {
        params.push(format!("path={}", urlencoding::encode(p)));
    }
    if let Some(h) = node.get_ws_host() {
        params.push(format!("host={}", urlencoding::encode(h)));
    }
    if let Some(m) = node.get_xhttp_mode() {
        params.push(format!("mode={}", urlencoding::encode(m)));
    }
    if let Some(p) = node.get_xhttp_path() {
        params.push(format!("path={}", urlencoding::encode(p)));
    }
    if let Some(skip) = node.skip_cert_verify
        && skip
    {
        params.push("allowInsecure=1".to_string());
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_vmess(node: &ProxyNodeItem) -> Result<String> {
    let mut payload = json!({
        "v": "2",
        "ps": node.name,
        "add": node.server,
        "port": node.port,
        "id": node.uuid.as_deref().unwrap_or_default(),
        "scy": node.cipher.as_deref().unwrap_or("auto"),
        "net": node.network.as_deref().unwrap_or("tcp"),
        "tls": if node.tls { "tls" } else { "none" },
        "sni": node.servername.as_deref().unwrap_or_default(),
    });

    if let Some(ref fp) = node.client_fingerprint {
        payload["fp"] = json!(fp);
    }
    if let Some(ref pe) = node.packet_encoding {
        payload["packetEncoding"] = json!(pe);
    }

    if let Some(aid) = node.extra.get("alterId").and_then(|v| v.as_u64()) {
        payload["aid"] = json!(aid);
    }

    let encoded = STANDARD.encode(serde_json::to_string(&payload)?);
    Ok(format!("vmess://{}", encoded))
}

fn export_hysteria2(node: &ProxyNodeItem) -> Result<String> {
    let auth = node
        .auth
        .as_deref()
        .or(node.password.as_deref())
        .unwrap_or_default();
    let mut uri = format!(
        "hysteria2://{}@{}:{}",
        urlencoding::encode(auth),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(sni) = node.sni.as_ref().or(node.servername.as_ref()) {
        params.push(format!("sni={}", urlencoding::encode(sni)));
    }
    if let Some(ref obfs) = node.obfs {
        params.push(format!("obfs={}", urlencoding::encode(obfs)));
    }
    if let Some(ref obfs_pw) = node.obfs_password {
        params.push(format!("obfs-password={}", urlencoding::encode(obfs_pw)));
    }
    if let Some(ref ports) = node.ports {
        params.push(format!("mport={}", urlencoding::encode(ports)));
    }
    if let Some(skip) = node.skip_cert_verify
        && skip
    {
        params.push("insecure=1".to_string());
    }
    if let Some(ref up) = node.up {
        params.push(format!("up={}", urlencoding::encode(up)));
    }
    if let Some(ref down) = node.down {
        params.push(format!("down={}", urlencoding::encode(down)));
    }
    if let Some(cwnd) = node.cwnd {
        params.push(format!("cwnd={cwnd}"));
    }
    if let Some(hop) = node.hop_interval {
        params.push(format!("hop_interval={hop}"));
    }
    if let Some(fo) = node.fast_open
        && fo
    {
        params.push("fast_open=1".to_string());
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_tuic(node: &ProxyNodeItem) -> Result<String> {
    let uuid = node.uuid.as_deref().unwrap_or_default();
    let password = node.password.as_deref().unwrap_or_default();
    let mut uri = format!(
        "tuic://{}:{}@{}:{}",
        urlencoding::encode(uuid),
        urlencoding::encode(password),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(ref cc) = node.congestion_controller {
        params.push(format!("congestion_controller={}", urlencoding::encode(cc)));
    }
    if let Some(ref mode) = node.udp_relay_mode {
        params.push(format!("udp_relay_mode={}", urlencoding::encode(mode)));
    }
    if let Some(ref alpn) = node.alpn {
        params.push(format!("alpn={}", urlencoding::encode(&alpn.join(","))));
    }
    if let Some(sni) = node.sni.as_ref().or(node.servername.as_ref()) {
        params.push(format!("sni={}", urlencoding::encode(sni)));
    }
    if let Some(skip) = node.skip_cert_verify
        && skip
    {
        params.push("allowInsecure=1".to_string());
    }
    if let Some(rtt) = node.reduce_rtt
        && rtt
    {
        params.push("reduce_rtt=1".to_string());
    }
    if let Some(hb) = node.heartbeat_interval {
        params.push(format!("heartbeat_interval={hb}"));
    }
    if let Some(timeout) = node.request_timeout {
        params.push(format!("request_timeout={timeout}"));
    }
    if let Some(fo) = node.fast_open
        && fo
    {
        params.push("fast_open=1".to_string());
    }
    if let Some(dis) = node.disable_sni
        && dis
    {
        params.push("disable_sni=1".to_string());
    }
    if let Some(ref ip) = node.ip {
        params.push(format!("ip={}", urlencoding::encode(ip)));
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_wireguard(node: &ProxyNodeItem) -> Result<String> {
    let pk = node.private_key.as_deref().unwrap_or_default();
    let scheme = if node.amnezia_opts.is_some() {
        "awg"
    } else {
        "wireguard"
    };
    let mut uri = format!(
        "{}://{}@{}:{}",
        scheme,
        urlencoding::encode(pk),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(ref pubkey) = node.public_key {
        params.push(format!("public_key={}", urlencoding::encode(pubkey)));
    }
    if let Some(ref psk) = node.preshared_key {
        params.push(format!("preshared_key={}", urlencoding::encode(psk)));
    }
    if let Some(ref ip) = node.ip {
        params.push(format!("ip={}", urlencoding::encode(ip)));
    }
    if let Some(ref ipv6) = node.ipv6 {
        params.push(format!("ipv6={}", urlencoding::encode(ipv6)));
    }
    if let Some(mtu) = node.mtu {
        params.push(format!("mtu={}", mtu));
    }
    if let Some(workers) = node.workers {
        params.push(format!("workers={workers}"));
    }
    if let Some(ka) = node.persistent_keepalive {
        params.push(format!("persistent_keepalive={ka}"));
    }
    if let Some(ref reserved) = node.reserved {
        let r_str = reserved
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",");
        params.push(format!("reserved={}", r_str));
    }

    if let Some(ref awg) = node.amnezia_opts
        && let Some(obj) = awg.as_object()
    {
        for (k, v) in obj {
            if let Some(num) = v.as_u64() {
                params.push(format!("{}={}", k, num));
            } else if let Some(s) = v.as_str() {
                params.push(format!("{}={}", k, urlencoding::encode(s)));
            }
        }
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_anytls(node: &ProxyNodeItem) -> Result<String> {
    let auth = node
        .password
        .as_deref()
        .or(node.uuid.as_deref())
        .unwrap_or_default();
    let mut uri = format!(
        "anytls://{}@{}:{}",
        urlencoding::encode(auth),
        node.server,
        node.port
    );

    let mut params = Vec::new();
    if let Some(sni) = node.sni.as_ref().or(node.servername.as_ref()) {
        params.push(format!("sni={}", urlencoding::encode(sni)));
    }
    if let Some(ref alpn) = node.alpn {
        params.push(format!("alpn={}", urlencoding::encode(&alpn.join(","))));
    }
    if let Some(ref fp) = node.client_fingerprint {
        params.push(format!("fp={}", urlencoding::encode(fp)));
    }
    if let Some(ref pr) = node.padding_range {
        params.push(format!("padding={}", urlencoding::encode(pr)));
    }
    if let Some(idle) = node.idle_timeout {
        params.push(format!("idle_timeout={idle}"));
    }
    if let Some(skip) = node.skip_cert_verify
        && skip
    {
        params.push("insecure=1".to_string());
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}

fn export_ssh(node: &ProxyNodeItem) -> Result<String> {
    let user = node.username.as_deref().unwrap_or("root");
    let mut uri = if let Some(ref pw) = node.password {
        format!(
            "ssh://{}:{}@{}:{}",
            urlencoding::encode(user),
            urlencoding::encode(pw),
            node.server,
            node.port
        )
    } else {
        format!(
            "ssh://{}@{}:{}",
            urlencoding::encode(user),
            node.server,
            node.port
        )
    };

    let mut params = Vec::new();
    if let Some(ref pk) = node.private_key {
        params.push(format!("private_key={}", urlencoding::encode(pk)));
    }
    if let Some(ref pp) = node.passphrase {
        params.push(format!("passphrase={}", urlencoding::encode(pp)));
    }
    if let Some(ref algs) = node.host_key_algorithms {
        params.push(format!(
            "host_key_algorithms={}",
            urlencoding::encode(&algs.join(","))
        ));
    }

    if !params.is_empty() {
        uri.push('?');
        uri.push_str(&params.join("&"));
    }

    uri.push('#');
    uri.push_str(&urlencoding::encode(&node.name));
    Ok(uri)
}
