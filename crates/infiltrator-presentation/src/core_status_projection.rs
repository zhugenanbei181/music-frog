//! Shared lifecycle and source copy; native products replay the same observed facts.
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::SurfaceOrigin;
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn lifecycle_copy(lifecycle: &CoreLifecycle, language: &str) -> String {
    let key = match lifecycle {
        CoreLifecycle::Stopped => "status_stopped",
        CoreLifecycle::Starting => "status_starting",
        CoreLifecycle::Ready | CoreLifecycle::Running => "status_running",
        CoreLifecycle::Stopping => "core_control_stopping",
        CoreLifecycle::Failed => "status_error",
    };
    Lang(language).tr(key).into_owned()
}

pub fn failure_copy(lifecycle: &CoreLifecycle, reason: Option<&str>, language: &str) -> String {
    match reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        Some(reason) => redact_line(reason, &[]),
        None if *lifecycle == CoreLifecycle::Failed => Lang(language)
            .tr("core_failure_reason_unobserved")
            .into_owned(),
        None => String::new(),
    }
}

pub fn source_copy(origin: SurfaceOrigin, version: Option<&str>, language: &str) -> String {
    let lang = Lang(language);
    if origin == SurfaceOrigin::Demo {
        return lang.tr("core_source_demo").into_owned();
    }
    match version.map(str::trim).filter(|version| !version.is_empty()) {
        Some(version) => interpolate(
            lang.tr("core_source_live_version").as_ref(),
            &[("version", &redact_line(version, &[]))],
        ),
        None => lang.tr("core_source_live_unknown_version").into_owned(),
    }
}

#[cfg(test)]
#[path = "core_status_projection_tests.rs"]
mod tests;
