//! One presentation of observed profile traffic and update schedules for peer products.
use crate::subscription_status_projection::schedule_status;
use crate::text::localize;
use infiltrator_shared::byte_units::format_bytes;
use lurker_ui_presentation::copy::TextCatalog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageGrade {
    Unknown,
    Normal,
    Warning,
    High,
    Exhausted,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficProjection {
    pub upload: String,
    pub download: String,
    pub used: String,
    pub total: String,
    pub fraction: Option<f32>,
    pub grade: UsageGrade,
}
pub fn traffic(
    upload: Option<u64>,
    download: Option<u64>,
    total: Option<u64>,
) -> TrafficProjection {
    let used = upload
        .zip(download)
        .and_then(|(upload, download)| upload.checked_add(download));
    let fraction = used
        .zip(total)
        .filter(|(_, total)| *total > 0)
        .map(|(used, total)| (used as f32 / total as f32).clamp(0.0, 1.0));
    let grade = match fraction {
        None => UsageGrade::Unknown,
        Some(value) if value >= 1.0 => UsageGrade::Exhausted,
        Some(value) if value >= 0.9 => UsageGrade::High,
        Some(value) if value >= 0.8 => UsageGrade::Warning,
        Some(_) => UsageGrade::Normal,
    };
    let display = |value: Option<u64>| value.map(format_bytes).unwrap_or_else(|| "—".into());
    TrafficProjection {
        upload: display(upload),
        download: display(download),
        used: display(used),
        total: display(total),
        fraction,
        grade,
    }
}
pub fn traffic_caption(facts: &TrafficProjection, catalog: &dyn TextCatalog) -> String {
    let percent = facts
        .fraction
        .map(|value| format!(" ({:.1}%)", value * 100.0))
        .unwrap_or_default();
    localize(
        catalog,
        "profile_observed_traffic",
        &[
            ("upload", facts.upload.clone()),
            ("download", facts.download.clone()),
            ("used", facts.used.clone()),
            ("total", facts.total.clone()),
            ("percent", percent),
        ],
    )
}
pub fn active_name(value: Option<&str>, catalog: &dyn TextCatalog) -> String {
    value
        .map(str::to_owned)
        .unwrap_or_else(|| catalog.text("profile_no_active").into_owned())
}
pub fn summary(count: usize, active: Option<&str>, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "profile_list_summary",
        &[
            ("count", count.to_string()),
            ("active", active_name(active, catalog)),
        ],
    )
}
pub fn updated(value: &str, catalog: &dyn TextCatalog) -> String {
    localize(catalog, "profile_updated_time", &[("time", value.into())])
}
pub fn active_status(active: bool, catalog: &dyn TextCatalog) -> String {
    catalog
        .text(if active {
            "profile_status_active"
        } else {
            "profile_status_activate"
        })
        .into_owned()
}
pub fn schedule(
    enabled: bool,
    cron: Option<&str>,
    hours: Option<u32>,
    next: Option<&str>,
    catalog: &dyn TextCatalog,
) -> String {
    let cadence = schedule_status(Some(enabled), cron, hours).render(catalog);
    match next.filter(|next| !next.trim().is_empty()) {
        Some(next) => localize(
            catalog,
            "profile_schedule_next",
            &[("cadence", cadence), ("next", next.into())],
        ),
        None => cadence,
    }
}
pub fn schedule_overview<'a>(
    values: impl IntoIterator<Item = (bool, Option<&'a str>, Option<u32>)>,
    catalog: &dyn TextCatalog,
) -> String {
    let enabled = values
        .into_iter()
        .filter(|(enabled, _, _)| *enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return catalog.text("profile_schedule_disabled").into_owned();
    }
    let interval = enabled
        .iter()
        .filter_map(|(_, _, hours)| *hours)
        .filter(|hours| *hours > 0)
        .min();
    let missing = enabled
        .iter()
        .filter(|(_, cron, hours)| {
            hours.is_none() && cron.is_none_or(|cron| cron.trim().is_empty())
        })
        .count();
    let key = if missing > 0 {
        "profile_schedule_missing"
    } else if interval.is_some() {
        "profile_schedule_interval_summary"
    } else {
        "profile_schedule_cron_summary"
    };
    localize(
        catalog,
        key,
        &[
            ("count", enabled.len().to_string()),
            ("missing", missing.to_string()),
            (
                "hours",
                interval
                    .map(|hours| hours.to_string())
                    .unwrap_or_else(|| "—".into()),
            ),
        ],
    )
}

#[cfg(test)]
#[path = "profile_metadata_projection_tests.rs"]
mod tests;
