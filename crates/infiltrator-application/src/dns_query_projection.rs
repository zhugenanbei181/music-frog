//! One fold of the full query response, shared window and original record data.
use crate::dns_observation_projection::DnsObservationTone;
use crate::dns_query_actions::{DnsQueryActions, QUERY_PAGE_SIZE};
use infiltrator_contract::error::ErrorCode;
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsQueryDisplay {
    pub status: String,
    pub tone: DnsObservationTone,
    pub provenance: String,
    pub question: String,
    pub flags: String,
    pub records: String,
    pub counter: String,
    pub previous: bool,
    pub next: bool,
    pub run: bool,
    pub retry: bool,
    pub close: bool,
    pub guide: bool,
}
pub fn project_query(state: &DnsQueryActions, code: &str) -> DnsQueryDisplay {
    let failure = state.current_failure();
    let (status, tone) = if let Some(failure) = failure {
        (
            localize(
                code,
                "dns_query_failure",
                &[("reason", failure.message.clone())],
            ),
            if failure.code == ErrorCode::Unsupported {
                DnsObservationTone::Neutral
            } else {
                DnsObservationTone::Danger
            },
        )
    } else if state.pending.is_some() {
        (
            localize(code, "dns_query_running", &[]),
            DnsObservationTone::Neutral,
        )
    } else if state.requested.is_some() && state.requested != state.snapshot.report_id {
        (
            localize(code, "dns_query_waiting", &[]),
            DnsObservationTone::Neutral,
        )
    } else if let Some(report) = &state.snapshot.report {
        let response = &report.response;
        (
            localize(
                code,
                "dns_query_response_status",
                &[("status", response.status.to_string())],
            ),
            if response.status == 0 && !response.truncated {
                DnsObservationTone::Success
            } else {
                DnsObservationTone::Warning
            },
        )
    } else {
        (
            localize(code, "dns_query_not_observed", &[]),
            DnsObservationTone::Neutral,
        )
    };
    let records = state.rows();
    let offset = state
        .page
        .saturating_mul(QUERY_PAGE_SIZE)
        .min(records.len());
    let rows: Vec<_> = records
        .iter()
        .skip(offset)
        .take(QUERY_PAGE_SIZE)
        .map(|row| {
            localize(
                code,
                "dns_query_record",
                &[
                    ("name", row.name.clone()),
                    ("type", row.record_type.to_string()),
                    ("ttl", row.ttl.to_string()),
                    ("data", row.data.clone()),
                ],
            )
        })
        .collect();
    let (question, flags) = state
        .snapshot
        .report
        .as_ref()
        .map(|report| {
            let response = &report.response;
            (
                localize(
                    code,
                    "dns_query_question",
                    &[
                        ("submitted", report.request.name.clone()),
                        ("name", response.questions[0].name.clone()),
                        ("type", response.questions[0].record_type.to_string()),
                        ("class", response.questions[0].class.to_string()),
                    ],
                ),
                format!(
                    "TC={} RD={} RA={} AD={} CD={}",
                    response.truncated,
                    response.recursion_desired,
                    response.recursion_available,
                    response.authenticated_data,
                    response.checking_disabled
                ),
            )
        })
        .unwrap_or_default();
    DnsQueryDisplay {
        status,
        tone,
        provenance: localize(
            code,
            if state.requested.is_some() && state.requested == state.snapshot.report_id {
                "dns_query_this_request"
            } else {
                "dns_query_previous_result"
            },
            &[],
        ),
        question,
        flags,
        records: if rows.is_empty() {
            localize(code, "dns_query_section_empty", &[])
        } else {
            rows.join("\n")
        },
        counter: localize(
            code,
            "dns_query_page",
            &[
                ("page", (state.page + 1).to_string()),
                (
                    "pages",
                    (records.len().saturating_sub(1) / QUERY_PAGE_SIZE + 1).to_string(),
                ),
                ("total", records.len().to_string()),
            ],
        ),
        previous: state.page > 0,
        next: offset + rows.len() < records.len(),
        run: state.open && state.pending.is_none(),
        retry: state.can_retry(),
        close: state.pending.is_none(),
        guide: state.can_guide(),
    }
}
