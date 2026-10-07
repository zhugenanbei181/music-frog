use super::*;
use crate::command_application::CommandApplication;
use crate::dns_observation_projection::DnsObservationTone;
use crate::dns_query_actions::{DnsQueryActions, QuerySection};
use crate::dns_query_fixtures::{IsolatedQueries, QueryFixtureMode};
use crate::dns_query_projection::project_query;
use futures_util::FutureExt;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dns_query::DnsRecordType;
use std::pin::pin;

fn request() -> DnsQueryRequest {
    DnsQueryRequest {
        name: "music.test".into(),
        record_type: DnsRecordType::Txt,
    }
}
#[tokio::test]
async fn real_query_command_preserves_all_sections_and_failure_categories_without_replacing_previous_facts()
 {
    let port = Arc::new(IsolatedQueries::default());
    let owner = DnsQueryApplication::new(Some(port.clone()));
    let command = CommandApplication::new().with_dns_query(owner.clone());
    let intent = |id| CommandIntent::QueryDns {
        operation: DnsQueryOperationId(id),
        request: request(),
    };
    command.execute(intent(1)).await.unwrap();
    let first = owner.snapshot();
    assert_eq!(first.report_id, Some(DnsQueryOperationId(1)));
    let report = first.report.as_ref().unwrap();
    assert_eq!(report.request, request());
    assert_eq!(report.response.answers.len(), 18);
    assert_eq!(report.response.authority[0].data, "ns.test.");
    assert_eq!(report.response.additional[0].ttl, 0);
    assert!(report.response.checking_disabled);
    port.set_mode(QueryFixtureMode::Permission);
    assert_eq!(
        command.execute(intent(2)).await.unwrap_err().code,
        ErrorCode::Permission
    );
    let failure = owner.snapshot();
    assert_eq!(failure.operation, DnsQueryOperation::Failed);
    assert_eq!(failure.report, first.report);
    assert_eq!(failure.report_id, first.report_id);
    assert_eq!(failure.operation_id, Some(DnsQueryOperationId(2)));
    port.set_mode(QueryFixtureMode::MismatchedQuestion);
    assert_eq!(
        command.execute(intent(3)).await.unwrap_err().code,
        ErrorCode::InvalidState
    );
    assert_eq!(owner.snapshot().report, first.report);
    port.set_mode(QueryFixtureMode::NegativeAnswer);
    command.execute(intent(4)).await.unwrap();
    let negative = owner.snapshot().report.unwrap().response;
    assert_eq!(negative.status, 3);
    assert!(negative.answers.is_empty());
    assert_eq!(negative.authority[0].data, "ns.test.");
    let calls = port.requests().len();
    assert_eq!(
        command.execute(intent(4)).await.unwrap_err().code,
        ErrorCode::InvalidState
    );
    let bad = DnsQueryRequest {
        name: "bad..name".into(),
        record_type: DnsRecordType::A,
    };
    assert_eq!(
        owner
            .query(DnsQueryOperationId(5), bad)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(port.requests().len(), calls);
    port.set_mode(QueryFixtureMode::Answer);
    command.execute(intent(5)).await.unwrap();
    assert_eq!(owner.snapshot().report_id, Some(DnsQueryOperationId(5)));
}
#[tokio::test]
async fn concurrent_or_dropped_query_retains_previous_response_and_allows_a_fresh_retry() {
    let port = Arc::new(IsolatedQueries::default());
    let owner = DnsQueryApplication::new(Some(port.clone()));
    owner
        .query(DnsQueryOperationId(1), request())
        .await
        .unwrap();
    let first = owner.snapshot().report;
    port.set_mode(QueryFixtureMode::Pending);
    {
        let mut pending = pin!(owner.query(DnsQueryOperationId(2), request()));
        assert!(pending.as_mut().now_or_never().is_none());
        assert_eq!(owner.snapshot().operation, DnsQueryOperation::Running);
        assert_eq!(
            owner
                .query(DnsQueryOperationId(3), request())
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotReady
        );
        assert_eq!(port.requests().len(), 2);
    }
    let canceled = owner.snapshot();
    assert_eq!(canceled.failure.unwrap().code, ErrorCode::Canceled);
    assert_eq!(canceled.report, first);
    assert_eq!(canceled.report_id, Some(DnsQueryOperationId(1)));
    port.set_mode(QueryFixtureMode::Answer);
    owner
        .query(DnsQueryOperationId(3), request())
        .await
        .unwrap();
    assert_eq!(owner.snapshot().operation, DnsQueryOperation::Completed);
    let absent = DnsQueryApplication::new(None);
    assert_eq!(
        absent
            .query(DnsQueryOperationId(1), request())
            .await
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert!(absent.snapshot().report.is_none());
}
#[tokio::test]
async fn shared_query_draft_cancel_tabs_pagination_retry_and_stale_results_follow_actual_owner_facts()
 {
    let port = Arc::new(IsolatedQueries::default());
    let owner = DnsQueryApplication::new(Some(port.clone()));
    let mut model = DnsQueryActions::default();
    assert!(model.begin().is_err());
    model.show();
    model.set_name(request().name);
    model.set_type(request().record_type);
    assert!(model.cancel());
    assert!(model.begin().is_err());
    assert!(port.requests().is_empty());
    model.show();
    let (token, query) = model.begin().unwrap();
    assert!(model.begin().is_err());
    model.set_name("must-not-change.test".into());
    assert_eq!(model.name, "music.test");
    owner
        .query(DnsQueryOperationId(token), query)
        .await
        .unwrap();
    model.observe(&owner.snapshot());
    assert_eq!(model.pending, Some(token));
    assert!(!model.finish(token + 1, Ok(())));
    assert!(model.finish(token, Ok(())));
    for code in ["zh-CN", "en-US"] {
        let display = project_query(&model, code);
        assert_eq!(display.tone, DnsObservationTone::Success);
        assert!(display.records.contains("observed-0 {ttl}"));
        assert!(!display.records.contains("observed-8"));
    }
    model.next();
    model.next();
    model.next();
    assert_eq!(model.page, 2);
    assert!(
        project_query(&model, "en-US")
            .records
            .contains("observed-17 {ttl}")
    );
    assert!(!project_query(&model, "en-US").next);
    model.select(QuerySection::Authority);
    assert_eq!(model.page, 0);
    assert!(project_query(&model, "en-US").records.contains("ns.test."));
    model.select(QuerySection::Additional);
    assert!(project_query(&model, "en-US").records.contains("TTL 0s"));
    port.set_mode(QueryFixtureMode::Permission);
    let (second, query) = model.begin().unwrap();
    let failure = owner.query(DnsQueryOperationId(second), query).await;
    assert!(model.finish(second, failure));
    model.observe(&owner.snapshot());
    assert!(model.can_retry());
    assert!(
        project_query(&model, "en-US")
            .status
            .ends_with("Allow controller DNS query access {reason}")
    );
    assert!(
        project_query(&model, "en-US")
            .provenance
            .contains("not the new query result")
    );
    let prior_revision = model.snapshot.revision;
    model.observe(&DnsQuerySnapshot::default());
    assert_eq!(model.snapshot.revision, prior_revision);
    let (third, _) = model.begin().unwrap();
    assert!(!model.finish(second, Ok(())));
    assert_eq!(model.pending, Some(third));
}
