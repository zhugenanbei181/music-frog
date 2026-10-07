//! test-intent: behavior
use super::*;
use serde_json::json;

fn entry(domain: &str, address: &str) -> DnsHostEntry {
    DnsHostEntry {
        domain: domain.to_owned(),
        address: address.to_owned(),
    }
}

#[test]
fn scalar_and_list_values_round_trip_losslessly() {
    let entries = vec![
        entry("multi.example.com", "1.1.1.1"),
        entry("multi.example.com", "8.8.8.8"),
        entry("single.example.com", "9.9.9.9"),
        entry("alias.example.com", "target.example.net"),
    ];
    let map = hosts_map_from_entries(&entries).unwrap();
    assert_eq!(
        map.get("single.example.com"),
        Some(&value::Value::String("9.9.9.9".to_owned()))
    );
    assert_eq!(
        map.get("multi.example.com"),
        Some(&value::Value::Array(vec![
            value::Value::String("1.1.1.1".to_owned()),
            value::Value::String("8.8.8.8".to_owned())
        ]))
    );
    let mut back = hosts_entries_from_map(&map).unwrap();
    back.sort_by(|a, b| a.domain.cmp(&b.domain).then(a.address.cmp(&b.address)));
    let mut expected = entries;
    expected.sort_by(|a, b| a.domain.cmp(&b.domain).then(a.address.cmp(&b.address)));
    assert_eq!(back, expected);
}

#[test]
fn duplicates_collapse_but_invalid_rows_and_shapes_are_rejected_without_data_loss() {
    let map = hosts_map_from_entries(&[
        entry("a.example.com", "1.1.1.1"),
        entry("a.example.com", "1.1.1.1"),
    ])
    .unwrap();
    assert_eq!(map.len(), 1);
    for rows in [
        vec![entry(" ", "1.1.1.1")],
        vec![entry("b.example.com", " ")],
    ] {
        assert!(hosts_map_from_entries(&rows).is_err());
    }
    for raw in [
        value::Value::Bool(true),
        value::Value::Array(vec![
            value::Value::String("1.1.1.1".into()),
            value::Value::Number(7.into()),
        ]),
        value::Value::Array(Vec::new()),
    ] {
        let map = BTreeMap::from([("bad.example.com".into(), raw)]);
        assert!(hosts_entries_from_map(&map).is_err());
    }
}

#[test]
fn actual_root_write_clear_and_explicit_legacy_migration_preserve_unrelated_fields() {
    let original = "mixed-port: 7890\ndns:\n  enable: true\n  hosts:\n    old.test: 1.1.1.1\nhosts:\n  live.test: 9.9.9.9\n";
    let doc: Value = serde_yaml_ng::from_str(original).unwrap();
    assert_eq!(
        entries_from_document(&doc, false).unwrap(),
        vec![entry("live.test", "9.9.9.9")]
    );
    assert_eq!(
        entries_from_document(&doc, true).unwrap(),
        vec![entry("old.test", "1.1.1.1")]
    );
    let updated =
        apply_hosts_patch_to_yaml(original, &[entry("new.test", "8.8.8.8")], false).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&updated).unwrap();
    assert_eq!(
        entries_from_document(&doc, false).unwrap(),
        vec![entry("new.test", "8.8.8.8")]
    );
    assert_eq!(
        entries_from_document(&doc, true).unwrap(),
        vec![entry("old.test", "1.1.1.1")]
    );
    assert_eq!(doc["mixed-port"].as_u64(), Some(7890));
    assert_eq!(doc["dns"]["enable"].as_bool(), Some(true));
    let migrated = apply_hosts_patch_to_yaml(
        &updated,
        &[entry("old.test", "1.1.1.1"), entry("new.test", "8.8.8.8")],
        true,
    )
    .unwrap();
    let doc: Value = serde_yaml_ng::from_str(&migrated).unwrap();
    assert!(doc["dns"].get("hosts").is_none());
    assert_eq!(entries_from_document(&doc, false).unwrap().len(), 2);
    let cleared = apply_hosts_patch_to_yaml(&migrated, &[], false).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&cleared).unwrap();
    assert!(doc.get("hosts").is_none());
    assert_eq!(doc["mixed-port"].as_u64(), Some(7890));
}

#[test]
fn case_insensitive_keys_are_canonical_on_write_and_ambiguous_existing_maps_are_rejected() {
    let map = hosts_map_from_entries(&[
        entry("Mixed.TEST", "1.1.1.1"),
        entry("mixed.test", "2.2.2.2"),
    ])
    .unwrap();
    assert_eq!(map.len(), 1);
    assert_eq!(
        hosts_entries_from_map(&map).unwrap(),
        vec![
            entry("mixed.test", "1.1.1.1"),
            entry("mixed.test", "2.2.2.2")
        ]
    );
    let ambiguous = BTreeMap::from([
        ("Mixed.TEST".into(), json!("1.1.1.1")),
        ("mixed.test".into(), json!("2.2.2.2")),
    ]);
    assert!(
        hosts_entries_from_map(&ambiguous)
            .unwrap_err()
            .to_string()
            .contains("same case-insensitive identity")
    );
}
