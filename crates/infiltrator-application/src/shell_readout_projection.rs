//! Shared copy for already-folded shell observations; toolkit layers only replay it.
use crate::byte_format::format_bytes;
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn observation_copy<T>(
    observation: &ShellObservation<T>,
    lang: &str,
    format: impl FnOnce(&T) -> String,
) -> String {
    match &observation.value {
        Some(value) if observation.current => format(value),
        Some(value) => interpolate(
            Lang(lang).tr("shell_readout_stale").as_ref(),
            &[("value", format(value).as_str())],
        ),
        None => Lang(lang).tr("shell_readout_unknown").into_owned(),
    }
}
pub fn count_copy(snapshot: &ShellReadoutSnapshot, page: PageId, lang: &str) -> String {
    let count = match page {
        PageId::Proxies => &snapshot.proxies,
        PageId::Rules => &snapshot.rules,
        PageId::Connections => &snapshot.connections,
        PageId::Dns => &snapshot.dns,
        _ => return Lang(lang).tr("shell_readout_unknown").into_owned(),
    };
    observation_copy(count, lang, usize::to_string)
}
pub fn rate_copy(rate: &ShellObservation<f64>, lang: &str) -> String {
    observation_copy(rate, lang, |rate| {
        format!("{}/s", format_bytes(*rate as u64))
    })
}
/// An explicit raw rate observation, used only by legacy transport adapters and fixtures.
pub fn observed_rate(value: Option<f64>) -> ShellObservation<f64> {
    let value = value.filter(|value| value.is_finite() && *value >= 0.0);
    ShellObservation {
        current: value.is_some(),
        value,
    }
}
pub fn rate_status(snapshot: &ShellReadoutSnapshot, lang: &str) -> String {
    snapshot
        .rate_failure
        .as_ref()
        .map(|failure| {
            let reason = redact_line(&failure.message, &[]);
            interpolate(
                Lang(lang).tr("shell_rate_read_failed").as_ref(),
                &[("reason", reason.as_str())],
            )
        })
        .unwrap_or_default()
}
pub fn profile_name(snapshot: &ShellReadoutSnapshot, lang: &str) -> String {
    if snapshot.profile.current && snapshot.profile.value.is_none() {
        return Lang(lang).tr("no_profiles").into_owned();
    }
    observation_copy(&snapshot.profile, lang, |profile| profile.name.clone())
}
pub fn profile_kind(snapshot: &ShellReadoutSnapshot, lang: &str) -> String {
    let key = snapshot
        .profile
        .value
        .as_ref()
        .map_or("sidebar_import_hint", |profile| {
            if profile.subscription {
                "sidebar_sub_profile"
            } else {
                "sidebar_local_profile"
            }
        });
    Lang(lang).tr(key).into_owned()
}
pub fn profile_usage(snapshot: &ShellReadoutSnapshot, lang: &str) -> String {
    observation_copy(&snapshot.profile, lang, |profile| {
        match (profile.used_bytes, profile.total_bytes) {
            (Some(used), Some(total)) => {
                format!("{} / {}", format_bytes(used), format_bytes(total))
            }
            _ => Lang(lang).tr("shell_readout_unknown").into_owned(),
        }
    })
}
pub fn profile_percent(snapshot: &ShellReadoutSnapshot, lang: &str) -> String {
    observation_copy(&snapshot.profile, lang, |profile| {
        profile
            .usage_fraction
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| Lang(lang).tr("shell_readout_unknown").into_owned())
    })
}
