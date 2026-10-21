//! The published rule row implements the shared rule-search seam.
use crate::rules::view::RuleView;
use infiltrator_contract::rule_snapshot::RuleSnapshot;

/// The published rule row is searchable by its expression.
///
/// The publisher keeps the raw expression when it has one, so the search
/// answers the same expression the editor shows. A row without a raw
/// expression is recombined from its parts, so its type, payload and target
/// stay searchable too.
impl RuleView for RuleSnapshot {
    fn view_rule_search(&self) -> String {
        if self.raw.is_empty() {
            format!("{},{},{}", self.rule_type, self.payload, self.proxy)
        } else {
            self.raw.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(raw: &str, rule_type: &str, payload: &str, proxy: &str) -> RuleSnapshot {
        RuleSnapshot {
            id: 1,
            edit_id: None,
            rule_type: rule_type.to_owned(),
            payload: payload.to_owned(),
            proxy: proxy.to_owned(),
            hit_count: None,
            raw: raw.to_owned(),
            source_ip: false,
            failure: None,
            is_enabled: true,
            no_resolve: false,
            last_hit_secs: None,
            is_shadowed: false,
            shadow_reason: None,
        }
    }

    #[test]
    fn the_published_rule_is_searchable_by_its_expression() {
        let rule = snapshot(
            "DOMAIN-SUFFIX,github.com,PROXY",
            "DOMAIN-SUFFIX",
            "github.com",
            "PROXY",
        );
        assert_eq!(rule.view_rule_search(), "DOMAIN-SUFFIX,github.com,PROXY");
        assert!(rule.view_rule_search().contains("github"));
        assert!(rule.view_rule_search().contains("PROXY"));
    }

    #[test]
    fn a_published_rule_without_a_raw_expression_is_recombined() {
        let rule = snapshot("", "GEOIP", "CN", "DIRECT");
        assert_eq!(rule.view_rule_search(), "GEOIP,CN,DIRECT");
        assert!(rule.view_rule_search().contains("CN"));
        assert!(rule.view_rule_search().contains("DIRECT"));
    }
}
