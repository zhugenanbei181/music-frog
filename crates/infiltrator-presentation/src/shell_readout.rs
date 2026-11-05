//! Shared copy for already-folded shell observations; toolkit layers only replay it.
use infiltrator_contract::shell_readout::{ShellObservation, ShellReadoutSnapshot};
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_shared::byte_units::format_bytes;
use lurker_core_contract::redact::redact_line;
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

/// A compact, non-sentence representation of one observed shell quantity.
///
/// Surfaces use this for navigation chips and rate footers. An unavailable
/// observation remains visibly unavailable; it never becomes `0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellQuantityBadge {
    /// The value belongs to the accepted snapshot.
    Current(String),
    /// The last accepted value, retained after the reader became unavailable.
    Stale(String),
    /// No value has been observed.
    Unobserved,
}

fn quantity_badge<T>(
    observation: &ShellObservation<T>,
    format: impl FnOnce(&T) -> String,
) -> ShellQuantityBadge {
    match observation {
        ShellObservation::Current(value) => ShellQuantityBadge::Current(format(value)),
        ShellObservation::Stale(value) => ShellQuantityBadge::Stale(format(value)),
        ShellObservation::Unobserved | ShellObservation::CurrentEmpty => {
            ShellQuantityBadge::Unobserved
        }
    }
}

pub fn count_badge(snapshot: &ShellReadoutSnapshot, page: PageId) -> ShellQuantityBadge {
    match page {
        PageId::Proxies => quantity_badge(&snapshot.proxies, usize::to_string),
        PageId::Rules => quantity_badge(&snapshot.rules, usize::to_string),
        PageId::Connections => quantity_badge(&snapshot.connections, usize::to_string),
        PageId::Dns => quantity_badge(&snapshot.dns, usize::to_string),
        _ => ShellQuantityBadge::Unobserved,
    }
}

pub fn rate_badge(rate: &ShellObservation<f64>) -> ShellQuantityBadge {
    quantity_badge(rate, |rate| format!("{}/s", format_bytes(*rate as u64)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_badges_keep_unobserved_distinct_from_zero() {
        let mut snapshot = ShellReadoutSnapshot::default();
        assert_eq!(
            count_badge(&snapshot, PageId::Rules),
            ShellQuantityBadge::Unobserved
        );
        assert_eq!(
            rate_badge(&snapshot.upload_bps),
            ShellQuantityBadge::Unobserved
        );

        snapshot.rules = ShellObservation::Current(0);
        snapshot.upload_bps = ShellObservation::Current(0.0);
        assert_eq!(
            count_badge(&snapshot, PageId::Rules),
            ShellQuantityBadge::Current("0".into())
        );
        assert_eq!(
            rate_badge(&snapshot.upload_bps),
            ShellQuantityBadge::Current("0 B/s".into())
        );

        snapshot.rules = ShellObservation::Stale(7);
        snapshot.download_bps = ShellObservation::Stale(1024.0);
        assert_eq!(
            count_badge(&snapshot, PageId::Rules),
            ShellQuantityBadge::Stale("7".into())
        );
        assert_eq!(
            rate_badge(&snapshot.download_bps),
            ShellQuantityBadge::Stale("1.00 KB/s".into())
        );
    }
}
