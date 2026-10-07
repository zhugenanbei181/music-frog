//! One localized fold for the confirmation and per-target facts on both peers.
use crate::dns_cache_actions::DnsCacheActions;
use crate::dns_status_projection::flush_outcome;
use infiltrator_contract::dns_cache::DnsCacheOperation;
use infiltrator_shared::i18n_interpolator::localize;
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
pub fn project_cache(state: &DnsCacheActions, code: &str) -> CachePresentation {
    let key = if state.pending.is_some() {
        "dns_cache_running"
    } else if !state.confirmed {
        "dns_cache_confirm_detail"
    } else if state.requested != state.snapshot.report_id {
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
    let failure = state.failure.as_ref().or_else(|| {
        (state.snapshot.operation_id == state.requested)
            .then_some(state.snapshot.failure.as_ref())
            .flatten()
    });
    let status = if state.confirmed
        && let Some(failure) = failure
    {
        localize(
            code,
            "dns_cache_failure_detail",
            &[("reason", failure.message.clone())],
        )
    } else {
        localize(code, key, &[])
    };
    CachePresentation {
        status,
        error: state.confirmed && failure.is_some(),
        results_label: localize(
            code,
            if state.confirmed && state.requested == state.snapshot.report_id {
                "dns_cache_this_request"
            } else {
                "dns_cache_previous_report"
            },
            &[],
        ),
        fake_ip: localize(
            code,
            "dns_cache_fake_ip_result",
            &[(
                "result",
                flush_outcome(&state.snapshot.report.fake_ip, code),
            )],
        ),
        system: localize(
            code,
            "dns_cache_system_result",
            &[(
                "result",
                flush_outcome(&state.snapshot.report.os_cache, code),
            )],
        ),
        confirm: state.open && !state.confirmed && state.pending.is_none(),
        retry: state.can_retry(),
        close: state.pending.is_none(),
    }
}
