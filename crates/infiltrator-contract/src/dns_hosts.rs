//! Strict shared Hosts rows, text codec and validation; malformed input never means clear.
use crate::dns_hosts_alias::alias_issues;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

/// DUAL-14-11: one top-level `hosts` mapping row.
///
/// The address token follows the host's own `hosts` value grammar: an IP
/// literal, the `lan` keyword, or an alias domain. A domain with several IPs
/// is published as one row per address so the editor round-trips losslessly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsHostEntry {
    pub domain: String,
    pub address: String,
}

/// Active top-level mappings and preserved, unapplied legacy mappings share one profile identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsHostsProfile {
    pub profile: String,
    pub entries: Vec<DnsHostEntry>,
    pub legacy_entries: Vec<DnsHostEntry>,
}

/// DUAL-14-11: a locally detected hosts-editor issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsHostsIssue {
    /// The value is neither an IP literal, `lan`, nor a domain alias.
    InvalidAddress { address: String },
    /// The mapping key is empty or carries whitespace.
    InvalidDomain { domain: String },
    /// A nonempty text row contains an address but no mapping key.
    MissingDomain { address: String },
    /// Multiple values for one domain must all be IP literals.
    MixedAddressList { domain: String },
    /// Alias chains, including wildcard matches, must not return to a visited mapping key.
    AliasCycle { domain: String },
    /// Expanded wildcard keys cannot assign different values to the same trie node.
    PatternConflict { domain: String },
}

impl DnsHostsIssue {
    /// The offending token, for localized copy.
    pub fn token(&self) -> &str {
        match self {
            Self::InvalidAddress { address } => address,
            Self::InvalidDomain { domain } => domain,
            Self::MissingDomain { address } => address,
            Self::MixedAddressList { domain }
            | Self::AliasCycle { domain }
            | Self::PatternConflict { domain } => domain,
        }
    }
}

/// Parse a shared hosts editor into mapping rows.
///
/// Rows are separated by newlines or `;` (so a single-line text field on one
/// surface and a multi-line editor on the other parse identically). Each row is
/// `address domain [domain ...]`, hosts-file order; `#` starts a comment.
pub fn parse_hosts_editor(raw: &str) -> Result<Vec<DnsHostEntry>, Vec<DnsHostsIssue>> {
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    for row in raw.split(['\n', ';']) {
        let row = row.split('#').next().unwrap_or("").trim();
        if row.is_empty() {
            continue;
        }
        let mut tokens = row.split_whitespace();
        let Some(address) = tokens.next() else {
            continue;
        };
        let domains: Vec<_> = tokens.collect();
        if domains.is_empty() {
            issues.push(DnsHostsIssue::MissingDomain {
                address: address.to_owned(),
            });
        }
        for domain in domains {
            entries.push(DnsHostEntry {
                domain: domain.to_owned(),
                address: address.to_owned(),
            });
        }
    }
    issues.extend(validate_hosts(&entries));
    if issues.is_empty() {
        Ok(entries)
    } else {
        Err(issues)
    }
}

/// The canonical editor string for a hosts mapping (one row per entry).
pub fn hosts_editor_text(entries: &[DnsHostEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} {}", entry.address, entry.domain))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append one `address domain` row, ignoring an exact duplicate.
pub fn append_host_row(
    raw: &str,
    address: &str,
    domain: &str,
) -> Result<String, Vec<DnsHostsIssue>> {
    let address = address.trim();
    let domain = domain.trim();
    if address.is_empty() || domain.is_empty() {
        return Ok(raw.to_owned());
    }
    let mut entries = parse_hosts_editor(raw)?;
    if !entries
        .iter()
        .any(|entry| entry.domain.eq_ignore_ascii_case(domain) && entry.address == address)
    {
        entries.push(DnsHostEntry {
            domain: domain.to_owned(),
            address: address.to_owned(),
        });
    }
    let issues = validate_hosts(&entries);
    if issues.is_empty() {
        Ok(hosts_editor_text(&entries))
    } else {
        Err(issues)
    }
}

/// Remove one row from the raw hosts editor string.
pub fn remove_host_at(raw: &str, index: usize) -> Result<String, Vec<DnsHostsIssue>> {
    let mut entries = parse_hosts_editor(raw)?;
    if index < entries.len() {
        entries.remove(index);
    }
    Ok(hosts_editor_text(&entries))
}

/// Kernel IP literals include IPv6 zone identifiers; a zone is never a domain alias.
pub fn is_hosts_ip(value: &str) -> bool {
    if value.parse::<Ipv4Addr>().is_ok() || value.parse::<Ipv6Addr>().is_ok() {
        return true;
    }
    value.split_once('%').is_some_and(|(address, zone)| {
        !zone.is_empty()
            && !zone.contains(char::is_whitespace)
            && address.parse::<Ipv6Addr>().is_ok()
    })
}

/// Whether a hosts value follows the host's accepted grammar.
pub fn is_valid_hosts_address(address: &str) -> bool {
    let value = address.trim();
    if value.is_empty() {
        return false;
    }
    if value == "lan" {
        return true;
    }
    if is_hosts_ip(value) {
        return true;
    }
    // The host resolves a value with two or more labels as an alias domain.
    !value.contains(char::is_whitespace)
        && value.split('.').filter(|part| !part.is_empty()).count() >= 2
}

/// Whether a hosts mapping key is acceptable for the host trie.
pub fn is_valid_hosts_domain(domain: &str) -> bool {
    let value = domain.trim();
    !value.is_empty()
        && !value.contains(char::is_whitespace)
        && !value.contains(':')
        && !value.ends_with('.')
        && value.split('.').skip(1).all(|part| !part.is_empty())
}

/// Validate a hosts mapping; both surfaces localize the returned issues.
pub fn validate_hosts(entries: &[DnsHostEntry]) -> Vec<DnsHostsIssue> {
    let mut issues = Vec::new();
    for entry in entries {
        if !is_valid_hosts_domain(&entry.domain) {
            issues.push(DnsHostsIssue::InvalidDomain {
                domain: entry.domain.clone(),
            });
        }
        if !is_valid_hosts_address(&entry.address) {
            issues.push(DnsHostsIssue::InvalidAddress {
                address: entry.address.clone(),
            });
        }
    }
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for entry in entries {
        let values = groups.entry(entry.domain.to_lowercase()).or_default();
        if !values.contains(&entry.address.as_str()) {
            values.push(&entry.address);
        }
    }
    for (domain, values) in &groups {
        if values.len() > 1 && values.iter().any(|value| !is_hosts_ip(value)) {
            issues.push(DnsHostsIssue::MixedAddressList {
                domain: domain.clone(),
            });
        }
    }
    let (cycles, conflicts) = alias_issues(&groups);
    issues.extend(
        cycles
            .into_iter()
            .map(|domain| DnsHostsIssue::AliasCycle { domain }),
    );
    issues.extend(
        conflicts
            .into_iter()
            .map(|domain| DnsHostsIssue::PatternConflict { domain }),
    );
    issues
}

#[cfg(test)]
#[path = "dns_hosts_test.rs"]
mod tests;
