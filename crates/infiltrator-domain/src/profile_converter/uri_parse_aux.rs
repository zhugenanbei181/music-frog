//! Auxiliary URI parsing implementations for Shadowsocks, VMess, Trojan, and SSH.

use anyhow::{Result, anyhow};
use serde_json::Value;
use url::Url;

use crate::profile_converter::{ProxyNodeItem, decode_base64_flexible};

pub(crate) fn parse_vmess_uri(encoded: &str) -> Result<ProxyNodeItem> {
    let json_str = decode_base64_flexible(encoded)?;
    let val: Value =
        serde_json::from_str(&json_str).map_err(|e| anyhow!("Invalid VMess JSON: {e}"))?;

    let name = val["ps"].as_str().unwrap_or("VMess Node").to_string();
    let server = val["add"].as_str().unwrap_or_default().to_string();
    let port = if let Some(p) = val["port"].as_u64() {
        p as u16
    } else if let Some(p_str) = val["port"].as_str() {
        p_str.parse::<u16>().unwrap_or(443)
    } else {
        443
    };
    let uuid = val["id"].as_str().map(|s| s.to_string());
    let cipher = val["scy"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| Some("auto".to_string()));
    let tls = val["tls"].as_str().map(|s| s == "tls").unwrap_or(false);
    let network = val["net"].as_str().map(|s| s.to_string());
    let servername = val["sni"]
        .as_str()
        .or_else(|| val["host"].as_str())
        .map(|s| s.to_string());
    let client_fingerprint = val["fp"].as_str().map(|s| s.to_string());
    let packet_encoding = val["packetEncoding"].as_str().map(|s| s.to_string());

    let mut extra = std::collections::BTreeMap::new();
    if let Some(aid) = val["aid"].as_u64() {
        extra.insert(
            "alterId".to_string(),
            serde_yaml_ng::Value::Number(aid.into()),
        );
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "vmess".to_string(),
        uuid,
        cipher,
        tls,
        client_fingerprint,
        servername,
        packet_encoding,
        network,
        udp: Some(true),
        extra,
        ..Default::default()
    })
}

pub(crate) fn parse_shadowsocks(parsed: &Url) -> Result<ProxyNodeItem> {
    let raw_username = parsed.username();
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "Shadowsocks Node".to_string());

    let (cipher, password) = if raw_username.contains(':') {
        let (c, p) = raw_username.split_once(':').unwrap();
        let decoded_c = urlencoding::decode(c).unwrap_or_default().to_string();
        let decoded_p = urlencoding::decode(p).unwrap_or_default().to_string();
        (Some(decoded_c), Some(decoded_p))
    } else if !raw_username.is_empty() {
        let unescaped = urlencoding::decode(raw_username).unwrap_or_default();
        let decoded_str = decode_base64_flexible(&unescaped).unwrap_or_default();
        if let Some((c, p)) = decoded_str.split_once(':') {
            (Some(c.to_string()), Some(p.to_string()))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(8388);

    let mut plugin = None;
    let mut plugin_opts = None;
    let mut udp_over_tcp = None;
    let mut uot_version = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "plugin" => {
                let decoded_plugin = urlencoding::decode(&v).unwrap_or_default().to_string();
                if let Some((p, opts_str)) = decoded_plugin.split_once(';') {
                    plugin = Some(p.to_string());
                    let mut opts_map = serde_json::Map::new();
                    for opt in opts_str.split(';') {
                        if opt.is_empty() {
                            continue;
                        }
                        if let Some((opt_k, opt_v)) = opt.split_once('=') {
                            if opt_v.eq_ignore_ascii_case("true") || opt_v == "1" {
                                opts_map.insert(opt_k.to_string(), Value::Bool(true));
                            } else if opt_v.eq_ignore_ascii_case("false") || opt_v == "0" {
                                opts_map.insert(opt_k.to_string(), Value::Bool(false));
                            } else {
                                opts_map
                                    .insert(opt_k.to_string(), Value::String(opt_v.to_string()));
                            }
                        } else {
                            opts_map.insert(opt.to_string(), Value::Bool(true));
                        }
                    }
                    plugin_opts = Some(Value::Object(opts_map));
                } else {
                    plugin = Some(decoded_plugin);
                }
            }
            "uot" | "udp-over-tcp" => {
                if v == "1" || v.eq_ignore_ascii_case("true") {
                    udp_over_tcp = Some(true);
                    uot_version = Some(1);
                } else if v == "2" {
                    udp_over_tcp = Some(true);
                    uot_version = Some(2);
                }
            }
            _ => {}
        }
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "ss".to_string(),
        password,
        cipher,
        plugin,
        plugin_opts,
        udp_over_tcp,
        uot_version,
        udp: Some(true),
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}

pub(crate) fn parse_trojan(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "Trojan Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(443);
    let password = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };

    let mut sni = None;
    let mut alpn = None;
    let mut skip_cert_verify = None;
    let mut network = None;
    let mut path = None;
    let mut host = None;
    let mut client_fingerprint = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" => sni = Some(v.to_string()),
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).collect()),
            "allowInsecure" | "insecure" => {
                skip_cert_verify = Some(v == "1" || v.eq_ignore_ascii_case("true"))
            }
            "type" => network = Some(v.to_string()),
            "path" => path = Some(v.to_string()),
            "host" => host = Some(v.to_string()),
            "fp" => client_fingerprint = Some(v.to_string()),
            _ => {}
        }
    }

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

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "trojan".to_string(),
        password,
        tls: true,
        client_fingerprint,
        servername: sni.clone(),
        sni,
        alpn,
        skip_cert_verify,
        network,
        udp: Some(true),
        ws_opts,
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}

pub(crate) fn parse_ssh(parsed: &Url) -> Result<ProxyNodeItem> {
    let name = parsed
        .fragment()
        .map(|f| urlencoding::decode(f).unwrap_or_default().to_string())
        .unwrap_or_else(|| "SSH Node".to_string());
    let server = parsed.host_str().unwrap_or_default().to_string();
    let port = parsed.port().unwrap_or(22);
    let username = if !parsed.username().is_empty() {
        Some(urlencoding::decode(parsed.username()).unwrap_or_default().to_string())
    } else {
        None
    };
    let password = parsed
        .password()
        .map(|p| urlencoding::decode(p).unwrap_or_default().to_string());

    let mut private_key = None;
    let mut passphrase = None;
    let mut host_key_algorithms = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "private_key" | "privatekey" | "pk" => private_key = Some(v.to_string()),
            "passphrase" | "pp" => passphrase = Some(v.to_string()),
            "host_key_algorithms" | "hostkeyalgorithms" => {
                host_key_algorithms = Some(v.split(',').map(|s| s.trim().to_string()).collect());
            }
            _ => {}
        }
    }

    Ok(ProxyNodeItem {
        name,
        server,
        port,
        node_type: "ssh".to_string(),
        password,
        private_key,
        username,
        passphrase,
        host_key_algorithms,
        udp: Some(true),
        extra: std::collections::BTreeMap::new(),
        ..Default::default()
    })
}
