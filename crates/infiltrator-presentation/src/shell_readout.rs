//! Shared copy for already-folded shell observations; toolkit layers only replay it.
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::byte_units::format_bytes;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

pub fn observation_copy<T>(
    observation: &ShellObservation<T>,
    catalog: &dyn TextCatalog,
    format: impl FnOnce(&T) -> String,
) -> String {
    match observation {
        ShellObservation::Current(value) => format(value),
        ShellObservation::Stale(value) => interpolate(
            catalog.text("shell_readout_stale").as_ref(),
            &[("value", format(value).as_str())],
        ),
        ShellObservation::Unobserved | ShellObservation::CurrentEmpty => {
            catalog.text("shell_readout_unknown").into_owned()
        }
    }
}
pub fn count_copy(
    snapshot: &ShellReadoutSnapshot,
    page: PageId,
    catalog: &dyn TextCatalog,
) -> String {
    let count = match page {
        PageId::Proxies => &snapshot.proxies,
        PageId::Rules => &snapshot.rules,
        PageId::Connections => &snapshot.connections,
        PageId::Dns => &snapshot.dns,
        _ => return catalog.text("shell_readout_unknown").into_owned(),
    };
    observation_copy(count, catalog, usize::to_string)
}
pub fn rate_copy(rate: &ShellObservation<f64>, catalog: &dyn TextCatalog) -> String {
    observation_copy(rate, catalog, |rate| {
        format!("{}/s", format_bytes(*rate as u64))
    })
}
/// An explicit raw rate observation, used only by legacy transport adapters and fixtures.
pub fn observed_rate(value: Option<f64>) -> ShellObservation<f64> {
    let value = value.filter(|value| value.is_finite() && *value >= 0.0);
    match value {
        Some(value) => ShellObservation::Current(value),
        None => ShellObservation::Unobserved,
    }
}
pub fn rate_status(snapshot: &ShellReadoutSnapshot, catalog: &dyn TextCatalog) -> String {
    snapshot
        .rate_failure
        .as_ref()
        .map(|failure| {
            let reason = redact_line(&failure.message, &[]);
            interpolate(
                catalog.text("shell_rate_read_failed").as_ref(),
                &[("reason", reason.as_str())],
            )
        })
        .unwrap_or_default()
}
pub fn profile_name(snapshot: &ShellReadoutSnapshot, catalog: &dyn TextCatalog) -> String {
    match &snapshot.profile {
        ShellObservation::CurrentEmpty => return catalog.text("no_profiles").into_owned(),
        ShellObservation::Unobserved
        | ShellObservation::Current(_)
        | ShellObservation::Stale(_) => {}
    }
    observation_copy(&snapshot.profile, catalog, |profile| profile.name.clone())
}
pub fn profile_kind(snapshot: &ShellReadoutSnapshot, catalog: &dyn TextCatalog) -> String {
    let key = snapshot
        .profile
        .value()
        .map_or("sidebar_import_hint", |profile| {
            if profile.subscription {
                "sidebar_sub_profile"
            } else {
                "sidebar_local_profile"
            }
        });
    catalog.text(key).into_owned()
}
pub fn profile_usage(snapshot: &ShellReadoutSnapshot, catalog: &dyn TextCatalog) -> String {
    observation_copy(&snapshot.profile, catalog, |profile| {
        match (profile.used_bytes, profile.total_bytes) {
            (Some(used), Some(total)) => {
                format!("{} / {}", format_bytes(used), format_bytes(total))
            }
            _ => catalog.text("shell_readout_unknown").into_owned(),
        }
    })
}
pub fn profile_percent(snapshot: &ShellReadoutSnapshot, catalog: &dyn TextCatalog) -> String {
    observation_copy(&snapshot.profile, catalog, |profile| {
        profile
            .usage_fraction
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned())
    })
}
