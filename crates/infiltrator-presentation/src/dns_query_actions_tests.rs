//! test-intent: behavior
use crate::dns_query_actions::DnsQueryPhase::Observed;
use crate::dns_query_actions::{DnsQueryActions, QUERY_PAGE_SIZE, QuerySection};
use crate::dns_query_projection::project_query;
use infiltrator_contract::dns_query::{
    DnsQueryOperation, DnsQueryOperationId, DnsQueryReport, DnsQueryRequest, DnsQueryResponse,
    DnsQuerySnapshot, DnsQuestion, DnsRecord, DnsRecordType,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_shared::dns_observation::DnsObservationTone;

fn request() -> DnsQueryRequest {
    DnsQueryRequest {
        name: "music.test".into(),
        record_type: DnsRecordType::Txt,
    }
}

fn response() -> DnsQueryResponse {
    DnsQueryResponse {
        status: 0,
        questions: vec![DnsQuestion {
            name: "music.test.".into(),
            record_type: DnsRecordType::Txt.number(),
            class: 1,
        }],
        truncated: false,
        recursion_desired: true,
        recursion_available: true,
        authenticated_data: false,
        checking_disabled: true,
        answers: (0..18)
            .map(|index| DnsRecord {
                name: format!("observed-{index}.music.test."),
                record_type: DnsRecordType::Txt.number(),
                ttl: 60,
                data: format!("observed-{index} {{ttl}}"),
            })
            .collect(),
        authority: vec![DnsRecord {
            name: "music.test.".into(),
            record_type: DnsRecordType::Ns.number(),
            ttl: 60,
            data: "ns.test.".into(),
        }],
        additional: vec![DnsRecord {
            name: "ns.test.".into(),
            record_type: DnsRecordType::A.number(),
            ttl: 0,
            data: "192.0.2.1".into(),
        }],
    }
}

fn completed(operation: DnsQueryOperationId, revision: u64) -> DnsQuerySnapshot {
    DnsQuerySnapshot {
        revision,
        operation_id: Some(operation),
        report_id: Some(operation),
        operation: DnsQueryOperation::Completed,
        report: Some(DnsQueryReport {
            request: request(),
            response: response(),
        }),
        failure: None,
    }
}

#[test]
fn query_draft_command_tabs_pagination_and_retry_follow_correlated_contract_snapshots() {
    let mut model = DnsQueryActions::default();
    assert!(model.begin().is_err());
    model.show();
    model.set_name(request().name);
    model.set_type(request().record_type);
    assert!(model.cancel());
    assert!(model.begin().is_err());

    model.show();
    let (operation, query) = model.begin().unwrap();
    assert_eq!(query, request());
    assert!(model.begin().is_err());
    model.set_name("must-not-change.test".into());
    assert_eq!(model.name, "music.test");
    model.observe(&completed(operation, 1));
    assert!(model.is_pending());
    assert!(!model.finish(DnsQueryOperationId(operation.0 + 1), Ok(())));
    assert!(model.finish(operation, Ok(())));

    for code in ["zh-CN", "en-US"] {
        let display = project_query(&model, code);
        assert_eq!(display.tone, DnsObservationTone::Success);
        assert!(display.records.contains("observed-0 {ttl}"));
        assert!(!display.records.contains("observed-8 {ttl}"));
    }
    for _ in 0..3 {
        model.next();
    }
    assert_eq!(model.page, 2);
    assert!(
        project_query(&model, "en-US")
            .records
            .contains("observed-17 {ttl}")
    );
    assert!(!project_query(&model, "en-US").next);
    model.next();
    assert_eq!(model.page, 2, "pagination cannot pass the last page");
    model.select(QuerySection::Authority);
    assert_eq!(model.page, 0);
    assert!(project_query(&model, "en-US").records.contains("ns.test."));
    model.select(QuerySection::Additional);
    assert!(project_query(&model, "en-US").records.contains("192.0.2.1"));

    let permission = Failure::new(ErrorCode::Permission, "query permission denied", true);
    let (retry, _) = model.begin().unwrap();
    assert!(model.finish(retry, Err(permission.clone())));
    let mut failed_snapshot = completed(operation, 2);
    failed_snapshot.operation_id = Some(retry);
    failed_snapshot.operation = DnsQueryOperation::Failed;
    failed_snapshot.failure = Some(permission.clone());
    model.observe(&failed_snapshot);
    assert!(model.can_retry());
    let failed_display = project_query(&model, "en-US");
    assert!(failed_display.status.contains("query permission denied"));
    assert!(
        failed_display
            .provenance
            .contains("not the new query result")
    );
    assert_eq!(model.snapshot.report_id, Some(operation));

    let (next, _) = model.begin().unwrap();
    assert!(!model.finish(retry, Ok(())));
    assert_eq!(model.requested_operation(), Some(next));
    assert!(model.is_pending());
}

#[test]
fn readback_before_receipt_is_retained_and_foreign_or_stale_snapshots_are_inert() {
    let mut model = DnsQueryActions::default();
    model.show();
    model.set_name("music.test".into());
    let (operation, _) = model.begin().unwrap();
    let matching = completed(operation, 4);
    model.observe(&matching);
    assert!(model.is_pending());
    assert!(model.finish(operation, Ok(())));
    assert_eq!(
        model.phase(),
        &Observed { operation }
    );

    let before = model.snapshot.clone();
    model.observe(&completed(DnsQueryOperationId(operation.0 + 1), 5));
    assert_eq!(
        model.snapshot, before,
        "foreign result cannot replace the active result"
    );
    model.observe(&completed(operation, 3));
    assert_eq!(
        model.snapshot, before,
        "older revision cannot replace the current result"
    );

    assert_eq!(QUERY_PAGE_SIZE, 8);
    model.page = usize::MAX;
    model.observe(&completed(operation, 6));
    assert_eq!(
        model.page, 2,
        "observed content clamps the page to its last valid page"
    );
}
