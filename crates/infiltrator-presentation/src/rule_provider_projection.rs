//! Shared provider facts and copy, independent of any native surface.
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, KernelEtagSupportState, ProviderCacheFingerprint,
    ProviderFingerprintChange, RuleProviderCacheSnapshot, RuleProviderCacheState,
};
use infiltrator_domain::rules::view::{format_content_fingerprint, format_refresh_interval};
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

pub fn rules_summary(total: usize, providers: usize, catalog: &dyn TextCatalog) -> String {
    interpolate(
        catalog.text("rules_observed_summary").as_ref(),
        &[
            ("rules", &total.to_string()),
            ("providers", &providers.to_string()),
        ],
    )
}
pub fn default_action(target: &str, catalog: &dyn TextCatalog) -> String {
    interpolate(
        catalog.text("rules_observed_default").as_ref(),
        &[("target", target)],
    )
}
pub fn provider_count(count: usize, behavior: &str, catalog: &dyn TextCatalog) -> String {
    interpolate(
        catalog.text("rules_provider_count").as_ref(),
        &[("count", &count.to_string()), ("behavior", behavior)],
    )
}
pub fn published_truncation(
    omitted: Option<usize>,
    limit: usize,
    catalog: &dyn TextCatalog,
) -> String {
    omitted
        .filter(|omitted| *omitted > 0)
        .map(|omitted| {
            interpolate(
                catalog.text("rules_publish_truncated").as_ref(),
                &[
                    ("omitted", &omitted.to_string()),
                    ("limit", &limit.to_string()),
                ],
            )
        })
        .unwrap_or_default()
}
pub fn provider_lifecycle_line(
    updated_at: &str,
    source_url: Option<&str>,
    refresh_interval_secs: Option<u64>,
    catalog: &dyn TextCatalog,
) -> String {
    let formatted_time = if updated_at.is_empty() {
        "—".to_string()
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(updated_at) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        updated_at.to_string()
    };
    let updated = interpolate(
        catalog.text("rules_provider_updated_copy").as_ref(),
        &[("time", &formatted_time)],
    );
    let undeclared = catalog.text("rules_etag_support_not_declared");
    let source = interpolate(
        catalog.text("rules_provider_source_copy").as_ref(),
        &[(
            "source",
            source_url
                .filter(|url| !url.is_empty())
                .unwrap_or(undeclared.as_ref()),
        )],
    );
    let schedule = match refresh_interval_secs {
        Some(seconds) => interpolate(
            catalog.text("rules_provider_schedule_copy").as_ref(),
            &[("interval", &format_refresh_interval(seconds))],
        ),
        None => interpolate(
            catalog.text("rules_provider_schedule_undeclared").as_ref(),
            &[("value", undeclared.as_ref())],
        ),
    };
    format!("{updated} · {source} · {schedule}")
}
pub fn provider_fingerprint_line(
    observation: &ProviderCacheFingerprint,
    catalog: &dyn TextCatalog,
) -> String {
    let change = catalog.text(match observation.change {
        ProviderFingerprintChange::FirstSeen => "rules_provider_fingerprint_first_seen",
        ProviderFingerprintChange::Unchanged => "rules_provider_fingerprint_unchanged",
        ProviderFingerprintChange::Changed => "rules_provider_fingerprint_changed",
    });
    let body = format_content_fingerprint(
        &observation.current.sha256,
        observation.current.size_bytes,
        observation.current.modified_unix_secs,
    );
    format!(
        "{}: {body} · {change}",
        catalog.text("rules_provider_fingerprint_label")
    )
}
pub fn etag_support_line(
    snapshot: &KernelEtagSupportSnapshot,
    catalog: &dyn TextCatalog,
) -> String {
    let state = catalog.text(match snapshot.state {
        KernelEtagSupportState::Enabled => "rules_etag_support_enabled",
        KernelEtagSupportState::Disabled => "rules_etag_support_disabled",
        KernelEtagSupportState::NotDeclared => "rules_etag_support_not_declared",
    });
    format!("{}: {state}", catalog.text("rules_etag_support_label"))
}
pub fn provider_cache_line(cache: &RuleProviderCacheSnapshot, catalog: &dyn TextCatalog) -> String {
    let value = match cache.state {
        RuleProviderCacheState::Ready | RuleProviderCacheState::Empty => interpolate(
            catalog.text("provider_cache_ready").as_ref(),
            &[
                (
                    "dir",
                    cache
                        .directory
                        .as_deref()
                        .unwrap_or(catalog.text("shell_readout_unknown").as_ref()),
                ),
                ("count", &cache.file_count.to_string()),
                ("bytes", &cache.total_bytes.to_string()),
            ],
        ),
        RuleProviderCacheState::Unsupported => {
            catalog.text("provider_cache_unsupported").into_owned()
        }
        RuleProviderCacheState::Failed => catalog.text("provider_cache_failed").into_owned(),
        RuleProviderCacheState::Unknown => catalog.text("provider_cache_unknown").into_owned(),
    };
    match cache.failure.as_deref().filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("{value}: {reason}"),
        None => value,
    }
}
