//! One fold for Hosts summaries, validation and operation feedback on both peers.
use crate::dns_hosts_editor::DnsHostsEditor;
use infiltrator_contract::dns_hosts::{DnsHostsIssue, validate_hosts};
use infiltrator_shared::i18n_interpolator::localize;
use std::collections::HashSet;
pub fn issue_text(issue: &DnsHostsIssue, code: &str) -> String {
    let key = match issue {
        DnsHostsIssue::InvalidAddress { .. } => "dns_hosts_issue_address",
        DnsHostsIssue::InvalidDomain { .. } => "dns_hosts_issue_domain",
        DnsHostsIssue::MissingDomain { .. } => "dns_hosts_issue_missing_domain",
        DnsHostsIssue::MixedAddressList { .. } => "dns_hosts_issue_mixed_list",
        DnsHostsIssue::AliasCycle { .. } => "dns_hosts_issue_alias_cycle",
        DnsHostsIssue::PatternConflict { .. } => "dns_hosts_issue_pattern_conflict",
    };
    localize(code, key, &[("value", issue.token().into())])
}
pub fn summary(editor: &DnsHostsEditor, code: &str) -> String {
    let Some(profile) = &editor.applied else {
        return localize(code, "dns_hosts_unobserved", &[]);
    };
    let domains: HashSet<_> = profile.entries.iter().map(|entry| &entry.domain).collect();
    localize(
        code,
        "dns_hosts_summary",
        &[
            ("addresses", profile.entries.len().to_string()),
            ("domains", domains.len().to_string()),
            ("profile", profile.profile.clone()),
        ],
    )
}
pub fn feedback(editor: &DnsHostsEditor, code: &str) -> String {
    if editor.pending.is_some() {
        return localize(code, "dns_hosts_saving", &[]);
    }
    if let Some(failure) = editor.failure.as_ref().or(editor.read_failure.as_ref()) {
        return localize(
            code,
            "dns_hosts_failed",
            &[("reason", failure.message.clone())],
        );
    }
    if let Some(issue) = editor.issues.first() {
        return issue_text(issue, code);
    }
    let rows: Vec<_> = editor.rows.iter().map(|row| row.entry.clone()).collect();
    if let Some(issue) = validate_hosts(&rows).first() {
        return issue_text(issue, code);
    }
    if editor.applied.is_none() {
        return localize(code, "dns_hosts_unobserved", &[]);
    }
    localize(
        code,
        if editor.dirty {
            "dns_hosts_pending"
        } else {
            "dns_hosts_saved"
        },
        &[],
    )
}
pub fn legacy_hint(editor: &DnsHostsEditor, code: &str) -> String {
    let count = editor
        .applied
        .as_ref()
        .map_or(0, |profile| profile.legacy_entries.len());
    if count == 0 {
        String::new()
    } else {
        localize(
            code,
            if editor.importing_legacy {
                "dns_hosts_legacy_preview"
            } else {
                "dns_hosts_legacy_available"
            },
            &[("count", count.to_string())],
        )
    }
}
