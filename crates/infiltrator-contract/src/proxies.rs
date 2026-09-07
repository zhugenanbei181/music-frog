//! Shared contract types for proxy strategy groups, leaf node selection,
//! alive-only filtering, and four-dimensional sorting.
//!
//! Track 2 / Group 04: Proxies & Sorting specifications.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Five canonical proxy group classifications supported across all Mihomo kernels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyGroupClassification {
    /// Manual selection group (用户手动挑选目标节点).
    Selector,
    /// Automated latency benchmark group (定期测速并自动选最低延迟节点).
    #[serde(alias = "url_test", alias = "url-test", alias = "URLTest")]
    UrlTest,
    /// Fallback failover group (依序健康检查并故障自动转移).
    #[serde(alias = "fallback", alias = "Fallback")]
    Fallback,
    /// Load balance group (负载均衡，散列或轮询流量).
    #[serde(alias = "load_balance", alias = "load-balance", alias = "LoadBalance")]
    LoadBalance,
    /// Chained relay group (多跳链式中继代理).
    #[serde(alias = "relay", alias = "Relay")]
    Relay,
}

impl ProxyGroupClassification {
    /// Parse from wire string leniently, supporting Mihomo PascalCase,
    /// kebab-case, and snake_case variants.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match normalized.as_str() {
            "selector" => Some(Self::Selector),
            "urltest" | "auto" => Some(Self::UrlTest),
            "fallback" => Some(Self::Fallback),
            "loadbalance" | "lb" => Some(Self::LoadBalance),
            "relay" => Some(Self::Relay),
            _ => None,
        }
    }

    /// Canonical kebab/wire string used by Mihomo configs and REST responses.
    pub fn mihomo_wire_type(&self) -> &'static str {
        match self {
            Self::Selector => "Selector",
            Self::UrlTest => "URLTest",
            Self::Fallback => "Fallback",
            Self::LoadBalance => "LoadBalance",
            Self::Relay => "Relay",
        }
    }

    /// Machine-readable snake_case identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Selector => "selector",
            Self::UrlTest => "url_test",
            Self::Fallback => "fallback",
            Self::LoadBalance => "load_balance",
            Self::Relay => "relay",
        }
    }

    /// Localized Chinese label for GUI displays.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Selector => "手动选择",
            Self::UrlTest => "自动测速",
            Self::Fallback => "故障转移",
            Self::LoadBalance => "负载均衡",
            Self::Relay => "链式中继",
        }
    }

    /// Localized English label for GUI displays.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::Selector => "Selector",
            Self::UrlTest => "URL-Test",
            Self::Fallback => "Fallback",
            Self::LoadBalance => "Load-Balance",
            Self::Relay => "Relay",
        }
    }

    /// Whether this strategy group supports user interactive node switching (`PUT /proxies/{group}`).
    /// In Mihomo, only Selector groups accept manual node switching; automated groups manage
    /// selected node internal to the core or relay in sequential order.
    pub fn is_manual_selectable(&self) -> bool {
        matches!(self, Self::Selector)
    }

    /// Whether this strategy group runs automated node scheduling.
    pub fn is_automated(&self) -> bool {
        !self.is_manual_selectable()
    }

    /// Whether this group requires background periodic healthcheck benchmarking.
    pub fn requires_healthcheck(&self) -> bool {
        matches!(self, Self::UrlTest | Self::Fallback)
    }
}

impl std::fmt::Display for ProxyGroupClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Four-dimensional sort order for proxy node cards within a strategy group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxySortOrder {
    /// Latency ascending: lowest delay first, dead/untested nodes at bottom.
    #[default]
    #[serde(alias = "delay_asc")]
    LatencyAsc,
    /// Latency descending: highest delay first, dead/untested nodes at bottom.
    #[serde(alias = "delay_desc")]
    LatencyDesc,
    /// Node name ascending: alphabetical A -> Z.
    #[serde(alias = "name_asc")]
    NameAsc,
    /// Node name descending: alphabetical Z -> A.
    #[serde(alias = "name_desc")]
    NameDesc,
}

impl ProxySortOrder {
    /// Parse from wire string leniently.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match normalized.as_str() {
            "delay_asc" | "latency_asc" | "default" => Some(Self::LatencyAsc),
            "delay_desc" | "latency_desc" => Some(Self::LatencyDesc),
            "name_asc" | "alpha_asc" => Some(Self::NameAsc),
            "name_desc" | "alpha_desc" => Some(Self::NameDesc),
            _ => None,
        }
    }

    /// Stable wire key compatible with Iced and Bevy settings.
    pub fn as_wire_key(&self) -> &'static str {
        match self {
            Self::LatencyAsc => "delay_asc",
            Self::LatencyDesc => "delay_desc",
            Self::NameAsc => "name_asc",
            Self::NameDesc => "name_desc",
        }
    }

    /// Localized Chinese label.
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::LatencyAsc => "延迟升序",
            Self::LatencyDesc => "延迟降序",
            Self::NameAsc => "名称升序",
            Self::NameDesc => "名称降序",
        }
    }

    /// Localized English label.
    pub fn label_en(&self) -> &'static str {
        match self {
            Self::LatencyAsc => "Latency Asc",
            Self::LatencyDesc => "Latency Desc",
            Self::NameAsc => "Name Asc",
            Self::NameDesc => "Name Desc",
        }
    }

    /// All four sort order variants in canonical display sequence.
    pub fn all_variants() -> &'static [Self] {
        &[
            Self::LatencyAsc,
            Self::LatencyDesc,
            Self::NameAsc,
            Self::NameDesc,
        ]
    }

    /// Deterministic candidate comparison implementing favorite pinning and four-way sorting.
    ///
    /// Rules:
    /// 1. Favorite pinned nodes ALWAYS come before non-favorites.
    /// 2. If favorites match, sort by the active order.
    /// 3. In latency sort, valid delays (>0) precede dead or untested nodes (`None` or `Some(0)`).
    /// 4. Ties are broken by name ascending.
    pub fn compare_candidates(
        &self,
        left_name: &str,
        left_delay: Option<u32>,
        left_fav: bool,
        right_name: &str,
        right_delay: Option<u32>,
        right_fav: bool,
    ) -> Ordering {
        // Rule 1: Favorite pinning
        if left_fav != right_fav {
            return if left_fav {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        // Filter out zero-delays as dead/timeout
        let l_delay = left_delay.filter(|&d| d > 0);
        let r_delay = right_delay.filter(|&d| d > 0);

        match self {
            Self::LatencyAsc => match (l_delay, r_delay) {
                (Some(ld), Some(rd)) => ld.cmp(&rd).then_with(|| left_name.cmp(right_name)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => left_name.cmp(right_name),
            },
            Self::LatencyDesc => match (l_delay, r_delay) {
                (Some(ld), Some(rd)) => rd.cmp(&ld).then_with(|| left_name.cmp(right_name)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => left_name.cmp(right_name),
            },
            Self::NameAsc => left_name.cmp(right_name),
            Self::NameDesc => right_name.cmp(left_name),
        }
    }
}

/// Snapshot of the "Filter Alive" (只看可用) state, tracking alive vs dead node counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyFilterAliveSnapshot {
    pub enabled: bool,
    pub total_nodes: usize,
    pub alive_nodes: usize,
    pub dead_nodes: usize,
}

impl ProxyFilterAliveSnapshot {
    pub fn new(enabled: bool, total_nodes: usize, alive_nodes: usize, dead_nodes: usize) -> Self {
        Self {
            enabled,
            total_nodes,
            alive_nodes,
            dead_nodes,
        }
    }

    /// Evaluates whether an individual node is considered alive and operational.
    /// A node is alive if it is marked alive and has a non-zero test delay.
    pub fn is_node_alive(delay_ms: Option<u32>, alive_flag: bool) -> bool {
        alive_flag && delay_ms.is_some_and(|ms| ms > 0)
    }

    /// Derive an alive snapshot from an iterator of (delay_ms, alive_flag) tuples.
    pub fn derive_from_candidates<I>(candidates: I, enabled: bool) -> Self
    where
        I: IntoIterator<Item = (Option<u32>, bool)>,
    {
        let mut total: usize = 0;
        let mut alive: usize = 0;
        for (delay, flag) in candidates {
            total += 1;
            if Self::is_node_alive(delay, flag) {
                alive += 1;
            }
        }
        let dead = total.saturating_sub(alive);
        Self {
            enabled,
            total_nodes: total,
            alive_nodes: alive,
            dead_nodes: dead,
        }
    }
}

/// Latency tier for multi-color scale rendering (5 color tiers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyTier {
    /// < 100ms: Emerald green / 翡翠绿.
    Fast,
    /// 100 - 200ms: Grass green / 青草绿.
    Normal,
    /// 200 - 300ms: Warm amber / 暖黄.
    Medium,
    /// > 300ms: Alert red / 警戒红.
    Slow,
    /// Timeout (0ms) or untested (None): Muted grey / 灰色.
    Timeout,
}

impl LatencyTier {
    pub fn classify(delay_ms: Option<u32>) -> Self {
        match delay_ms {
            Some(0) => Self::Timeout,
            Some(ms) if ms < 100 => Self::Fast,
            Some(ms) if ms < 200 => Self::Normal,
            Some(ms) if ms < 300 => Self::Medium,
            Some(_) => Self::Slow,
            None => Self::Timeout,
        }
    }
}

/// Standardized human label and tier for node latency rendering.
pub fn format_latency_standard(delay_ms: Option<u32>) -> (String, LatencyTier) {
    let tier = LatencyTier::classify(delay_ms);
    let label = match delay_ms {
        Some(0) => "超时".to_owned(),
        Some(ms) => format!("{ms} ms"),
        None => "未测速".to_owned(),
    };
    (label, tier)
}

/// Persistent user preferences for proxy strategies, sorting, filtering, and custom group ordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyUiPreferences {
    /// Strategy group names currently collapsed by the user.
    pub collapsed_groups: Vec<String>,
    /// Global or active sort order.
    pub sort_order: ProxySortOrder,
    /// Filter alive flag (只看可用).
    pub filter_alive: bool,
    /// Pinned favorite proxy node names.
    pub favorite_proxies: Vec<String>,
    /// Compact list view vs 2-4 column grid view.
    pub compact_view: bool,
    /// Custom proxy group reordering (drag-and-drop priority).
    pub custom_group_order: Vec<String>,
}

impl ProxyUiPreferences {
    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.iter().any(|g| g == group)
    }

    pub fn is_group_expanded(&self, group: &str) -> bool {
        !self.is_group_collapsed(group)
    }

    /// Toggle group expansion state. Returns new expanded state (`true` = expanded).
    pub fn toggle_group_expand(&mut self, group: &str) -> bool {
        if let Some(pos) = self.collapsed_groups.iter().position(|g| g == group) {
            self.collapsed_groups.remove(pos);
            true
        } else {
            self.collapsed_groups.push(group.to_owned());
            false
        }
    }

    pub fn set_group_expanded(&mut self, group: &str, expanded: bool) {
        if expanded {
            self.collapsed_groups.retain(|g| g != group);
        } else if !self.is_group_collapsed(group) {
            self.collapsed_groups.push(group.to_owned());
        }
    }

    pub fn expand_all(&mut self) {
        self.collapsed_groups.clear();
    }

    pub fn collapse_all(&mut self, groups: &[String]) {
        self.collapsed_groups = groups.to_vec();
    }

    pub fn is_favorite(&self, proxy: &str) -> bool {
        self.favorite_proxies.iter().any(|p| p == proxy)
    }

    /// Toggle favorite status. Returns new favorite state.
    pub fn toggle_favorite(&mut self, proxy: &str) -> bool {
        if let Some(pos) = self.favorite_proxies.iter().position(|p| p == proxy) {
            self.favorite_proxies.remove(pos);
            false
        } else {
            self.favorite_proxies.push(proxy.to_owned());
            true
        }
    }

    pub fn set_sort_order(&mut self, order: ProxySortOrder) {
        self.sort_order = order;
    }

    pub fn set_filter_alive(&mut self, enabled: bool) {
        self.filter_alive = enabled;
    }

    pub fn set_compact_view(&mut self, compact: bool) {
        self.compact_view = compact;
    }

    pub fn reorder_groups(&mut self, ordered: Vec<String>) {
        self.custom_group_order = ordered;
    }

    pub fn reset_group_order(&mut self) {
        self.custom_group_order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_group_classifications_coverage() {
        let cases = [
            ("Selector", ProxyGroupClassification::Selector, true),
            ("selector", ProxyGroupClassification::Selector, true),
            ("URLTest", ProxyGroupClassification::UrlTest, false),
            ("url-test", ProxyGroupClassification::UrlTest, false),
            ("url_test", ProxyGroupClassification::UrlTest, false),
            ("auto", ProxyGroupClassification::UrlTest, false),
            ("Fallback", ProxyGroupClassification::Fallback, false),
            ("fallback", ProxyGroupClassification::Fallback, false),
            ("LoadBalance", ProxyGroupClassification::LoadBalance, false),
            ("load-balance", ProxyGroupClassification::LoadBalance, false),
            ("lb", ProxyGroupClassification::LoadBalance, false),
            ("Relay", ProxyGroupClassification::Relay, false),
            ("relay", ProxyGroupClassification::Relay, false),
        ];

        for (input, expected, selectable) in cases {
            let parsed = ProxyGroupClassification::from_str_loose(input)
                .unwrap_or_else(|| panic!("failed to parse {input}"));
            assert_eq!(parsed, expected);
            assert_eq!(parsed.is_manual_selectable(), selectable);
            assert_eq!(parsed.is_automated(), !selectable);
        }

        assert_eq!(ProxyGroupClassification::from_str_loose("unknown"), None);
    }

    #[test]
    fn four_way_sorting_and_favorite_pinning() {
        let order_asc = ProxySortOrder::LatencyAsc;
        let order_desc = ProxySortOrder::LatencyDesc;
        let order_name_asc = ProxySortOrder::NameAsc;
        let order_name_desc = ProxySortOrder::NameDesc;

        // Favorite pinning beats lower latency
        assert_eq!(
            order_asc.compare_candidates("Node-A", Some(200), true, "Node-B", Some(50), false),
            Ordering::Less
        );
        assert_eq!(
            order_asc.compare_candidates("Node-A", Some(50), false, "Node-B", Some(200), true),
            Ordering::Greater
        );

        // Within non-favorites: latency asc
        assert_eq!(
            order_asc.compare_candidates("Node-A", Some(50), false, "Node-B", Some(100), false),
            Ordering::Less
        );
        assert_eq!(
            order_asc.compare_candidates("Node-A", Some(150), false, "Node-B", Some(100), false),
            Ordering::Greater
        );

        // Latency desc
        assert_eq!(
            order_desc.compare_candidates("Node-A", Some(50), false, "Node-B", Some(100), false),
            Ordering::Greater
        );
        assert_eq!(
            order_desc.compare_candidates("Node-A", Some(150), false, "Node-B", Some(100), false),
            Ordering::Less
        );

        // Dead/untested (delay None or 0) stays at bottom in latency sort
        assert_eq!(
            order_asc.compare_candidates("Alive", Some(100), false, "Dead", None, false),
            Ordering::Less
        );
        assert_eq!(
            order_asc.compare_candidates("Alive", Some(100), false, "Timeout", Some(0), false),
            Ordering::Less
        );
        assert_eq!(
            order_desc.compare_candidates("Alive", Some(100), false, "Dead", None, false),
            Ordering::Less
        );

        // Name sorting
        assert_eq!(
            order_name_asc.compare_candidates("Alpha", Some(200), false, "Beta", Some(50), false),
            Ordering::Less
        );
        assert_eq!(
            order_name_desc.compare_candidates("Alpha", Some(200), false, "Beta", Some(50), false),
            Ordering::Greater
        );
    }

    #[test]
    fn filter_alive_snapshot_calculations() {
        let nodes = vec![
            (Some(45), true),
            (Some(120), true),
            (Some(0), true),   // timeout
            (None, true),      // untested
            (Some(80), false), // marked dead
        ];

        let snapshot = ProxyFilterAliveSnapshot::derive_from_candidates(nodes, true);
        assert!(snapshot.enabled);
        assert_eq!(snapshot.total_nodes, 5);
        assert_eq!(snapshot.alive_nodes, 2);
        assert_eq!(snapshot.dead_nodes, 3);
    }

    #[test]
    fn proxy_ui_preferences_persistence_logic() {
        let mut prefs = ProxyUiPreferences::default();
        assert!(prefs.is_group_expanded("Proxies"));
        assert!(!prefs.is_group_collapsed("Proxies"));

        // Toggle collapse
        let exp = prefs.toggle_group_expand("Proxies");
        assert!(!exp);
        assert!(prefs.is_group_collapsed("Proxies"));
        assert!(!prefs.is_group_expanded("Proxies"));

        // Toggle expand
        let exp = prefs.toggle_group_expand("Proxies");
        assert!(exp);
        assert!(prefs.is_group_expanded("Proxies"));

        // Favorite toggle
        assert!(!prefs.is_favorite("Node-1"));
        let fav = prefs.toggle_favorite("Node-1");
        assert!(fav);
        assert!(prefs.is_favorite("Node-1"));
        let fav = prefs.toggle_favorite("Node-1");
        assert!(!fav);
        assert!(!prefs.is_favorite("Node-1"));
    }

    #[test]
    fn latency_tier_color_classification() {
        assert_eq!(LatencyTier::classify(Some(45)), LatencyTier::Fast);
        assert_eq!(LatencyTier::classify(Some(120)), LatencyTier::Normal);
        assert_eq!(LatencyTier::classify(Some(250)), LatencyTier::Medium);
        assert_eq!(LatencyTier::classify(Some(350)), LatencyTier::Slow);
        assert_eq!(LatencyTier::classify(Some(0)), LatencyTier::Timeout);
        assert_eq!(LatencyTier::classify(None), LatencyTier::Timeout);

        let (label, tier) = format_latency_standard(Some(88));
        assert_eq!(label, "88 ms");
        assert_eq!(tier, LatencyTier::Fast);

        let (label, tier) = format_latency_standard(Some(0));
        assert_eq!(label, "超时");
        assert_eq!(tier, LatencyTier::Timeout);
    }
}
