//! Pure mapping between the neutral typed `ProtocolDraft` and the domain's
//! flat `ProxyNodeItem`. Engine-specific YAML key ownership and field merge
//! rules live in `mihomo-config`.

use crate::profile_converter::ProxyNodeItem;
use crate::protocol_node_params::params_from_node;
use infiltrator_contract::protocol_fidelity::{
    ProtocolDraft, ProtocolFamily, RealityParams, SmuxParams,
};

/// DUAL-05: project a parsed profile node into the shared draft.
/// `spider-x` lives on the flat field when a URI parse set it and inside
/// `reality-opts` when a profile node carried it; read both shapes.
fn effective_spider_x(item: &ProxyNodeItem) -> String {
    if let Some(spider_x) = item.spider_x.as_deref()
        && !spider_x.trim().is_empty()
    {
        return spider_x.to_string();
    }
    item.reality_opts
        .as_ref()
        .and_then(|value| value.get("spider-x"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn draft_from_node(item: &ProxyNodeItem) -> ProtocolDraft {
    let family = ProtocolFamily::from_type_str(&item.node_type);
    let mut draft = ProtocolDraft::new(item.node_type.clone());
    draft.name = item.name.clone();
    draft.server = item.server.clone();
    draft.port = item.port;
    draft.password = item
        .password
        .clone()
        .or_else(|| item.auth.clone())
        .unwrap_or_default();
    draft.uuid = item.uuid.clone().unwrap_or_default();
    draft.sni = item
        .get_effective_sni()
        .map(str::to_string)
        .unwrap_or_default();
    draft.cipher = item.cipher.clone().unwrap_or_default();
    draft.flow = item.flow.clone().unwrap_or_default();
    // `public_key` is shared by REALITY and WireGuard in the flat DTO; only the
    // families whose schema carries a REALITY block may read it as such.
    draft.reality = if family.supports_reality() {
        RealityParams {
            public_key: item
                .get_effective_public_key()
                .map(str::to_string)
                .unwrap_or_default(),
            short_id: item
                .get_effective_short_id()
                .map(str::to_string)
                .unwrap_or_default(),
            spider_x: effective_spider_x(item),
            fingerprint: item.client_fingerprint.clone().unwrap_or_default(),
        }
    } else {
        RealityParams::default()
    };
    draft.smux = smux_from_json(item.smux.as_ref());
    draft.tls = item.tls;
    draft.skip_cert_verify = item.skip_cert_verify.unwrap_or(false);
    draft.alpn = item.alpn.clone().unwrap_or_default();
    // DUAL-05-09: the static hop the node dials through.
    draft.dialer_proxy = item.dialer_proxy.clone().unwrap_or_default();
    draft.params = params_from_node(item, family);
    // The selected engine codec fills this from its schema-owned key table.
    // A neutral domain mapping must not guess which flattened keys are typed.
    draft.preserved_fields = Vec::new();
    draft
}

fn smux_from_json(value: Option<&serde_json::Value>) -> SmuxParams {
    use infiltrator_contract::protocol_fidelity::SmuxParams;
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return SmuxParams::default();
    };
    let text = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let number = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as u32)
    };
    let flag = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    let mut params = SmuxParams::default();
    if let Some(protocol) = text("protocol") {
        params.protocol = protocol;
    }
    params.enabled = flag("enabled");
    if let Some(value) = number("max-connections") {
        params.max_connections = value;
    }
    if let Some(value) = number("min-streams") {
        params.min_streams = value;
    }
    if let Some(value) = number("max-streams") {
        params.max_streams = value;
    }
    params.padding = flag("padding");
    params.statistic = flag("statistic");
    params.only_tcp = flag("only-tcp");
    params
}

pub(crate) fn smux_to_json(params: &SmuxParams) -> serde_json::Value {
    serde_json::json!({
        "enabled": params.enabled,
        "protocol": params.protocol.trim(),
        "max-connections": params.max_connections,
        "min-streams": params.min_streams,
        "max-streams": params.max_streams,
        "padding": params.padding,
        "statistic": params.statistic,
        "only-tcp": params.only_tcp,
    })
}
