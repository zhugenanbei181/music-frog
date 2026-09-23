use super::*;

#[test]
fn test_semantic_badge_kind_mapping() {
    assert_eq!(
        semantic_badge_kind("DOMAIN", RuleBadgeKind::Domain),
        BadgeKind::Accent
    );
    assert_eq!(
        semantic_badge_kind("DOMAIN-SUFFIX", RuleBadgeKind::Domain),
        BadgeKind::Accent
    );
    assert_eq!(
        semantic_badge_kind("DOMAIN-KEYWORD", RuleBadgeKind::Domain),
        BadgeKind::Accent
    );
    assert_eq!(
        semantic_badge_kind("IP-CIDR", RuleBadgeKind::Ip),
        BadgeKind::Warning
    );
    assert_eq!(
        semantic_badge_kind("IP-CIDR6", RuleBadgeKind::Ip),
        BadgeKind::Warning
    );
    assert_eq!(
        semantic_badge_kind("IP-ASN", RuleBadgeKind::Ip),
        BadgeKind::Warning
    );
    assert_eq!(
        semantic_badge_kind("GEOIP", RuleBadgeKind::Ip),
        BadgeKind::Neutral
    );
    assert_eq!(
        semantic_badge_kind("GEOSITE", RuleBadgeKind::Other),
        BadgeKind::Neutral
    );
    assert_eq!(
        semantic_badge_kind("MATCH", RuleBadgeKind::Other),
        BadgeKind::Neutral
    );
    assert_eq!(
        semantic_badge_kind("CUSTOM", RuleBadgeKind::Domain),
        BadgeKind::Accent
    );
    assert_eq!(
        semantic_badge_kind("CUSTOM", RuleBadgeKind::Ip),
        BadgeKind::Warning
    );
    assert_eq!(
        semantic_badge_kind("CUSTOM", RuleBadgeKind::Other),
        BadgeKind::Neutral
    );
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
        map.insert(
            "domainsuffix:google.com".to_string(),
            RuleHitStats {
                count: 5,
                is_recent: true,
            },
        );
        map.insert(
            "match:".to_string(),
            RuleHitStats {
                count: 12,
                is_recent: true,
            },
        );
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
    let _rule_element = rule_provider_row(
        &rule_p,
        Some("https://example.com/reject.mrs"),
        Some(86_400),
        None,
        &lang,
    );

    assert_eq!(format_provider_behavior(&rule_p.behavior), "Domain");
    assert_eq!(format_rule_provider_format(&rule_p), "HTTP");
    assert_eq!(total_external_rules(&[rule_p]), 179);
}

#[test]
fn test_provider_lifecycle_line_reports_shared_source_url() {
    // DUAL-11-04: declared URL renders; runtime-only providers stay honest.
    assert_eq!(
        provider_lifecycle_line(
            "2026-09-06 12:00",
            Some("https://example.com/a.mrs"),
            Some(86_400)
        ),
        "Updated: 2026-09-06 12:00 · Source: https://example.com/a.mrs · Auto: 1d (kernel-scheduled)"
    );
    // DUAL-11-05: the declared schedule is disclosed; an undeclared one says so
    // and no cache hit/miss state is invented (the kernel owns ETag/304).
    assert_eq!(
        provider_lifecycle_line("2026-09-06 12:00", None, None),
        "Updated: 2026-09-06 12:00 · Source: not declared · Auto: not declared"
    );
    assert_eq!(
        provider_lifecycle_line("", None, Some(3_600)),
        "Updated: — · Source: not declared · Auto: 1h (kernel-scheduled)"
    );
}

#[test]
fn test_provider_fingerprint_line_reports_local_file_facts() {
    use infiltrator_contract::provider_cache::{
        ProviderCacheFingerprint, ProviderFileFingerprint, ProviderFingerprintChange,
    };

    let observation = ProviderCacheFingerprint {
        provider: "ads".to_owned(),
        path: "/home/u/.config/mihomo-rs/rules/8f14e45fceea167a5a36dedd4bea2543".to_owned(),
        change: ProviderFingerprintChange::Changed,
        current: ProviderFileFingerprint {
            size_bytes: 4_096,
            sha256: "abcdef0123456789deadbeef".to_owned(),
            modified_unix_secs: Some(1_700_000_000),
        },
        previous: Some(ProviderFileFingerprint {
            size_bytes: 2_048,
            sha256: "0123456789abcdef".to_owned(),
            modified_unix_secs: Some(1_699_000_000),
        }),
    };

    let zh = Lang("zh-CN");
    let line = provider_fingerprint_line(&observation, &zh);
    assert!(
        line.starts_with(
            "本地缓存内容指纹（非 HTTP ETag）: sha256:abcdef012345… · 4096 B · mtime 2023-11-14 22:13:20 UTC"
        ),
        "{line}"
    );
    assert!(line.ends_with("较上次观测已变化"), "{line}");
    // The line must never turn the local read into an HTTP validator claim.
    assert!(!line.contains("304"), "{line}");
    assert!(!line.contains("If-None-Match"), "{line}");

    // The English table carries the same fact with the same layout.
    let en = Lang("en");
    let english = provider_fingerprint_line(&observation, &en);
    assert!(
        english.starts_with(
            "Local cache content fingerprint (not an HTTP ETag): sha256:abcdef012345… · 4096 B · mtime 2023-11-14 22:13:20 UTC"
        ),
        "{english}"
    );
    assert!(
        english.ends_with("changed since last observation"),
        "{english}"
    );

    // A first observation says so instead of pretending a previous read.
    let first = ProviderCacheFingerprint {
        change: ProviderFingerprintChange::FirstSeen,
        previous: None,
        ..observation.clone()
    };
    assert!(provider_fingerprint_line(&first, &zh).ends_with("首次观测"));
    let unchanged = ProviderCacheFingerprint {
        change: ProviderFingerprintChange::Unchanged,
        ..observation
    };
    assert!(provider_fingerprint_line(&unchanged, &zh).ends_with("较上次观测未变化"));
}

#[test]
fn test_etag_support_line_reports_the_declared_kernel_capability() {
    use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;

    let zh = Lang("zh-CN");
    let en = Lang("en");

    // DUAL-11-05: the kernel's real top-level `etag-support` declaration is
    // rendered as-is. `etag-support: true` / `false` are explicit; an absent key
    // is honestly "not declared", never presented as an explicit on/off claim.
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(Some(true)), &zh),
        "ETag 缓存: 内核已启用"
    );
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(Some(false)), &zh),
        "ETag 缓存: 内核未启用"
    );
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(None), &zh),
        "ETag 缓存: 未声明"
    );

    // The English table carries the same three states.
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(Some(true)), &en),
        "ETag cache: kernel enabled"
    );
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(Some(false)), &en),
        "ETag cache: kernel disabled"
    );
    assert_eq!(
        etag_support_line(&KernelEtagSupportSnapshot::from_declared(None), &en),
        "ETag cache: not declared"
    );

    // The line is a declaration fact: it never renders a per-request 304
    // outcome, which the kernel does not expose.
    let declared = etag_support_line(&KernelEtagSupportSnapshot::from_declared(Some(true)), &zh);
    assert!(!declared.contains("304"), "{declared}");
    assert!(!declared.contains("If-None-Match"), "{declared}");
}
