//! High-fidelity profile converter and aggregator for Mihomo / Clash.Meta.
//!
//! Provides bidirectional lossless translation between Clash YAML,
//! Shadowrocket/V2Ray/QuantumultX URI formats, raw JSON, and Base64 subscriptions.

use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::{BTreeMap, HashSet};

use crate::filter::{
    ContentDedupStrategy, DeduplicationStrategy, FilterPipeline, FilterStage, NodeSortOrder,
    extract_country_code,
};
use crate::proxy_nodes::model::PortHopping;

#[path = "profile_converter/uri_export.rs"]
pub(crate) mod uri_export;
#[path = "profile_converter/uri_parse.rs"]
pub(crate) mod uri_parse;
#[path = "profile_converter/uri_parse_aux.rs"]
pub(crate) mod uri_parse_aux;

#[cfg(test)]
#[path = "profile_converter_test.rs"]
mod profile_converter_test;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyNodeItem {
    pub name: String,
    pub server: String,
    pub port: u16,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    #[serde(default)]
    pub tls: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servername: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_cert_verify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packet_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spider_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reality_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub down: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwnd: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recv_window_conn: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recv_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_relay_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_rtt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heartbeat_interval: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_sni: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preshared_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_dns_resolve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_keepalive: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amnezia_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_over_tcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uot_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_key_algorithms: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialer_proxy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tfo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mptcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h2_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_opts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xhttp_opts: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml_ng::Value>,
}

impl ProxyNodeItem {
    pub fn new(
        name: impl Into<String>,
        node_type: impl Into<String>,
        server: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            name: name.into(),
            server: server.into(),
            port,
            node_type: node_type.into(),
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
            public_key: None,
            short_id: None,
            spider_x: None,
            reality_opts: None,
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
            ws_opts: None,
            grpc_opts: None,
            h2_opts: None,
            http_opts: None,
            xhttp_opts: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn get_effective_public_key(&self) -> Option<&str> {
        if let Some(ref pk) = self.public_key {
            return Some(pk.as_str());
        }
        if let Some(ref ro) = self.reality_opts
            && let Some(pk) = ro.get("public-key").and_then(|v| v.as_str())
        {
            return Some(pk);
        }
        None
    }

    pub fn get_effective_short_id(&self) -> Option<&str> {
        if let Some(ref sid) = self.short_id {
            return Some(sid.as_str());
        }
        if let Some(ref ro) = self.reality_opts
            && let Some(sid) = ro.get("short-id").and_then(|v| v.as_str())
        {
            return Some(sid);
        }
        None
    }

    pub fn get_effective_sni(&self) -> Option<&str> {
        self.sni.as_deref().or(self.servername.as_deref())
    }

    pub fn get_effective_password(&self) -> Option<&str> {
        self.password.as_deref().or(self.auth.as_deref())
    }

    pub fn get_ports_spec(&self) -> Option<PortHopping> {
        self.ports.as_deref().and_then(|s| PortHopping::parse(s).ok())
    }

    pub fn get_grpc_service_name(&self) -> Option<&str> {
        self.grpc_opts
            .as_ref()
            .and_then(|v| v.get("grpc-service-name"))
            .and_then(|v| v.as_str())
    }

    pub fn get_ws_path(&self) -> Option<&str> {
        self.ws_opts
            .as_ref()
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
    }

    pub fn get_ws_host(&self) -> Option<&str> {
        self.ws_opts
            .as_ref()
            .and_then(|v| v.get("headers"))
            .and_then(|v| v.get("Host"))
            .and_then(|v| v.as_str())
    }

    pub fn get_xhttp_path(&self) -> Option<&str> {
        self.xhttp_opts
            .as_ref()
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
    }

    pub fn get_xhttp_mode(&self) -> Option<&str> {
        self.xhttp_opts
            .as_ref()
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileFormat {
    ClashYaml,
    ShadowrocketUriList,
    RawJson,
    Base64Subscription,
}

#[derive(Serialize, Deserialize, Default)]
struct ClashProfile {
    #[serde(default)]
    proxies: Vec<ProxyNodeItem>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml_ng::Value>,
}

pub struct ProfileConverter;

impl ProfileConverter {
    pub fn parse_uri(uri: &str) -> Result<ProxyNodeItem> {
        uri_parse::parse_uri(uri)
    }

    pub fn export_uri(node: &ProxyNodeItem) -> Result<String> {
        uri_export::export_uri(node)
    }

    pub fn parse_nodes(input: &str, format: ProfileFormat) -> Result<Vec<ProxyNodeItem>> {
        match format {
            ProfileFormat::ClashYaml => {
                let profile: ClashProfile = serde_yaml_ng::from_str(input)
                    .map_err(|e| anyhow!("Failed to parse YAML: {e}"))?;
                Ok(profile.proxies)
            }
            ProfileFormat::RawJson => {
                let nodes: Vec<ProxyNodeItem> = serde_json::from_str(input)
                    .map_err(|e| anyhow!("Failed to parse JSON: {e}"))?;
                Ok(nodes)
            }
            ProfileFormat::ShadowrocketUriList => {
                let mut nodes = Vec::new();
                for line in input.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                        continue;
                    }
                    if let Ok(node) = uri_parse::parse_uri(line) {
                        nodes.push(node);
                    }
                }
                Ok(nodes)
            }
            ProfileFormat::Base64Subscription => {
                let text = decode_base64_flexible(input)?;
                Self::parse_nodes(&text, ProfileFormat::ShadowrocketUriList)
            }
        }
    }

    pub fn export_nodes(nodes: &[ProxyNodeItem], target_format: ProfileFormat) -> Result<String> {
        match target_format {
            ProfileFormat::ClashYaml => {
                let profile = ClashProfile {
                    proxies: nodes.to_vec(),
                    extra: BTreeMap::new(),
                };
                serde_yaml_ng::to_string(&profile)
                    .map_err(|e| anyhow!("Failed to export YAML: {e}"))
            }
            ProfileFormat::RawJson => serde_json::to_string_pretty(nodes)
                .map_err(|e| anyhow!("Failed to export JSON: {e}")),
            ProfileFormat::ShadowrocketUriList => {
                let mut out = String::new();
                for node in nodes {
                    out.push_str(&uri_export::export_uri(node)?);
                    out.push('\n');
                }
                Ok(out)
            }
            ProfileFormat::Base64Subscription => {
                let uri_list = Self::export_nodes(nodes, ProfileFormat::ShadowrocketUriList)?;
                Ok(STANDARD.encode(uri_list))
            }
        }
    }

    pub fn detect_and_convert(input: &str) -> Result<String> {
        let trimmed = input.trim();
        if trimmed.starts_with("proxies:")
            || (trimmed.contains("proxies:") && trimmed.contains("port:"))
        {
            return Ok(trimmed.to_string());
        }
        if (trimmed.starts_with('{') || trimmed.starts_with('['))
            && let Ok(nodes) = Self::parse_nodes(trimmed, ProfileFormat::RawJson)
        {
            return Self::export_nodes(&nodes, ProfileFormat::ClashYaml);
        }
        if trimmed.starts_with("ss://")
            || trimmed.starts_with("vmess://")
            || trimmed.starts_with("trojan://")
            || trimmed.starts_with("vless://")
            || trimmed.starts_with("hysteria2://")
            || trimmed.starts_with("hy2://")
            || trimmed.starts_with("tuic://")
            || trimmed.starts_with("wireguard://")
            || trimmed.starts_with("wg://")
            || trimmed.starts_with("awg://")
            || trimmed.starts_with("amnezia-wg://")
            || trimmed.starts_with("anytls://")
            || trimmed.starts_with("ssh://")
        {
            let nodes = Self::parse_nodes(trimmed, ProfileFormat::ShadowrocketUriList)?;
            return Self::export_nodes(&nodes, ProfileFormat::ClashYaml);
        }
        if let Ok(nodes) = Self::parse_nodes(trimmed, ProfileFormat::Base64Subscription) {
            return Self::export_nodes(&nodes, ProfileFormat::ClashYaml);
        }
        Err(anyhow!("Unrecognized subscription or node format"))
    }

    pub fn aggregate_profiles(profile_yamls: &[&str]) -> Result<String> {
        let mut all_nodes = Vec::new();
        let mut seen_names = HashSet::new();

        for yaml in profile_yamls {
            if let Ok(nodes) = Self::parse_nodes(yaml, ProfileFormat::ClashYaml) {
                for mut node in nodes {
                    if seen_names.contains(&node.name) {
                        let mut idx = 2;
                        let original = node.name.clone();
                        while seen_names.contains(&format!("{original} ({idx})")) {
                            idx += 1;
                        }
                        node.name = format!("{original} ({idx})");
                    }
                    seen_names.insert(node.name.clone());
                    all_nodes.push(node);
                }
            }
        }

        if all_nodes.is_empty() {
            return Err(anyhow!(
                "No valid proxy nodes found across profiles to aggregate"
            ));
        }

        Self::export_nodes(&all_nodes, ProfileFormat::ClashYaml)
    }

    pub fn convert(input: &str, from_fmt: ProfileFormat, to_fmt: ProfileFormat) -> Result<String> {
        let nodes = Self::parse_nodes(input, from_fmt)?;
        Self::export_nodes(&nodes, to_fmt)
    }
}

pub(crate) fn decode_base64_flexible(input: &str) -> Result<String> {
    let sanitized: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = STANDARD
        .decode(&sanitized)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&sanitized))
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&sanitized))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&sanitized))
        .map_err(|e| anyhow!("Failed to decode Base64: {e}"))?;
    String::from_utf8(decoded).map_err(|e| anyhow!("Invalid UTF-8 in Base64: {e}"))
}

/// Source subscription input for multi-subscription aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSubscription {
    pub name: String,
    pub content: String,
    pub prefix: Option<String>,
}

impl SourceSubscription {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            prefix: None,
        }
    }

    pub fn with_prefix(
        name: impl Into<String>,
        content: impl Into<String>,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
            prefix: Some(prefix.into()),
        }
    }
}

/// Aggregation tuning options for [`MultiSubscriptionAggregator`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AggregationOptions {
    pub content_dedup: ContentDedupStrategy,
    pub name_dedup: DeduplicationStrategy,
    pub normalize_country_code: bool,
    pub remove_emojis: bool,
    pub sort_by: NodeSortOrder,
    pub generate_proxy_groups: bool,
}

/// Multi-subscription aggregator that merges, deduplicates, cleans, and structures nodes from multiple subscriptions.
pub struct MultiSubscriptionAggregator;

impl MultiSubscriptionAggregator {
    pub fn aggregate(
        sources: &[SourceSubscription],
        options: &AggregationOptions,
    ) -> Result<String> {
        let mut all_nodes = Vec::new();

        for source in sources {
            let converted = ProfileConverter::detect_and_convert(&source.content)?;
            let mut nodes = ProfileConverter::parse_nodes(&converted, ProfileFormat::ClashYaml)?;
            if let Some(ref pfx) = source.prefix {
                let tag = format!("[{pfx}]");
                for node in &mut nodes {
                    if !node.name.starts_with(&tag) {
                        node.name = format!("{tag} {}", node.name.trim());
                    }
                }
            }
            all_nodes.extend(nodes);
        }

        if all_nodes.is_empty() {
            return Err(anyhow!("No valid proxy nodes found across sources to aggregate"));
        }

        // Apply filter pipeline for cleaning / dedup / sorting
        let mut pipeline = FilterPipeline::new();
        if options.remove_emojis {
            pipeline.add_stage(FilterStage::remove_emojis());
        }
        if options.normalize_country_code {
            pipeline.add_stage(FilterStage::country_code_normalizer());
        }
        if options.content_dedup != ContentDedupStrategy::Disabled {
            pipeline.add_stage(FilterStage::content_deduplicator(options.content_dedup));
        }
        if options.name_dedup != DeduplicationStrategy::Disabled {
            pipeline.add_stage(FilterStage::duplicate_deduplicator(options.name_dedup));
        }
        if options.sort_by != NodeSortOrder::Preserve {
            pipeline.add_stage(FilterStage::sort_nodes(options.sort_by));
        }

        pipeline.apply_pipeline(&mut all_nodes);

        if options.generate_proxy_groups {
            Self::generate_profile_with_groups(&all_nodes)
        } else {
            ProfileConverter::export_nodes(&all_nodes, ProfileFormat::ClashYaml)
        }
    }

    fn generate_profile_with_groups(nodes: &[ProxyNodeItem]) -> Result<String> {
        let node_names: Vec<String> = nodes.iter().map(|n| n.name.clone()).collect();
        let mut country_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for node in nodes {
            if let Some(iso) = extract_country_code(&node.name) {
                country_groups
                    .entry(iso.to_string())
                    .or_default()
                    .push(node.name.clone());
            }
        }

        let mut groups = Vec::new();

        // 1. Main select group
        let mut main_proxies = vec!["♻️ 自动选择".to_string(), "🎯 全球直连".to_string()];
        for iso in country_groups.keys() {
            main_proxies.push(format!("{iso} 节点"));
        }
        main_proxies.extend(node_names.clone());

        groups.push(serde_json::json!({
            "name": "🚀 节点选择",
            "type": "select",
            "proxies": main_proxies,
        }));

        // 2. Auto-test group
        groups.push(serde_json::json!({
            "name": "♻️ 自动选择",
            "type": "url-test",
            "url": "http://www.gstatic.com/generate_204",
            "interval": 300,
            "tolerance": 50,
            "proxies": node_names,
        }));

        // 3. Country groups
        for (iso, proxies) in country_groups {
            groups.push(serde_json::json!({
                "name": format!("{iso} 节点"),
                "type": "url-test",
                "url": "http://www.gstatic.com/generate_204",
                "interval": 300,
                "tolerance": 50,
                "proxies": proxies,
            }));
        }

        // 4. Direct & Reject
        groups.push(serde_json::json!({
            "name": "🎯 全球直连",
            "type": "select",
            "proxies": ["DIRECT"],
        }));
        groups.push(serde_json::json!({
            "name": "🛑 广告拦截",
            "type": "select",
            "proxies": ["REJECT", "DIRECT"],
        }));

        let mut doc = serde_yaml_ng::Mapping::new();
        doc.insert(Value::String("port".into()), Value::Number(7890.into()));
        doc.insert(Value::String("socks-port".into()), Value::Number(7891.into()));
        doc.insert(Value::String("mode".into()), Value::String("rule".into()));
        doc.insert(Value::String("log-level".into()), Value::String("info".into()));

        let proxies_yaml: Value = serde_yaml_ng::to_value(nodes)?;
        doc.insert(Value::String("proxies".into()), proxies_yaml);

        let groups_yaml: Value = serde_yaml_ng::to_value(groups)?;
        doc.insert(Value::String("proxy-groups".into()), groups_yaml);

        let rules = vec![Value::String("MATCH,🚀 节点选择".into())];
        doc.insert(Value::String("rules".into()), Value::Sequence(rules));

        serde_yaml_ng::to_string(&Value::Mapping(doc)).map_err(|e| anyhow!("{e}"))
    }
}
