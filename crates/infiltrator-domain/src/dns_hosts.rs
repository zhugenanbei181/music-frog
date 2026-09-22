//! DUAL-14-11: `dns.hosts` mapping conversion between the profile's
//! domain → value map and the shared flat [`DnsHostEntry`] row list.
//!
//! The host accepts three value shapes for one domain: a scalar IP, a scalar
//! alias domain, or a list of IPs. The shared editor is flat (one row per
//! address), so these two functions own the lossless round-trip; grouping a
//! single domain into a scalar keeps the written profile identical to the
//! common case, and grouping into a list preserves multi-address domains.

use infiltrator_contract::dns::DnsHostEntry;
use serde_json::Value;
use std::collections::BTreeMap;

/// Group flat rows into the profile's `hosts` map.
///
/// One row for a domain writes a scalar string; several rows write a list in
/// first-seen order. Later duplicates of the same `(domain, address)` pair are
/// dropped so the editor cannot produce a repeated list entry.
pub fn hosts_map_from_entries(entries: &[DnsHostEntry]) -> BTreeMap<String, Value> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        let domain = entry.domain.trim();
        let address = entry.address.trim();
        if domain.is_empty() || address.is_empty() {
            continue;
        }
        let addresses = grouped.entry(domain.to_owned()).or_default();
        if !addresses.iter().any(|existing| existing == address) {
            addresses.push(address.to_owned());
        }
    }

    grouped
        .into_iter()
        .map(|(domain, mut addresses)| {
            let value = if addresses.len() == 1 {
                Value::String(addresses.remove(0))
            } else {
                Value::Array(addresses.into_iter().map(Value::String).collect())
            };
            (domain, value)
        })
        .collect()
}

/// Flatten the profile's `hosts` map into one row per address.
///
/// Non-string list members (a shape the host would reject) are skipped rather
/// than rendered as a fabricated address.
pub fn hosts_entries_from_map(map: &BTreeMap<String, Value>) -> Vec<DnsHostEntry> {
    let mut entries = Vec::new();
    for (domain, value) in map {
        match value {
            Value::String(address) => entries.push(DnsHostEntry {
                domain: domain.clone(),
                address: address.clone(),
            }),
            Value::Array(addresses) => {
                for address in addresses {
                    if let Value::String(address) = address {
                        entries.push(DnsHostEntry {
                            domain: domain.clone(),
                            address: address.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let map = hosts_map_from_entries(&entries);
        assert_eq!(
            map.get("single.example.com"),
            Some(&Value::String("9.9.9.9".to_owned()))
        );
        assert_eq!(
            map.get("multi.example.com"),
            Some(&Value::Array(vec![
                Value::String("1.1.1.1".to_owned()),
                Value::String("8.8.8.8".to_owned())
            ]))
        );
        let mut back = hosts_entries_from_map(&map);
        back.sort_by(|a, b| a.domain.cmp(&b.domain).then(a.address.cmp(&b.address)));
        let mut expected = entries;
        expected.sort_by(|a, b| a.domain.cmp(&b.domain).then(a.address.cmp(&b.address)));
        assert_eq!(back, expected);
    }

    #[test]
    fn duplicate_rows_collapse_and_blank_rows_are_dropped() {
        let map = hosts_map_from_entries(&[
            entry("a.example.com", "1.1.1.1"),
            entry("a.example.com", "1.1.1.1"),
            entry("  ", "1.1.1.1"),
            entry("b.example.com", "  "),
        ]);
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("a.example.com"),
            Some(&Value::String("1.1.1.1".to_owned()))
        );
    }

    #[test]
    fn non_string_shapes_are_skipped_not_fabricated() {
        let mut map = BTreeMap::new();
        map.insert("legacy.example.com".to_owned(), Value::Bool(true));
        map.insert(
            "mixed.example.com".to_owned(),
            Value::Array(vec![
                Value::String("1.1.1.1".to_owned()),
                Value::Number(7.into()),
            ]),
        );
        let entries = hosts_entries_from_map(&map);
        assert_eq!(entries, vec![entry("mixed.example.com", "1.1.1.1")]);
    }
}
