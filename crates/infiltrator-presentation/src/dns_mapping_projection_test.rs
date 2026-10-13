use super::project_mappings;
use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};

#[test]
fn mapping_search_preserves_observed_subset_and_distinguishes_no_matches_from_missing_feed() {
    let mut pool = FakeIpMappingPool {
        source: FakeIpMappingSource::LiveConnections,
        range: "198.18.0.1/16 {shown}".into(),
        total: 2,
        entries: vec![
            FakeIpMappingEntry {
                domain: "Music{total}.example".into(),
                address: "198.18.0.5".into(),
            },
            FakeIpMappingEntry {
                domain: "cdn.example".into(),
                address: "198.18.0.7".into(),
            },
        ],
    };
    for code in ["en-US", "zh-CN"] {
        let filtered = project_mappings(&pool, " MUSIC ", code);
        assert_eq!(filtered.rows, vec![pool.entries[0].clone()]);
        assert_eq!(filtered.listing(), "198.18.0.5 ↔ Music{total}.example");
        assert!(filtered.count.contains("1"));
        assert!(filtered.count.contains("2"));
        assert!(filtered.count.ends_with("198.18.0.1/16 {shown})"));
        let absent = project_mappings(&pool, "missing", code);
        assert!(absent.rows.is_empty());
        let mut empty = pool.clone();
        empty.entries.clear();
        empty.total = 0;
        assert_ne!(
            absent.empty,
            project_mappings(&empty, "missing", code).empty
        );
    }
    pool.source = FakeIpMappingSource::Unsupported {
        reason: "no port {shown}".into(),
    };
    let unsupported = project_mappings(&pool, "", "en-US");
    assert!(unsupported.rows.is_empty());
    assert!(unsupported.empty.ends_with("no port {shown})"));
    pool.source = FakeIpMappingSource::Unavailable {
        reason: "connection failure {shown}".into(),
    };
    let unavailable = project_mappings(&pool, "", "en-US");
    assert!(unavailable.rows.is_empty());
    assert!(unavailable.empty.ends_with("connection failure {shown})"));
    assert_ne!(unsupported.source, unavailable.source);
}
