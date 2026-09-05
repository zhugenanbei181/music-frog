use super::*;

#[test]
fn test_semantic_badge_kind_mapping() {
    assert_eq!(semantic_badge_kind("DOMAIN", RuleBadgeKind::Domain), BadgeKind::Accent);
    assert_eq!(semantic_badge_kind("DOMAIN-SUFFIX", RuleBadgeKind::Domain), BadgeKind::Accent);
    assert_eq!(semantic_badge_kind("DOMAIN-KEYWORD", RuleBadgeKind::Domain), BadgeKind::Accent);
    assert_eq!(semantic_badge_kind("IP-CIDR", RuleBadgeKind::Ip), BadgeKind::Warning);
    assert_eq!(semantic_badge_kind("IP-CIDR6", RuleBadgeKind::Ip), BadgeKind::Warning);
    assert_eq!(semantic_badge_kind("IP-ASN", RuleBadgeKind::Ip), BadgeKind::Warning);
    assert_eq!(semantic_badge_kind("GEOIP", RuleBadgeKind::Ip), BadgeKind::Neutral);
    assert_eq!(semantic_badge_kind("GEOSITE", RuleBadgeKind::Other), BadgeKind::Neutral);
    assert_eq!(semantic_badge_kind("MATCH", RuleBadgeKind::Other), BadgeKind::Neutral);
    assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Domain), BadgeKind::Accent);
    assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Ip), BadgeKind::Warning);
    assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Other), BadgeKind::Neutral);
}

#[test]
fn test_display_rule_type_formatting() {
    assert_eq!(display_rule_type("DOMAIN"), "Domain");
    assert_eq!(display_rule_type("DOMAIN-SUFFIX"), "DomainSuffix");
    assert_eq!(display_rule_type("IP-CIDR"), "IPCIDR");
    assert_eq!(display_rule_type("GEOIP"), "GeoIP");
    assert_eq!(display_rule_type("MATCH"), "Match");
    assert_eq!(display_rule_type("RULE-SET"), "RuleSet");
}

#[test]
fn test_rule_hit_stats_matching() {
    let stats_map = {
        let mut map = HashMap::new();
        map.insert("domainsuffix:google.com".to_string(), RuleHitStats { count: 5, is_recent: true });
        map.insert("match:".to_string(), RuleHitStats { count: 12, is_recent: true });
        map
    };

    let hit = lookup_hit_stats(&stats_map, "DOMAIN-SUFFIX", "google.com");
    assert_eq!(hit.count, 5);
    assert!(hit.is_recent);

    let match_hit = lookup_hit_stats(&stats_map, "MATCH", "");
    assert_eq!(match_hit.count, 12);
    assert!(match_hit.is_recent);

    let unhit = lookup_hit_stats(&stats_map, "DOMAIN", "unknown.com");
    assert_eq!(unhit.count, 0);
    assert!(!unhit.is_recent);
}

#[test]
fn test_proxy_and_rule_provider_row_render() {
    let lang = Lang("en");
    let proxy_p = ProxyProvider {
        name: "DefaultProxies".into(),
        provider_type: "http".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-09-02 12:00:00".into(),
    };
    let _proxy_element = proxy_provider_row(&proxy_p, &lang);

    let rule_p = RuleProvider {
        name: "RejectAds".into(),
        provider_type: "http".into(),
        behavior: "domain".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-09-02 12:00:00".into(),
        rule_count: 179,
    };
    let _rule_element = rule_provider_row(&rule_p, &lang);

    assert_eq!(format_provider_behavior(&rule_p.behavior), "Domain");
    assert_eq!(format_rule_provider_format(&rule_p), "HTTP");
    assert_eq!(total_external_rules(&[rule_p]), 179);
}
