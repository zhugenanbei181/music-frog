//! One localized fold for the confirmation and per-target facts on both peers.
use crate::dns_cache_actions::DnsCacheActions;
use crate::text::localize;
use infiltrator_contract::dns_cache::DnsCacheOperation;
use infiltrator_contract::dns_cache::{DnsCacheFlushReport, DnsFlushOutcome};
use lurker_ui_presentation::copy::TextCatalog;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachePresentation {
    pub status: String,
    pub error: bool,
    pub results_label: String,
    pub fake_ip: String,
    pub system: String,
    pub confirm: bool,
    pub retry: bool,
    pub close: bool,
}

pub fn flush_outcome(outcome: &DnsFlushOutcome, catalog: &dyn TextCatalog) -> String {
    match outcome {
        DnsFlushOutcome::NotRequested => catalog.text("dns_flush_not_requested").into_owned(),
        DnsFlushOutcome::Flushed => catalog.text("dns_flush_flushed").into_owned(),
        DnsFlushOutcome::Unsupported { reason } => localize(
            catalog,
            "dns_flush_unsupported_detail",
            &[("reason", reason.clone())],
        ),
        DnsFlushOutcome::Failed { failure } => localize(
            catalog,
            "dns_flush_failed_detail",
            &[("reason", failure.message.clone())],
        ),
    }
}

pub fn flush_summary(report: &DnsCacheFlushReport, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "dns_flush_summary",
        &[
            ("fakeip", flush_outcome(&report.fake_ip, catalog)),
            ("os", flush_outcome(&report.os_cache, catalog)),
        ],
    )
}

pub fn project_cache(state: &DnsCacheActions, catalog: &dyn TextCatalog) -> CachePresentation {
    let requested = state.requested_operation();
    let confirmed = state.is_confirmed();
    let pending = state.is_pending();
    let key = if pending {
        "dns_cache_running"
    } else if !confirmed {
        "dns_cache_confirm_detail"
    } else if requested != state.snapshot.report_id {
        "dns_cache_waiting_report"
    } else {
        match state.snapshot.report.operation() {
            DnsCacheOperation::Idle => "dns_cache_waiting_report",
            DnsCacheOperation::Running => "dns_cache_running",
            DnsCacheOperation::Completed => "dns_cache_completed",
            DnsCacheOperation::Failed => "dns_cache_failed",
            DnsCacheOperation::Unsupported => "dns_cache_unavailable",
        }
    };
    let failure = state.failure().or_else(|| {
        (state.snapshot.operation_id == requested)
            .then_some(state.snapshot.failure.as_ref())
            .flatten()
    });
    let status = if confirmed && let Some(failure) = failure {
        localize(
            catalog,
            "dns_cache_failure_detail",
            &[("reason", failure.message.clone())],
        )
    } else {
        localize(catalog, key, &[])
    };
    CachePresentation {
        status,
        error: confirmed && failure.is_some(),
        results_label: localize(
            catalog,
            if confirmed && requested == state.snapshot.report_id {
                "dns_cache_this_request"
            } else {
                "dns_cache_previous_report"
            },
            &[],
        ),
        fake_ip: localize(
            catalog,
            "dns_cache_fake_ip_result",
            &[(
                "result",
                flush_outcome(&state.snapshot.report.fake_ip, catalog),
            )],
        ),
        system: localize(
            catalog,
            "dns_cache_system_result",
            &[(
                "result",
                flush_outcome(&state.snapshot.report.os_cache, catalog),
            )],
        ),
        confirm: state.is_open() && !confirmed && !pending,
        retry: state.can_retry(),
        close: !pending,
    }
}
