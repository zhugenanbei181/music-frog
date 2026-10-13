//! Shared provider facts and copy, independent of any native surface.
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, KernelEtagSupportState, ProviderCacheFingerprint,
    ProviderFingerprintChange, RuleProviderCacheSnapshot, RuleProviderCacheState,
};
use infiltrator_domain::rules::view::{format_content_fingerprint, format_refresh_interval};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn rules_summary(total: usize, providers: usize, language: &str) -> String {
    interpolate(
        Lang(language).tr("rules_observed_summary").as_ref(),
        &[
            ("rules", &total.to_string()),
            ("providers", &providers.to_string()),
        ],
    )
}
pub fn default_action(target: &str, language: &str) -> String {
    interpolate(
        Lang(language).tr("rules_observed_default").as_ref(),
        &[("target", target)],
    )
}
pub fn provider_count(count: usize, behavior: &str, language: &str) -> String {
    interpolate(
        Lang(language).tr("rules_provider_count").as_ref(),
        &[("count", &count.to_string()), ("behavior", behavior)],
    )
}
pub fn published_truncation(omitted: Option<usize>, limit: usize, language: &str) -> String {
    omitted
        .filter(|omitted| *omitted > 0)
        .map(|omitted| {
            interpolate(
                Lang(language).tr("rules_publish_truncated").as_ref(),
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
    lang: &Lang<'_>,
) -> String {
    let updated = interpolate(
        lang.tr("rules_provider_updated_copy").as_ref(),
        &[(
            "time",
            if updated_at.is_empty() {
                "—"
            } else {
                updated_at
            },
        )],
    );
    let undeclared = lang.tr("rules_etag_support_not_declared");
    let source = interpolate(
        lang.tr("rules_provider_source_copy").as_ref(),
        &[(
            "source",
            source_url
                .filter(|url| !url.is_empty())
                .unwrap_or(undeclared.as_ref()),
        )],
    );
    let schedule = match refresh_interval_secs {
        Some(seconds) => interpolate(
            lang.tr("rules_provider_schedule_copy").as_ref(),
            &[("interval", &format_refresh_interval(seconds))],
        ),
        None => interpolate(
            lang.tr("rules_provider_schedule_undeclared").as_ref(),
            &[("value", undeclared.as_ref())],
        ),
    };
    format!("{updated} · {source} · {schedule}")
}
pub fn provider_fingerprint_line(
    observation: &ProviderCacheFingerprint,
    lang: &Lang<'_>,
) -> String {
    let change = lang.tr(match observation.change {
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
        lang.tr("rules_provider_fingerprint_label")
    )
}
pub fn etag_support_line(snapshot: &KernelEtagSupportSnapshot, lang: &Lang<'_>) -> String {
    let state = lang.tr(match snapshot.state {
        KernelEtagSupportState::Enabled => "rules_etag_support_enabled",
        KernelEtagSupportState::Disabled => "rules_etag_support_disabled",
        KernelEtagSupportState::NotDeclared => "rules_etag_support_not_declared",
    });
    format!("{}: {state}", lang.tr("rules_etag_support_label"))
}
pub fn provider_cache_line(cache: &RuleProviderCacheSnapshot, lang: &Lang<'_>) -> String {
    let value = match cache.state {
        RuleProviderCacheState::Ready | RuleProviderCacheState::Empty => interpolate(
            lang.tr("provider_cache_ready").as_ref(),
            &[
                (
                    "dir",
                    cache
                        .directory
                        .as_deref()
                        .unwrap_or(lang.tr("shell_readout_unknown").as_ref()),
                ),
                ("count", &cache.file_count.to_string()),
                ("bytes", &cache.total_bytes.to_string()),
            ],
        ),
        RuleProviderCacheState::Unsupported => lang.tr("provider_cache_unsupported").into_owned(),
        RuleProviderCacheState::Failed => lang.tr("provider_cache_failed").into_owned(),
        RuleProviderCacheState::Unknown => lang.tr("provider_cache_unknown").into_owned(),
    };
    match cache.failure.as_deref().filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("{value}: {reason}"),
        None => value,
    }
}
