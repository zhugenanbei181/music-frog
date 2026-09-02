//! Node filtering and transformation pipeline for subscriptions and proxy node lists.
//!
//! Provides both node-level [`FilterPipeline`] with composable stages and YAML-level
//! [`SubscriptionFilterPipeline`] for reshaping subscription documents.

use crate::profile_converter::ProxyNodeItem;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;

#[path = "filter_pipeline.rs"]
mod filter_pipeline;
#[path = "filter_subscription.rs"]
mod filter_subscription;
/// Strategy used when encountering duplicate proxy node names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeduplicationStrategy {
    #[default]
    Disabled,
    KeepFirst,
    KeepLast,
    AppendIndex,
}

/// Strategy for content fingerprint-based deduplication (identical server/port/credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentDedupStrategy {
    #[default]
    Disabled,
    KeepFirst,
    KeepLast,
    KeepLowerMultiplier,
}

/// Sort order for proxy nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeSortOrder {
    #[default]
    Preserve,
    NameAsc,
    NameDesc,
    CountryCode,
    Protocol,
    MultiplierAsc,
    MultiplierDesc,
}

/// A regex rename rule for proxy node names.
#[derive(Debug, Clone)]
pub struct RenameRule {
    pub pattern: Regex,
    pub replacement: String,
}

impl RenameRule {
    pub fn new(pattern: &str, replacement: impl Into<String>) -> Result<Self> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            replacement: replacement.into(),
        })
    }
}

/// Traffic multiplier override rule.
#[derive(Debug, Clone)]
pub struct MultiplierRule {
    pub pattern: Regex,
    pub multiplier: f64,
}

impl MultiplierRule {
    pub fn new(pattern: &str, multiplier: f64) -> Result<Self> {
        Ok(Self {
            pattern: Regex::new(pattern)?,
            multiplier,
        })
    }
}

/// Configuration for mutating node properties (forcing TLS, UDP, fingerprint, ALPN, etc.).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct NodeMutatorConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_tls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_udp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_cert_verify: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tfo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mptcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smux: Option<bool>,
}

/// Configuration for port-based filtering.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct PortFilterConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ports: Option<HashSet<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_ports: Option<HashSet<u16>>,
}

/// Configuration for server/host address filtering.
#[derive(Debug, Clone, Default)]
pub struct ServerFilterConfig {
    pub allowed_patterns: Vec<Regex>,
    pub blocked_patterns: Vec<Regex>,
    pub drop_private_ip: bool,
}

/// Filter configuration rule for [`SubscriptionFilterPipeline`].
#[derive(Debug, Clone, Default)]
pub struct FilterRule {
    pub include_keywords: Vec<Regex>,
    pub exclude_keywords: Vec<Regex>,
    pub rename_rules: Vec<RenameRule>,
    pub exclude_types: Vec<String>,
    pub deduplication: DeduplicationStrategy,
    pub normalize_country_code: bool,
    pub remove_emojis: bool,
    pub allowed_ports: Option<HashSet<u16>>,
    pub blocked_ports: Option<HashSet<u16>>,
    pub drop_private_ip: bool,
    pub multiplier_rules: Vec<MultiplierRule>,
    pub node_mutator: Option<NodeMutatorConfig>,
    pub sort_order: NodeSortOrder,
    pub content_dedup: ContentDedupStrategy,
}

/// Statistics reported after YAML or name filtering via [`SubscriptionFilterPipeline`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterReport {
    pub total_input: usize,
    pub passed: usize,
    pub excluded_by_blacklist: usize,
    pub excluded_by_whitelist: usize,
    pub excluded_by_type: usize,
    #[serde(default)]
    pub excluded_by_port: usize,
    #[serde(default)]
    pub excluded_by_server: usize,
    pub renamed: usize,
    pub deduplicated: usize,
    #[serde(default)]
    pub mutated: usize,
}

/// Filter pipeline that operates directly on YAML strings or name slices.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilterPipeline {
    pub rule: FilterRule,
}

/// Global country and region definition mapping (40+ countries and regions).
pub const COUNTRY_DEFS: &[(&str, &[&str])] = &[
    ("HK", &["🇭🇰", "香港", "Hong Kong", "HongKong", "HKG", "HK"]),
    (
        "TW",
        &["🇹🇼", "台湾", "Taiwan", "台北", "Taipei", "TWN", "TW"],
    ),
    (
        "JP",
        &[
            "🇯🇵", "日本", "Japan", "东京", "Tokyo", "大阪", "Osaka", "JPN", "JP",
        ],
    ),
    ("SG", &["🇸🇬", "新加坡", "Singapore", "狮城", "SGP", "SG"]),
    (
        "US",
        &[
            "🇺🇸",
            "美国",
            "United States",
            "USA",
            "洛杉矶",
            "Los Angeles",
            "硅谷",
            "Silicon Valley",
            "圣何塞",
            "San Jose",
            "纽约",
            "New York",
            "US",
        ],
    ),
    (
        "KR",
        &[
            "🇰🇷",
            "韩国",
            "Korea",
            "South Korea",
            "首尔",
            "Seoul",
            "KOR",
            "KR",
        ],
    ),
    (
        "GB",
        &[
            "🇬🇧",
            "英国",
            "United Kingdom",
            "Great Britain",
            "UK",
            "London",
            "伦敦",
            "GBR",
            "GB",
        ],
    ),
    (
        "DE",
        &[
            "🇩🇪",
            "德国",
            "Germany",
            "Deutschland",
            "Frankfurt",
            "法兰克福",
            "DEU",
            "DE",
        ],
    ),
    (
        "FR",
        &["🇫🇷", "法国", "France", "Paris", "巴黎", "FRA", "FR"],
    ),
    (
        "CA",
        &[
            "🇨🇦",
            "加拿大",
            "Canada",
            "Toronto",
            "Vancouver",
            "CAN",
            "CA",
        ],
    ),
    (
        "AU",
        &[
            "🇦🇺",
            "澳大利亚",
            "澳洲",
            "Australia",
            "Sydney",
            "Melbourne",
            "AUS",
            "AU",
        ],
    ),
    (
        "NL",
        &["🇳🇱", "荷兰", "Netherlands", "Amsterdam", "NLD", "NL"],
    ),
    (
        "RU",
        &["🇷🇺", "俄罗斯", "Russia", "Moscow", "莫斯科", "RUS", "RU"],
    ),
    ("IN", &["🇮🇳", "印度", "India", "Mumbai", "IND", "IN"]),
    ("MO", &["🇲🇴", "澳门", "Macau", "Macao", "MAC", "MO"]),
    (
        "TH",
        &["🇹🇭", "泰国", "Thailand", "曼谷", "Bangkok", "THA", "TH"],
    ),
    (
        "VN",
        &[
            "🇻🇳",
            "越南",
            "Vietnam",
            "胡志明",
            "河内",
            "Hanoi",
            "VNM",
            "VN",
        ],
    ),
    (
        "MY",
        &[
            "🇲🇾",
            "马来西亚",
            "Malaysia",
            "吉隆坡",
            "Kuala Lumpur",
            "MYS",
            "MY",
        ],
    ),
    (
        "PH",
        &[
            "🇵🇭",
            "菲律宾",
            "Philippines",
            "马尼拉",
            "Manila",
            "PHL",
            "PH",
        ],
    ),
    (
        "ID",
        &[
            "🇮🇩",
            "印度尼西亚",
            "印尼",
            "Indonesia",
            "雅加达",
            "Jakarta",
            "IDN",
            "ID",
        ],
    ),
    (
        "TR",
        &[
            "🇹🇷",
            "土耳其",
            "Turkey",
            "伊斯坦布尔",
            "Istanbul",
            "TUR",
            "TR",
        ],
    ),
    (
        "BR",
        &["🇧🇷", "巴西", "Brazil", "圣保罗", "Sao Paulo", "BRA", "BR"],
    ),
    (
        "AR",
        &["🇦🇷", "阿根廷", "Argentina", "布宜诺斯艾利斯", "ARG", "AR"],
    ),
    (
        "ZA",
        &["🇿🇦", "南非", "South Africa", "约翰内斯堡", "ZAF", "ZA"],
    ),
    (
        "IL",
        &[
            "🇮🇱",
            "以色列",
            "Israel",
            "特拉维夫",
            "Tel Aviv",
            "ISR",
            "IL",
        ],
    ),
    (
        "AE",
        &[
            "🇦🇪",
            "阿联酋",
            "United Arab Emirates",
            "UAE",
            "迪拜",
            "Dubai",
            "ARE",
            "AE",
        ],
    ),
    (
        "UA",
        &["🇺🇦", "乌克兰", "Ukraine", "基辅", "Kyiv", "UKR", "UA"],
    ),
    (
        "SE",
        &[
            "🇸🇪",
            "瑞典",
            "Sweden",
            "斯德哥尔摩",
            "Stockholm",
            "SWE",
            "SE",
        ],
    ),
    (
        "CH",
        &["🇨🇭", "瑞士", "Switzerland", "苏黎世", "Zurich", "CHE", "CH"],
    ),
    (
        "IT",
        &[
            "🇮🇹",
            "意大利",
            "Italy",
            "罗马",
            "米兰",
            "Milan",
            "Rome",
            "ITA",
            "IT",
        ],
    ),
    (
        "ES",
        &[
            "🇪🇸",
            "西班牙",
            "Spain",
            "马德里",
            "Madrid",
            "巴塞罗那",
            "ESP",
            "ES",
        ],
    ),
    (
        "IE",
        &["🇮🇪", "爱尔兰", "Ireland", "都柏林", "Dublin", "IRL", "IE"],
    ),
    (
        "PL",
        &["🇵🇱", "波兰", "Poland", "华沙", "Warsaw", "POL", "PL"],
    ),
    (
        "NO",
        &["🇳🇴", "挪威", "Norway", "奥斯陆", "Oslo", "NOR", "NO"],
    ),
    (
        "FI",
        &["🇫🇮", "芬兰", "Finland", "赫尔辛基", "Helsinki", "FIN", "FI"],
    ),
    (
        "AT",
        &["🇦🇹", "奥地利", "Austria", "维也纳", "Vienna", "AUT", "AT"],
    ),
    (
        "BE",
        &[
            "🇧🇪",
            "比利时",
            "Belgium",
            "布鲁塞尔",
            "Brussels",
            "BEL",
            "BE",
        ],
    ),
    (
        "NZ",
        &[
            "🇳🇿",
            "新西兰",
            "New Zealand",
            "奥克兰",
            "Auckland",
            "NZL",
            "NZ",
        ],
    ),
    ("MX", &["🇲🇽", "墨西哥", "Mexico", "MEX", "MX"]),
    ("CL", &["🇨🇱", "智利", "Chile", "圣地亚哥", "CHL", "CL"]),
    (
        "KZ",
        &["🇰🇿", "哈萨克斯坦", "Kazakhstan", "阿拉木图", "KAZ", "KZ"],
    ),
];

fn matches_alias(text: &str, upper: &str, alias: &str) -> bool {
    if !alias.is_ascii() {
        return text.contains(alias);
    }
    let alias_upper = alias.to_ascii_uppercase();
    if alias.len() <= 3 {
        let mut idx = 0;
        while let Some(pos) = upper[idx..].find(&alias_upper) {
            let actual_pos = idx + pos;
            let before_ok =
                actual_pos == 0 || !text.as_bytes()[actual_pos - 1].is_ascii_alphabetic();
            let end = actual_pos + alias_upper.len();
            let after_ok = end >= text.len() || !text.as_bytes()[end].is_ascii_alphabetic();
            if before_ok && after_ok {
                return true;
            }
            idx = actual_pos + 1;
            if idx >= upper.len() {
                break;
            }
        }
        false
    } else {
        upper.contains(&alias_upper)
    }
}

/// Normalizes country flags, emojis, and aliases in a node name to standard `[ISO]` prefix.
pub fn normalize_country_code(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let already_tagged = trimmed.starts_with('[')
        && trimmed.len() >= 4
        && trimmed[1..3].chars().all(|c| c.is_ascii_uppercase())
        && (trimmed.as_bytes()[3] == b']' || trimmed.as_bytes()[3] == b'-');
    if already_tagged {
        return trimmed.to_string();
    }

    let upper_name = trimmed.to_ascii_uppercase();
    for (iso, aliases) in COUNTRY_DEFS {
        for alias in *aliases {
            if matches_alias(trimmed, &upper_name, alias) {
                return format!("[{iso}] {trimmed}");
            }
        }
    }
    trimmed.to_string()
}

/// Extracts standard ISO 2-letter country code from a node name if recognized.
pub fn extract_country_code(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('[') && trimmed.len() >= 4 && trimmed.as_bytes()[3] == b']' {
        let tag = &trimmed[1..3];
        for (iso, _) in COUNTRY_DEFS {
            if tag.eq_ignore_ascii_case(iso) {
                return Some(iso);
            }
        }
    }
    let upper_name = trimmed.to_ascii_uppercase();
    for (iso, aliases) in COUNTRY_DEFS {
        for alias in *aliases {
            if matches_alias(trimmed, &upper_name, alias) {
                return Some(iso);
            }
        }
    }
    None
}

/// Returns true if character is an emoji or regional indicator flag symbol.
pub fn is_emoji_char(c: char) -> bool {
    let u = c as u32;
    matches!(
        u,
        0x1F1E6..=0x1F1FF   // Flags (Regional indicator symbols)
        | 0x1F300..=0x1F5FF // Misc Symbols and Pictographs
        | 0x1F600..=0x1F64F // Emoticons
        | 0x1F680..=0x1F6FF // Transport and Map
        | 0x1F900..=0x1F9FF // Supplemental Symbols and Pictographs
        | 0x1FA70..=0x1FAFF // Symbols and Pictographs Extended-A
        | 0x2600..=0x26FF   // Misc Symbols
        | 0x2700..=0x27BF   // Dingbats
        | 0xFE00..=0xFE0F   // Variation Selectors
        | 0x200D            // Zero Width Joiner
    )
}

/// Strips emoji symbols and flag icons from text.
pub fn strip_emojis(text: &str) -> String {
    let s: String = text.chars().filter(|&c| !is_emoji_char(c)).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Checks whether an IP or hostname points to a private/loopback/unroutable address.
pub fn is_private_ip(server: &str) -> bool {
    let trimmed = server.trim().trim_matches('[').trim_matches(']');
    if trimmed.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                octets[0] == 127
                    || octets[0] == 10
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168)
                    || (octets[0] == 169 && octets[1] == 254)
                    || octets[0] == 0
                    || (octets[0] == 100 && (64..=127).contains(&octets[1]))
            }
            IpAddr::V6(ipv6) => {
                ipv6.is_loopback()
                    || ipv6.is_unspecified()
                    || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                    || (ipv6.segments()[0] & 0xffc0) == 0xfe80
            }
        }
    } else {
        false
    }
}

/// Extracts traffic multiplier factor from a node name (e.g. `[1.5x]` -> `1.5`).
pub fn extract_multiplier(name: &str) -> Option<f64> {
    let mult_re =
        Regex::new(r"(?i)(?:[\[（【])?(?:(\d+(?:\.\d+)?)[xX]|[xX](\d+(?:\.\d+)?))(?:[\]）】])?")
            .ok()?;
    if let Some(caps) = mult_re.captures(name) {
        if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
            return m.as_str().parse::<f64>().ok();
        }
    }
    None
}

/// Computes a unique content fingerprint hash for a proxy node.
pub fn compute_node_fingerprint(node: &ProxyNodeItem) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    node.node_type.to_ascii_lowercase().hash(&mut hasher);
    node.server.to_ascii_lowercase().hash(&mut hasher);
    node.port.hash(&mut hasher);
    if let Some(ref p) = node.password {
        p.hash(&mut hasher);
    }
    if let Some(ref u) = node.uuid {
        u.hash(&mut hasher);
    }
    if let Some(ref c) = node.cipher {
        c.hash(&mut hasher);
    }
    if let Some(ref pk) = node.public_key {
        pk.hash(&mut hasher);
    }
    if let Some(ref sni) = node.sni {
        sni.to_ascii_lowercase().hash(&mut hasher);
    }
    if let Some(ref ws) = node.ws_opts {
        ws.to_string().hash(&mut hasher);
    }
    if let Some(ref grpc) = node.grpc_opts {
        grpc.to_string().hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Individual stage in a [`FilterPipeline`].
#[derive(Debug, Clone)]
pub enum FilterStage {
    /// Renames matching node names using regular expression replacement.
    RegexRename { pattern: Regex, replacement: String },
    /// Overrides or appends a traffic multiplier tag (e.g. `[1.5x]`) for matching node names.
    MultiplierOverride { pattern: Regex, multiplier: f64 },
    /// Drops nodes whose protocol type (`node_type`) is not in the allowed set.
    ProtocolFilter { allowed_types: HashSet<String> },
    /// Normalizes country flags, emojis, and aliases in node names to standard `[ISO]` codes.
    CountryCodeNormalizer,
    /// Strips emojis and flag symbols from node names.
    RemoveEmojis,
    /// Drops nodes whose names match any of the blacklist patterns.
    KeywordBlacklist { patterns: Vec<Regex> },
    /// Drops nodes whose names do not match any of the whitelist patterns (when non-empty).
    KeywordWhitelist { patterns: Vec<Regex> },
    /// Filters nodes by allowed or blocked TCP/UDP destination ports.
    PortFilter { config: PortFilterConfig },
    /// Filters nodes by server hostname or drops private IP addresses.
    ServerFilter { config: ServerFilterConfig },
    /// Mutates node settings (force TLS, UDP, fingerprint, TFO, ALPN).
    NodeMutator { config: NodeMutatorConfig },
    /// Sorts nodes according to the specified ordering criterion.
    SortNodes { order: NodeSortOrder },
    /// Deduplicates nodes with identical names according to the selected strategy.
    DuplicateDeduplicator { strategy: DeduplicationStrategy },
    /// Deduplicates identical backend proxy nodes based on content fingerprints.
    ContentDeduplicator { strategy: ContentDedupStrategy },
}

/// Statistics collected during execution of a [`FilterPipeline`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStats {
    pub nodes_in: usize,
    pub nodes_out: usize,
    pub renamed_count: usize,
    pub dropped_count: usize,
    #[serde(default)]
    pub mutated_count: usize,
    #[serde(default)]
    pub deduplicated_count: usize,
}

/// Ordered sequence of filter and transformation stages applied to [`ProxyNodeItem`] collections.
#[derive(Debug, Clone, Default)]
pub struct FilterPipeline {
    stages: Vec<FilterStage>,
}

#[cfg(test)]
#[path = "filter_test.rs"]
mod tests;
