//! DUAL-05 draft ⇄ profile-node parameter mapping (05-03…05-12).
//!
//! Split from [`crate::protocol_node_projection`] to respect the business-file
//! line budget; this module owns the per-family parameter blocks and the
//! JSON/YAML sub-shape conversions, the projection module owns the field-level
//! draft⇄node mapping and the family dynamic owned-key list.

use std::collections::BTreeMap;

use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_contract::protocol_params::{
    EchParams, Hysteria2Params, ProtocolParams, TuicParams,
};
use infiltrator_contract::protocol_params_ext::{
    AmneziaWgParams, AnyTlsParams, GrpcOptsParams, H2OptsParams, HttpOptsParams, PluginOptValue,
    Sip003Plugin, SshParams, TransportParams, TrojanSsParams, WireGuardParams, WsOptsParams,
    XhttpOptsParams,
};
use infiltrator_domain::profile_converter::ProxyNodeItem;
use serde_json;
use serde_yaml_ng::{Mapping, Value};

use crate::protocol_node_projection::smux_to_json;

fn extra_u64(item: &ProxyNodeItem, key: &str) -> u64 {
    item.extra.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn extra_mapping<'a>(item: &'a ProxyNodeItem, key: &str) -> Option<&'a Mapping> {
    item.extra.get(key).and_then(Value::as_mapping)
}

fn mapping_string(mapping: &Mapping, key: &str) -> String {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn mapping_bool(mapping: &Mapping, key: &str) -> bool {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn set_extra(item: &mut ProxyNodeItem, key: &str, value: Value) {
    item.extra.insert(key.to_string(), value);
}

fn json_text(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn json_u64(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> u64 {
    object
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn json_bool(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(serde_json::Value::String(text)) => vec![text.clone()],
        _ => Vec::new(),
    }
}

fn ws_from_json(value: &serde_json::Value) -> WsOptsParams {
    let mut ws = WsOptsParams::default();
    let Some(object) = value.as_object() else {
        return ws;
    };
    ws.path = json_text(object, "path");
    if let Some(headers) = object.get("headers").and_then(serde_json::Value::as_object) {
        for (key, value) in headers {
            if let Some(text) = value.as_str() {
                ws.headers.insert(key.clone(), text.to_string());
            }
        }
    }
    ws.max_early_data = json_u64(object, "max-early-data");
    ws.early_data_header_name = json_text(object, "early-data-header-name");
    ws.v2ray_http_upgrade = json_bool(object, "v2ray-http-upgrade");
    ws.v2ray_http_upgrade_fast_open = json_bool(object, "v2ray-http-upgrade-fast-open");
    ws
}

fn ws_to_json(ws: &WsOptsParams) -> Option<serde_json::Value> {
    if !ws.is_present() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if !ws.path.trim().is_empty() {
        object.insert("path".to_string(), ws.path.trim().into());
    }
    if !ws.headers.is_empty() {
        let headers: serde_json::Map<String, serde_json::Value> = ws
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect();
        object.insert("headers".to_string(), serde_json::Value::Object(headers));
    }
    if ws.max_early_data > 0 {
        object.insert("max-early-data".to_string(), ws.max_early_data.into());
    }
    if !ws.early_data_header_name.trim().is_empty() {
        object.insert(
            "early-data-header-name".to_string(),
            ws.early_data_header_name.trim().into(),
        );
    }
    if ws.v2ray_http_upgrade {
        object.insert("v2ray-http-upgrade".to_string(), true.into());
    }
    if ws.v2ray_http_upgrade_fast_open {
        object.insert("v2ray-http-upgrade-fast-open".to_string(), true.into());
    }
    Some(serde_json::Value::Object(object))
}

fn grpc_from_json(value: &serde_json::Value) -> GrpcOptsParams {
    GrpcOptsParams {
        service_name: value
            .as_object()
            .map(|object| json_text(object, "grpc-service-name"))
            .unwrap_or_default(),
    }
}

fn grpc_to_json(grpc: &GrpcOptsParams) -> Option<serde_json::Value> {
    if !grpc.is_present() {
        return None;
    }
    Some(serde_json::json!({ "grpc-service-name": grpc.service_name.trim() }))
}

fn h2_from_json(value: &serde_json::Value) -> H2OptsParams {
    let Some(object) = value.as_object() else {
        return H2OptsParams::default();
    };
    H2OptsParams {
        hosts: string_list(object.get("host")),
        path: json_text(object, "path"),
    }
}

fn h2_to_json(h2: &H2OptsParams) -> Option<serde_json::Value> {
    if !h2.is_present() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if !h2.hosts.is_empty() {
        object.insert(
            "host".to_string(),
            serde_json::Value::Array(
                h2.hosts
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !h2.path.trim().is_empty() {
        object.insert("path".to_string(), h2.path.trim().into());
    }
    Some(serde_json::Value::Object(object))
}

fn http_from_json(value: &serde_json::Value) -> HttpOptsParams {
    let Some(object) = value.as_object() else {
        return HttpOptsParams::default();
    };
    let mut headers = BTreeMap::new();
    if let Some(map) = object.get("headers").and_then(serde_json::Value::as_object) {
        for (key, value) in map {
            headers.insert(key.clone(), string_list(Some(value)));
        }
    }
    HttpOptsParams {
        method: json_text(object, "method"),
        paths: string_list(object.get("path")),
        headers,
    }
}

fn http_to_json(http: &HttpOptsParams) -> Option<serde_json::Value> {
    if !http.is_present() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if !http.method.trim().is_empty() {
        object.insert("method".to_string(), http.method.trim().into());
    }
    if !http.paths.is_empty() {
        object.insert(
            "path".to_string(),
            serde_json::Value::Array(
                http.paths
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !http.headers.is_empty() {
        let headers: serde_json::Map<String, serde_json::Value> = http
            .headers
            .iter()
            .map(|(key, values)| {
                (
                    key.clone(),
                    serde_json::Value::Array(
                        values
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                )
            })
            .collect();
        object.insert("headers".to_string(), serde_json::Value::Object(headers));
    }
    Some(serde_json::Value::Object(object))
}

fn xhttp_from_json(value: &serde_json::Value) -> XhttpOptsParams {
    let Some(object) = value.as_object() else {
        return XhttpOptsParams::default();
    };
    XhttpOptsParams {
        mode: json_text(object, "mode"),
        path: json_text(object, "path"),
        host: json_text(object, "host"),
    }
}

fn xhttp_to_json(xhttp: &XhttpOptsParams) -> Option<serde_json::Value> {
    if !xhttp.is_present() {
        return None;
    }
    let mut object = serde_json::Map::new();
    if !xhttp.mode.trim().is_empty() {
        object.insert("mode".to_string(), xhttp.mode.trim().into());
    }
    if !xhttp.path.trim().is_empty() {
        object.insert("path".to_string(), xhttp.path.trim().into());
    }
    if !xhttp.host.trim().is_empty() {
        object.insert("host".to_string(), xhttp.host.trim().into());
    }
    Some(serde_json::Value::Object(object))
}

fn plugin_opts_from_json(value: Option<&serde_json::Value>) -> BTreeMap<String, PluginOptValue> {
    let mut opts = BTreeMap::new();
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return opts;
    };
    for (key, value) in object {
        let parsed = match value {
            serde_json::Value::Bool(flag) => Some(PluginOptValue::Bool(*flag)),
            serde_json::Value::Number(number) => number.as_i64().map(PluginOptValue::Number),
            serde_json::Value::String(text) => Some(PluginOptValue::Text(text.clone())),
            _ => None,
        };
        if let Some(parsed) = parsed {
            opts.insert(key.clone(), parsed);
        }
    }
    opts
}

fn plugin_opts_to_json(opts: &BTreeMap<String, PluginOptValue>) -> Option<serde_json::Value> {
    if opts.is_empty() {
        return None;
    }
    let object: serde_json::Map<String, serde_json::Value> = opts
        .iter()
        .map(|(key, value)| {
            let json = match value {
                PluginOptValue::Bool(flag) => serde_json::Value::Bool(*flag),
                PluginOptValue::Number(number) => serde_json::Value::Number((*number).into()),
                PluginOptValue::Text(text) => serde_json::Value::String(text.clone()),
            };
            (key.clone(), json)
        })
        .collect();
    Some(serde_json::Value::Object(object))
}

fn amnezia_from_json(value: Option<&serde_json::Value>) -> AmneziaWgParams {
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return AmneziaWgParams::default();
    };
    let u8_of = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
    };
    let u16_of = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
    };
    let u32_of = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };
    AmneziaWgParams {
        jc: u8_of("jc"),
        jmin: u16_of("jmin"),
        jmax: u16_of("jmax"),
        s1: u16_of("s1"),
        s2: u16_of("s2"),
        h1: u32_of("h1"),
        h2: u32_of("h2"),
        h3: u32_of("h3"),
        h4: u32_of("h4"),
    }
}

fn amnezia_to_json(amnezia: &AmneziaWgParams) -> Option<serde_json::Value> {
    if !amnezia.is_present() {
        return None;
    }
    let mut object = serde_json::Map::new();
    let mut put = |key: &str, value: Option<u64>| {
        if let Some(value) = value {
            object.insert(key.to_string(), value.into());
        }
    };
    put("jc", amnezia.jc.map(u64::from));
    put("jmin", amnezia.jmin.map(u64::from));
    put("jmax", amnezia.jmax.map(u64::from));
    put("s1", amnezia.s1.map(u64::from));
    put("s2", amnezia.s2.map(u64::from));
    put("h1", amnezia.h1.map(u64::from));
    put("h2", amnezia.h2.map(u64::from));
    put("h3", amnezia.h3.map(u64::from));
    put("h4", amnezia.h4.map(u64::from));
    Some(serde_json::Value::Object(object))
}

pub fn params_from_node(item: &ProxyNodeItem, family: ProtocolFamily) -> ProtocolParams {
    let ech_map = extra_mapping(item, "ech-opts");
    let ss_map = extra_mapping(item, "ss-opts");
    let anytls_idle = if item.extra.contains_key("idle-session-timeout") {
        extra_u64(item, "idle-session-timeout")
    } else {
        item.idle_timeout.unwrap_or(0)
    };
    let amnezia = item
        .amnezia_opts
        .as_ref()
        .map(|value| amnezia_from_json(Some(value)))
        .unwrap_or_default();
    let mut params = ProtocolParams::default();
    if EchParams::supported_by(family) {
        params.ech = EchParams {
            enabled: ech_map
                .map(|map| mapping_bool(map, "enable"))
                .unwrap_or(false),
            config: ech_map
                .map(|map| mapping_string(map, "config"))
                .unwrap_or_default(),
        };
    }
    if family == ProtocolFamily::Tuic {
        params.tuic = TuicParams {
            congestion_controller: item.congestion_controller.clone().unwrap_or_default(),
            udp_relay_mode: item.udp_relay_mode.clone().unwrap_or_default(),
            reduce_rtt: item.reduce_rtt.unwrap_or(false),
            heartbeat_interval: item.heartbeat_interval.unwrap_or(0),
            request_timeout: item.request_timeout.unwrap_or(0),
            recv_window_conn: item.recv_window_conn.unwrap_or(0),
            recv_window: item.recv_window.unwrap_or(0),
            disable_sni: item.disable_sni.unwrap_or(false),
        };
    }
    if family == ProtocolFamily::Hysteria2 {
        params.hysteria2 = Hysteria2Params {
            ports: item.ports.clone().unwrap_or_default(),
            hop_interval: item.hop_interval.unwrap_or(0),
            obfs: item.obfs.clone().unwrap_or_default(),
            obfs_password: item.obfs_password.clone().unwrap_or_default(),
            up: item.up.clone().unwrap_or_default(),
            down: item.down.clone().unwrap_or_default(),
            cwnd: item.cwnd.unwrap_or(0),
            udp_mtu: extra_u64(item, "udp-mtu"),
        };
    }
    if family == ProtocolFamily::WireGuard {
        params.wireguard = WireGuardParams {
            private_key: item.private_key.clone().unwrap_or_default(),
            public_key: item.public_key.clone().unwrap_or_default(),
            pre_shared_key: item.preshared_key.clone().unwrap_or_default(),
            reserved: item.reserved_text().unwrap_or_default(),
            reserved_is_base64: item.reserved_is_base64(),
            ip: item.ip.clone().unwrap_or_default(),
            ipv6: item.ipv6.clone().unwrap_or_default(),
            mtu: item.mtu.map(u32::from).unwrap_or(0),
            dns: match item.extra.get("dns") {
                Some(Value::Sequence(items)) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                _ => Vec::new(),
            },
            workers: item.workers.unwrap_or(0),
            persistent_keepalive: item.persistent_keepalive.unwrap_or(0),
            allowed_ips: item.allowed_ips.clone().unwrap_or_default(),
            remote_dns_resolve: item.remote_dns_resolve.unwrap_or(false),
            amnezia,
        };
    }
    if TransportParams::supported_by(family) {
        params.transport = TransportParams {
            network: item.network.clone().unwrap_or_default(),
            packet_encoding: item.packet_encoding.clone().unwrap_or_default(),
            ws: item.ws_opts.as_ref().map(ws_from_json).unwrap_or_default(),
            grpc: item
                .grpc_opts
                .as_ref()
                .map(grpc_from_json)
                .unwrap_or_default(),
            h2: item.h2_opts.as_ref().map(h2_from_json).unwrap_or_default(),
            http: item
                .http_opts
                .as_ref()
                .map(http_from_json)
                .unwrap_or_default(),
            xhttp: item
                .xhttp_opts
                .as_ref()
                .map(xhttp_from_json)
                .unwrap_or_default(),
        };
    }
    if family == ProtocolFamily::Shadowsocks {
        params.plugin = Sip003Plugin {
            name: item.plugin.clone().unwrap_or_default(),
            opts: plugin_opts_from_json(item.plugin_opts.as_ref()),
        };
    }
    if family == ProtocolFamily::Ssh {
        params.ssh = SshParams {
            username: item.username.clone().unwrap_or_default(),
            private_key: item.private_key.clone().unwrap_or_default(),
            passphrase: item.passphrase.clone().unwrap_or_default(),
            host_key_algorithms: item.host_key_algorithms.clone().unwrap_or_default(),
        };
    }
    if family == ProtocolFamily::Anytls {
        params.anytls = AnyTlsParams {
            idle_session_timeout: anytls_idle,
            idle_session_check_interval: extra_u64(item, "idle-session-check-interval"),
            min_idle_session: extra_u64(item, "min-idle-session"),
        };
    }
    if family == ProtocolFamily::Trojan {
        params.trojan_ss = TrojanSsParams {
            enabled: ss_map
                .map(|map| mapping_bool(map, "enabled"))
                .unwrap_or(false),
            method: ss_map
                .map(|map| mapping_string(map, "method"))
                .unwrap_or_default(),
            password: ss_map
                .map(|map| mapping_string(map, "password"))
                .unwrap_or_default(),
        };
    }
    // DUAL-05-13: certificate trust. `fingerprint` is a real v1.19.18 node key;
    // `ca`/`ca-str` are read back so a forward-compatible profile node keeps
    // them losslessly even though the pinned node schema has neither.
    params.tls_trust = infiltrator_contract::protocol_trust::TlsTrustParams {
        ca_path: item
            .extra
            .get("ca")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        ca_str: item
            .extra
            .get("ca-str")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        fingerprint: item.fingerprint.clone().unwrap_or_default(),
    };
    params
}

/// DUAL-05: project the draft back into the flat profile node shape.
pub fn node_from_draft(draft: &ProtocolDraft) -> ProxyNodeItem {
    let mut item = ProxyNodeItem::new(
        draft.name.trim(),
        draft.node_type.trim(),
        draft.server.trim(),
        draft.port,
    );
    // `udp` is not part of the draft: a replaced node keeps whatever the
    // profile already said, and a new node omits the key entirely.
    item.udp = None;
    item.password = non_empty(&draft.password);
    item.uuid = non_empty(&draft.uuid);
    item.cipher = non_empty(&draft.cipher);
    item.flow = non_empty(&draft.flow);
    item.client_fingerprint = non_empty(&draft.reality.fingerprint);
    item.servername = non_empty(&draft.sni);
    item.tls = draft.tls;
    item.skip_cert_verify = draft.skip_cert_verify.then_some(true);
    if !draft.alpn.is_empty() {
        item.alpn = Some(draft.alpn.clone());
    }
    if draft.reality.is_present() {
        let mut reality = Mapping::new();
        if !draft.reality.public_key.trim().is_empty() {
            reality.insert(
                Value::String("public-key".to_string()),
                Value::String(draft.reality.public_key.trim().to_string()),
            );
        }
        if !draft.reality.short_id.trim().is_empty() {
            reality.insert(
                Value::String("short-id".to_string()),
                Value::String(draft.reality.short_id.trim().to_string()),
            );
        }
        if !draft.reality.spider_x.trim().is_empty() {
            reality.insert(
                Value::String("spider-x".to_string()),
                Value::String(draft.reality.spider_x.trim().to_string()),
            );
        }
        item.reality_opts = Some(serde_json::to_value(Value::Mapping(reality)).unwrap_or_default());
    }
    if draft.smux.has_overrides() {
        item.smux = Some(smux_to_json(&draft.smux));
    }
    // DUAL-05-09: the static dialer hop.
    item.dialer_proxy = non_empty(&draft.dialer_proxy);
    write_params(&mut item, &draft.params, draft.family());
    item
}

/// Write every typed parameter block onto the flat node. Blocks are gated by
/// the node family so two families sharing a flat field (`public_key`,
/// `private_key`, `ip`) can never leak into each other. Only blocks that carry
/// a value are emitted, so an empty draft never fabricates keys.
fn write_params(item: &mut ProxyNodeItem, params: &ProtocolParams, family: ProtocolFamily) {
    if EchParams::supported_by(family) && params.ech.is_present() {
        let mut map = Mapping::new();
        map.insert(
            Value::String("enable".to_string()),
            Value::Bool(params.ech.enabled),
        );
        if !params.ech.config.trim().is_empty() {
            map.insert(
                Value::String("config".to_string()),
                Value::String(params.ech.config.trim().to_string()),
            );
        }
        set_extra(item, "ech-opts", Value::Mapping(map));
    }

    if family == ProtocolFamily::Tuic {
        let tuic = &params.tuic;
        item.congestion_controller = non_empty(&tuic.congestion_controller);
        item.udp_relay_mode = non_empty(&tuic.udp_relay_mode);
        item.reduce_rtt = tuic.reduce_rtt.then_some(true);
        item.heartbeat_interval = (tuic.heartbeat_interval > 0).then_some(tuic.heartbeat_interval);
        item.request_timeout = (tuic.request_timeout > 0).then_some(tuic.request_timeout);
        item.recv_window_conn = (tuic.recv_window_conn > 0).then_some(tuic.recv_window_conn);
        item.recv_window = (tuic.recv_window > 0).then_some(tuic.recv_window);
        item.disable_sni = tuic.disable_sni.then_some(true);
    }

    if family == ProtocolFamily::Hysteria2 {
        let hy2 = &params.hysteria2;
        item.ports = non_empty(&hy2.ports);
        item.hop_interval = (hy2.hop_interval > 0).then_some(hy2.hop_interval);
        item.obfs = non_empty(&hy2.obfs);
        item.obfs_password = non_empty(&hy2.obfs_password);
        item.up = non_empty(&hy2.up);
        item.down = non_empty(&hy2.down);
        item.cwnd = (hy2.cwnd > 0).then_some(hy2.cwnd);
        if hy2.udp_mtu > 0 {
            set_extra(item, "udp-mtu", Value::Number(hy2.udp_mtu.into()));
        }
    }

    if family == ProtocolFamily::WireGuard {
        let wg = &params.wireguard;
        item.private_key = non_empty(&wg.private_key);
        item.public_key = non_empty(&wg.public_key);
        item.preshared_key = non_empty(&wg.pre_shared_key);
        if !wg.reserved.trim().is_empty() {
            item.reserved = Some(if wg.reserved_is_base64 {
                infiltrator_domain::profile_converter::ReservedField::Base64(
                    wg.reserved.trim().to_string(),
                )
            } else {
                infiltrator_domain::profile_converter::ReservedField::Array(
                    wg.reserved
                        .split(',')
                        .filter_map(|part| part.trim().parse::<u8>().ok())
                        .collect(),
                )
            });
        }
        item.ip = non_empty(&wg.ip);
        item.ipv6 = non_empty(&wg.ipv6);
        item.mtu = (wg.mtu > 0).then_some(wg.mtu as u16);
        item.workers = (wg.workers > 0).then_some(wg.workers);
        item.persistent_keepalive =
            (wg.persistent_keepalive > 0).then_some(wg.persistent_keepalive);
        if !wg.allowed_ips.is_empty() {
            item.allowed_ips = Some(wg.allowed_ips.clone());
        }
        item.remote_dns_resolve = wg.remote_dns_resolve.then_some(true);
        item.amnezia_opts = amnezia_to_json(&wg.amnezia);
        if !wg.dns.is_empty() {
            set_extra(
                item,
                "dns",
                Value::Sequence(
                    wg.dns
                        .iter()
                        .map(|entry| Value::String(entry.trim().to_string()))
                        .collect(),
                ),
            );
        }
    }

    if TransportParams::supported_by(family) {
        let transport = &params.transport;
        item.network = non_empty(&transport.network);
        item.packet_encoding = non_empty(&transport.packet_encoding);
        item.ws_opts = ws_to_json(&transport.ws);
        item.grpc_opts = grpc_to_json(&transport.grpc);
        item.h2_opts = h2_to_json(&transport.h2);
        item.http_opts = http_to_json(&transport.http);
        item.xhttp_opts = xhttp_to_json(&transport.xhttp);
    }

    if family == ProtocolFamily::Shadowsocks {
        item.plugin = non_empty(&params.plugin.name);
        item.plugin_opts = plugin_opts_to_json(&params.plugin.opts);
    }

    if family == ProtocolFamily::Ssh {
        let ssh = &params.ssh;
        item.username = non_empty(&ssh.username);
        item.private_key = non_empty(&ssh.private_key);
        item.passphrase = non_empty(&ssh.passphrase);
        if !ssh.host_key_algorithms.is_empty() {
            item.host_key_algorithms = Some(ssh.host_key_algorithms.clone());
        }
    }

    if family == ProtocolFamily::Anytls {
        let anytls = &params.anytls;
        if anytls.idle_session_timeout > 0 {
            item.idle_timeout = Some(anytls.idle_session_timeout);
        }
        if anytls.idle_session_check_interval > 0 {
            set_extra(
                item,
                "idle-session-check-interval",
                Value::Number(anytls.idle_session_check_interval.into()),
            );
        }
        if anytls.min_idle_session > 0 {
            set_extra(
                item,
                "min-idle-session",
                Value::Number(anytls.min_idle_session.into()),
            );
        }
    }

    if family == ProtocolFamily::Trojan {
        let trojan_ss = &params.trojan_ss;
        if trojan_ss.is_present() {
            let mut map = Mapping::new();
            map.insert(
                Value::String("enabled".to_string()),
                Value::Bool(trojan_ss.enabled),
            );
            if !trojan_ss.method.trim().is_empty() {
                map.insert(
                    Value::String("method".to_string()),
                    Value::String(trojan_ss.method.trim().to_string()),
                );
            }
            if !trojan_ss.password.is_empty() {
                map.insert(
                    Value::String("password".to_string()),
                    Value::String(trojan_ss.password.clone()),
                );
            }
            set_extra(item, "ss-opts", Value::Mapping(map));
        }
    }

    // DUAL-05-13: certificate trust. `fingerprint` is a real v1.19.18 node key;
    // `ca`/`ca-str` are emitted only when the draft carries them (the pinned
    // node schema has neither, the shared report says so, and the file-path
    // anchor is additionally projected into the global tls list).
    item.fingerprint = non_empty(&params.tls_trust.fingerprint);
    if let Some(ca) = non_empty(&params.tls_trust.ca_path) {
        set_extra(item, "ca", Value::String(ca));
    }
    if let Some(ca_str) = non_empty(&params.tls_trust.ca_str) {
        set_extra(item, "ca-str", Value::String(ca_str));
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
