//! Top-level `hosts` mapping conversion between the profile's
//! domain → value map and the shared flat [`DnsHostEntry`] row list.
//!
//! The host accepts three value shapes for one domain: a scalar IP, a scalar
//! alias domain, or a list of IPs. The shared editor is flat (one row per
//! address), so these two functions own the lossless round-trip; grouping a
//! single domain into a scalar keeps the written profile identical to the
//! common case, and grouping into a list preserves multi-address domains.

use anyhow::{Context, Result, anyhow};
use infiltrator_contract::dns_hosts::{DnsHostEntry, validate_hosts};
use serde_json::value;
use serde_yaml_ng::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Group flat rows into the profile's `hosts` map.
///
/// One row for a domain writes a scalar string; several rows write a list in
/// first-seen order. Later duplicates of the same `(domain, address)` pair are
/// dropped so the editor cannot produce a repeated list entry.
pub fn hosts_map_from_entries(entries: &[DnsHostEntry]) -> Result<BTreeMap<String, value::Value>> {
    if let Some(issue) = validate_hosts(entries).first() {
        return Err(anyhow!("invalid Hosts mapping: {issue:?}"));
    }
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        let domain = entry.domain.trim();
        let address = entry.address.trim();
        if domain.is_empty() || address.is_empty() {
            continue;
        }
        let addresses = grouped.entry(domain.to_lowercase()).or_default();
        if !addresses.iter().any(|existing| existing == address) {
            addresses.push(address.to_owned());
        }
    }

    Ok(grouped
        .into_iter()
        .map(|(domain, mut addresses)| {
            let value = if addresses.len() == 1 {
                value::Value::String(addresses.remove(0))
            } else {
                value::Value::Array(addresses.into_iter().map(value::Value::String).collect())
            };
            (domain, value)
        })
        .collect())
}

/// Flatten the profile's `hosts` map into one row per address.
///
/// Non-string list members (a shape the host would reject) are skipped rather
/// than rendered as a fabricated address.
pub fn hosts_entries_from_map(map: &BTreeMap<String, value::Value>) -> Result<Vec<DnsHostEntry>> {
    let identities: BTreeSet<_> = map.keys().map(|key| key.to_lowercase()).collect();
    if identities.len() != map.len() {
        return Err(anyhow!(
            "Hosts contains keys with the same case-insensitive identity"
        ));
    }
    let mut entries = Vec::new();
    for (domain, value) in map {
        match value {
            value::Value::String(address) => entries.push(DnsHostEntry {
                domain: domain.clone(),
                address: address.clone(),
            }),
            value::Value::Array(addresses) => {
                for address in addresses {
                    if let value::Value::String(address) = address {
                        entries.push(DnsHostEntry {
                            domain: domain.clone(),
                            address: address.clone(),
                        });
                    } else {
                        return Err(anyhow!(
                            "Hosts mapping {domain} contains a non-string address"
                        ));
                    }
                }
                if addresses.is_empty() {
                    return Err(anyhow!("Hosts mapping {domain} has no addresses"));
                }
            }
            _ => {
                return Err(anyhow!(
                    "Hosts mapping {domain} is not a string or address list"
                ));
            }
        }
    }
    if let Some(issue) = validate_hosts(&entries).first() {
        return Err(anyhow!("invalid Hosts mapping: {issue:?}"));
    }
    Ok(entries)
}

/// Read either the active root map or the preserved historical nested map.
pub fn entries_from_document(doc: &Value, legacy: bool) -> Result<Vec<DnsHostEntry>> {
    if !doc.is_mapping() {
        return Err(anyhow!("profile config is not a mapping"));
    }
    let container = if legacy { doc.get("dns") } else { Some(doc) };
    let Some(raw) = container.and_then(|container| container.get("hosts")) else {
        return Ok(Vec::new());
    };
    let map: BTreeMap<String, value::Value> =
        serde_yaml_ng::from_value(raw.clone()).context("decode Hosts mapping")?;
    hosts_entries_from_map(&map)
}

/// Write the key the locked kernel actually reads; migration removal is explicit.
pub fn apply_hosts_patch_to_yaml(
    content: &str,
    entries: &[DnsHostEntry],
    remove_legacy: bool,
) -> Result<String> {
    let map = hosts_map_from_entries(entries)?;
    let mut doc: Value = serde_yaml_ng::from_str(content).context("parse profile yaml")?;
    let root = doc
        .as_mapping_mut()
        .ok_or_else(|| anyhow!("profile config is not a mapping"))?;
    let key = Value::String("hosts".into());
    if map.is_empty() {
        root.remove(&key);
    } else {
        root.insert(
            key,
            serde_yaml_ng::to_value(map).context("encode Hosts mapping")?,
        );
    }
    if remove_legacy
        && let Some(dns) = root
            .get_mut(Value::String("dns".into()))
            .and_then(Value::as_mapping_mut)
    {
        dns.remove(Value::String("hosts".into()));
    }
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

#[cfg(test)]
#[path = "dns_hosts_test.rs"]
mod tests;
