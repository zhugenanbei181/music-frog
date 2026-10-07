//! Typed protocol-field folding. Invalid edits preserve the previous value.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_contract::protocol_form::ProtocolField;
use infiltrator_contract::protocol_params::EchParams;
use infiltrator_contract::protocol_params_ext::{PluginOptValue, TransportParams};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolFieldProjection {
    pub id: ProtocolField,
    pub label_key: &'static str,
    pub value: String,
    pub toggle: bool,
}

fn invalid() -> Failure {
    Failure::new(
        ErrorCode::InvalidInput,
        "invalid protocol field value",
        false,
    )
}
fn parse<T: FromStr>(value: &str) -> Result<T, Failure> {
    value.trim().parse().map_err(|_| invalid())
}
fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}

macro_rules! value {
    (text, $v:expr) => {
        $v.clone()
    };
    (list, $v:expr) => {
        $v.join(",")
    };
    (number, $v:expr) => {
        $v.to_string()
    };
    (toggle, $v:expr) => {
        $v.to_string()
    };
    (optional, $v:expr) => {
        $v.map(|v| v.to_string()).unwrap_or_default()
    };
}
macro_rules! edit {
    (text, $target:expr, $v:expr) => {
        $target = $v.to_owned()
    };
    (list, $target:expr, $v:expr) => {
        $target = list($v)
    };
    (number, $target:expr, $v:expr) => {
        $target = parse($v)?
    };
    (toggle, $target:expr, $v:expr) => {
        $target = parse($v)?
    };
    (optional, $target:expr, $v:expr) => {
        $target = if $v.trim().is_empty() {
            None
        } else {
            Some(parse($v)?)
        }
    };
}
macro_rules! toggle_kind {
    (toggle) => {
        true
    };
    ($other:ident) => {
        false
    };
}
macro_rules! native_fields {
    ($($id:ident => ($key:literal, $kind:ident, $group:ident, $($path:ident).+)),+ $(,)?) => {
        pub fn field_projection(id: ProtocolField, draft: &ProtocolDraft) -> Option<ProtocolFieldProjection> {
            match id {
                $( ProtocolField::$id => Some(ProtocolFieldProjection {
                    id, label_key: $key, value: if id == ProtocolField::Secret { draft.password_or_uuid() } else { value!($kind, draft.$($path).+) },
                    toggle: toggle_kind!($kind),
                }), )+
                _ => None,
            }
        }
        fn group(id: ProtocolField) -> FieldGroup {
            match id {
                $( ProtocolField::$id => FieldGroup::$group, )+
                ProtocolField::WsHost => FieldGroup::Transport,
                ProtocolField::PluginHost | ProtocolField::PluginPassword | ProtocolField::PluginMode => FieldGroup::Shadowsocks,
            }
        }
        fn apply_regular(draft: &mut ProtocolDraft, id: ProtocolField, raw: &str) -> Result<bool, Failure> {
            match id {
                $( ProtocolField::$id => { edit!($kind, draft.$($path).+, raw); }, )+
                _ => return Ok(false),
            }
            Ok(true)
        }
    };
}
#[derive(Clone, Copy)]
enum FieldGroup {
    Common,
    Ech,
    Tuic,
    Hysteria2,
    WireGuard,
    Transport,
    Shadowsocks,
    Ssh,
    Anytls,
    Trojan,
}

native_fields! {
    Name => ("custom_node_name", text, Common, name),
    Type => ("custom_node_type", text, Common, node_type),
    Server => ("custom_node_server", text, Common, server),
    Port => ("custom_node_port", number, Common, port),
    Secret => ("custom_node_secret", text, Common, uuid),
    Password => ("custom_node_uuid_pass", text, Common, password),
    Sni => ("custom_node_sni", text, Common, sni),
    Cipher => ("custom_node_cipher", text, Common, cipher),
    Flow => ("custom_node_flow", text, Common, flow),
    Tls => ("custom_node_tls", toggle, Common, tls),
    SkipVerify => ("custom_node_skip_verify", toggle, Common, skip_cert_verify),
    Alpn => ("custom_node_alpn", list, Common, alpn),
    MuxEnabled => ("custom_node_mux_enabled", toggle, Common, smux.enabled),
    MuxProtocol => ("custom_node_mux_protocol", text, Common, smux.protocol),
    MuxMax => ("custom_node_mux_max", number, Common, smux.max_connections),
    MuxMinStreams => ("custom_node_mux_min_streams", number, Common, smux.min_streams),
    MuxMaxStreams => ("custom_node_mux_max_streams", number, Common, smux.max_streams),
    MuxPadding => ("custom_node_mux_padding", toggle, Common, smux.padding),
    Dialer => ("custom_node_dialer_proxy", text, Common, dialer_proxy),
    CaPath => ("custom_node_ca_path", text, Common, params.tls_trust.ca_path),
    CaFingerprint => ("custom_node_ca_fingerprint", text, Common, params.tls_trust.fingerprint),
    CaString => ("custom_node_ca_str", text, Common, params.tls_trust.ca_str),
    EchConfig => ("custom_node_ech_config", text, Ech, params.ech.config),
    TuicCc => ("custom_node_tuic_cc", text, Tuic, params.tuic.congestion_controller),
    TuicRelay => ("custom_node_tuic_udp_relay", text, Tuic, params.tuic.udp_relay_mode),
    TuicRtt => ("custom_node_tuic_reduce_rtt", toggle, Tuic, params.tuic.reduce_rtt),
    TuicHeartbeat => ("custom_node_tuic_heartbeat", number, Tuic, params.tuic.heartbeat_interval),
    TuicTimeout => ("custom_node_tuic_request_timeout", number, Tuic, params.tuic.request_timeout),
    TuicWindow => ("custom_node_recv_window", number, Tuic, params.tuic.recv_window),
    HyPorts => ("custom_node_hy2_ports", text, Hysteria2, params.hysteria2.ports),
    HyHop => ("custom_node_hy2_hop", number, Hysteria2, params.hysteria2.hop_interval),
    HyCwnd => ("custom_node_hy2_cwnd", number, Hysteria2, params.hysteria2.cwnd),
    HyMtu => ("custom_node_hy2_udp_mtu", number, Hysteria2, params.hysteria2.udp_mtu),
    HyObfs => ("custom_node_hy2_obfs", text, Hysteria2, params.hysteria2.obfs),
    HyObfsPassword => ("custom_node_hy2_obfs_password", text, Hysteria2, params.hysteria2.obfs_password),
    HyUp => ("custom_node_hy2_up", text, Hysteria2, params.hysteria2.up),
    HyDown => ("custom_node_hy2_down", text, Hysteria2, params.hysteria2.down),
    WgPrivate => ("custom_node_wg_private_key", text, WireGuard, params.wireguard.private_key),
    WgPublic => ("custom_node_wg_public_key", text, WireGuard, params.wireguard.public_key),
    WgPreShared => ("custom_node_wg_preshared_key", text, WireGuard, params.wireguard.pre_shared_key),
    WgReserved => ("custom_node_wg_reserved", text, WireGuard, params.wireguard.reserved),
    WgIp => ("custom_node_wg_ip", text, WireGuard, params.wireguard.ip),
    WgMtu => ("custom_node_wg_mtu", number, WireGuard, params.wireguard.mtu),
    WgKeepalive => ("custom_node_wg_keepalive", number, WireGuard, params.wireguard.persistent_keepalive),
    WgWorkers => ("custom_node_wg_workers", number, WireGuard, params.wireguard.workers),
    WgAllowed => ("custom_node_wg_allowed_ips", list, WireGuard, params.wireguard.allowed_ips),
    AwgJc => ("custom_node_awg_jc", optional, WireGuard, params.wireguard.amnezia.jc),
    AwgJmin => ("custom_node_awg_jmin", optional, WireGuard, params.wireguard.amnezia.jmin),
    AwgJmax => ("custom_node_awg_jmax", optional, WireGuard, params.wireguard.amnezia.jmax),
    Network => ("custom_node_transport_network", text, Transport, params.transport.network),
    WsPath => ("custom_node_ws_path", text, Transport, params.transport.ws.path),
    WsEarly => ("custom_node_ws_early_data", number, Transport, params.transport.ws.max_early_data),
    GrpcService => ("custom_node_grpc_service", text, Transport, params.transport.grpc.service_name),
    XhttpMode => ("custom_node_xhttp_mode", text, Transport, params.transport.xhttp.mode),
    XhttpPath => ("custom_node_xhttp_path", text, Transport, params.transport.xhttp.path),
    PluginName => ("custom_node_plugin_name", text, Shadowsocks, params.plugin.name),
    SshUser => ("custom_node_ssh_username", text, Ssh, params.ssh.username),
    SshPrivate => ("custom_node_ssh_private_key", text, Ssh, params.ssh.private_key),
    SshPassphrase => ("custom_node_ssh_passphrase", text, Ssh, params.ssh.passphrase),
    SshAlgorithms => ("custom_node_ssh_algorithms", list, Ssh, params.ssh.host_key_algorithms),
    AnyIdle => ("custom_node_anytls_idle", number, Anytls, params.anytls.idle_session_timeout),
    AnyCheck => ("custom_node_anytls_idle_check", number, Anytls, params.anytls.idle_session_check_interval),
    AnyMin => ("custom_node_anytls_min_idle", number, Anytls, params.anytls.min_idle_session),
    TrojanSs => ("custom_node_trojan_ss", toggle, Trojan, params.trojan_ss.enabled),
    TrojanMethod => ("custom_node_trojan_ss_method", text, Trojan, params.trojan_ss.method),
    TrojanPassword => ("custom_node_trojan_ss_password", text, Trojan, params.trojan_ss.password),
}

pub fn visible(id: ProtocolField, draft: &ProtocolDraft) -> bool {
    if id == ProtocolField::Secret {
        return draft.required_credentials().contains(&"uuid");
    }
    if id == ProtocolField::Password {
        return draft.required_credentials().contains(&"password");
    }
    match group(id) {
        FieldGroup::Common => true,
        FieldGroup::Ech => EchParams::supported_by(draft.family()),
        FieldGroup::Transport => TransportParams::supported_by(draft.family()),
        FieldGroup::Tuic => draft.family() == ProtocolFamily::Tuic,
        FieldGroup::Hysteria2 => draft.family() == ProtocolFamily::Hysteria2,
        FieldGroup::WireGuard => draft.family() == ProtocolFamily::WireGuard,
        FieldGroup::Shadowsocks => draft.family() == ProtocolFamily::Shadowsocks,
        FieldGroup::Ssh => draft.family() == ProtocolFamily::Ssh,
        FieldGroup::Anytls => draft.family() == ProtocolFamily::Anytls,
        FieldGroup::Trojan => draft.family() == ProtocolFamily::Trojan,
    }
}

pub fn special_projection(id: ProtocolField, draft: &ProtocolDraft) -> ProtocolFieldProjection {
    let (label_key, value) = match id {
        ProtocolField::WsHost => ("custom_node_ws_host", draft.params.transport.ws.host()),
        ProtocolField::PluginHost => (
            "custom_node_plugin_host",
            draft.params.plugin.opt("host").unwrap_or_default(),
        ),
        ProtocolField::PluginPassword => (
            "custom_node_plugin_password",
            draft.params.plugin.opt("password").unwrap_or_default(),
        ),
        ProtocolField::PluginMode => (
            "custom_node_plugin_mode",
            draft.params.plugin.opt("mode").unwrap_or_default(),
        ),
        _ => unreachable!("regular fields handled first"),
    };
    ProtocolFieldProjection {
        id,
        label_key,
        value,
        toggle: false,
    }
}

pub fn project_fields(draft: &ProtocolDraft) -> Vec<ProtocolFieldProjection> {
    ProtocolField::ALL
        .iter()
        .copied()
        .filter(|id| visible(*id, draft))
        .map(|id| field_projection(id, draft).unwrap_or_else(|| special_projection(id, draft)))
        .collect()
}

pub fn edit_field(
    draft: &ProtocolDraft,
    id: ProtocolField,
    raw: &str,
) -> Result<ProtocolDraft, Failure> {
    let mut next = draft.clone();
    if !apply_regular(&mut next, id, raw)? {
        match id {
            ProtocolField::WsHost => next.params.transport.ws.set_host(raw.to_owned()),
            ProtocolField::PluginHost
            | ProtocolField::PluginPassword
            | ProtocolField::PluginMode => {
                let key = match id {
                    ProtocolField::PluginHost => "host",
                    ProtocolField::PluginPassword => "password",
                    _ => "mode",
                };
                next.params
                    .plugin
                    .opts
                    .insert(key.to_owned(), PluginOptValue::Text(raw.to_owned()));
            }
            _ => unreachable!("regular fields handled first"),
        }
    }
    // The main credential field follows the same family rule as the Iced form.
    if id == ProtocolField::Secret && !next.required_credentials().contains(&"uuid") {
        next.password = raw.to_owned();
        next.uuid = draft.uuid.clone();
    }
    if id == ProtocolField::WgReserved {
        next.params.wireguard.reserved_is_base64 =
            raw.trim().chars().any(|c| matches!(c, '=' | '/' | '+'));
    }
    if id == ProtocolField::TuicWindow {
        next.params.tuic.recv_window_conn = next
            .params
            .tuic
            .recv_window_conn
            .min(next.params.tuic.recv_window);
    }
    Ok(next)
}

#[cfg(test)]
#[path = "protocol_form_test.rs"]
mod tests;

/// Staged text and validation are UI-neutral; no edit writes a profile.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProtocolInputs {
    pub values: BTreeMap<ProtocolField, String>,
    pub errors: BTreeMap<ProtocolField, String>,
}

impl ProtocolInputs {
    pub fn edit(
        &mut self,
        draft: &ProtocolDraft,
        id: ProtocolField,
        raw: String,
    ) -> Result<ProtocolDraft, Failure> {
        self.values.insert(id, raw.clone());
        match edit_field(draft, id, &raw) {
            Ok(next) => {
                self.errors.remove(&id);
                Ok(next)
            }
            Err(error) => {
                self.errors.insert(id, error.message.clone());
                Err(error)
            }
        }
    }
    pub fn value(&self, field: &ProtocolFieldProjection) -> String {
        self.values
            .get(&field.id)
            .cloned()
            .unwrap_or_else(|| field.value.clone())
    }
}
