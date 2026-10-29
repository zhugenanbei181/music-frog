//! Multi-dimensional proxy filter query and batch action specification (FLPARITY-023).

use crate::geo_region::GeoRegion;
use crate::proxies::ProxyProtocolBadge;
use serde::{Deserialize, Serialize};

/// Advanced multi-dimensional proxy filter query.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyFilterQuery {
    /// Multi-selected protocols. Empty means all protocols are accepted.
    #[serde(default)]
    pub protocols: Vec<ProxyProtocolBadge>,
    /// Multi-selected geographical regions. Empty means all regions are accepted.
    #[serde(default)]
    pub regions: Vec<GeoRegion>,
    /// Minimum latency threshold in milliseconds (e.g. 50ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_latency_ms: Option<u32>,
    /// Maximum latency threshold in milliseconds (e.g. 300ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_latency_ms: Option<u32>,
    /// Optional positive search keyword (display name, protocol, pinyin initials).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_keyword: Option<String>,
    /// Negative exclusion keywords (e.g. ["重置", "官网", "expired"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_keywords: Vec<String>,
    /// Filter only pinned favorite nodes.
    #[serde(default)]
    pub favorite_only: bool,
}

impl ProxyFilterQuery {
    /// Create a new query matching all nodes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this filter has any active criteria restricting nodes.
    pub fn is_active(&self) -> bool {
        !self.protocols.is_empty()
            || !self.regions.is_empty()
            || self.min_latency_ms.is_some()
            || self.max_latency_ms.is_some()
            || self
                .search_keyword
                .as_deref()
                .is_some_and(|k| !k.trim().is_empty())
            || !self.exclude_keywords.is_empty()
            || self.favorite_only
    }

    /// Reset all criteria back to matching all nodes.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Parse free-text search query, extracting negative exclusion tokens (e.g. `-重置`, `!expired`).
    pub fn from_raw_search(input: &str) -> Self {
        let mut query = Self::default();
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return query;
        }

        let mut positive_tokens = Vec::new();
        for token in trimmed.split_whitespace() {
            if let Some(negative) = token.strip_prefix('-').or_else(|| token.strip_prefix('!')) {
                if !negative.trim().is_empty() {
                    query
                        .exclude_keywords
                        .push(negative.trim().to_ascii_lowercase());
                }
            } else if token.starts_with('<')
                && let Ok(max) = token[1..].trim().parse::<u32>()
            {
                query.max_latency_ms = Some(max);
            } else if token.starts_with('>')
                && let Ok(min) = token[1..].trim().parse::<u32>()
            {
                query.min_latency_ms = Some(min);
            } else {
                positive_tokens.push(token);
            }
        }

        if !positive_tokens.is_empty() {
            query.search_keyword = Some(positive_tokens.join(" ").to_ascii_lowercase());
        }

        query
    }
}

/// Batch action on the filtered set of proxy nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchProxyAction {
    /// Benchmark latency of all currently matching nodes.
    BenchmarkFiltered,
    /// Export share URIs of all currently matching nodes.
    ExportFilteredUris,
    /// Pin all currently matching nodes as favorites.
    FavoriteAllFiltered,
    /// Unpin all currently matching nodes.
    UnfavoriteAllFiltered,
}

impl BatchProxyAction {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::BenchmarkFiltered => "测速筛选结果",
            Self::ExportFilteredUris => "导出节点链接",
            Self::FavoriteAllFiltered => "批量设为常用",
            Self::UnfavoriteAllFiltered => "批量取消常用",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::BenchmarkFiltered => "Benchmark Filtered",
            Self::ExportFilteredUris => "Export Node URIs",
            Self::FavoriteAllFiltered => "Favorite Filtered",
            Self::UnfavoriteAllFiltered => "Unfavorite Filtered",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_is_inactive() {
        let query = ProxyFilterQuery::default();
        assert!(!query.is_active());
    }

    #[test]
    fn raw_search_parses_exclusions_and_latency_bounds() {
        let query = ProxyFilterQuery::from_raw_search("HK -重置 !expired <200 >50");
        assert_eq!(query.search_keyword.as_deref(), Some("hk"));
        assert_eq!(
            query.exclude_keywords,
            vec!["重置".to_owned(), "expired".to_owned()]
        );
        assert_eq!(query.max_latency_ms, Some(200));
        assert_eq!(query.min_latency_ms, Some(50));
        assert!(query.is_active());
    }

    #[test]
    fn batch_action_labels_are_sound() {
        assert_eq!(
            BatchProxyAction::BenchmarkFiltered.label_zh(),
            "测速筛选结果"
        );
        assert_eq!(
            BatchProxyAction::BenchmarkFiltered.label_en(),
            "Benchmark Filtered"
        );
    }
}
