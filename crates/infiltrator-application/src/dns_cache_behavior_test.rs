//! test-intent: behavior
use super::*;
use crate::command_application::handler::{CommandHandler, ValidatedCoreCommandScope};
use crate::dns_cache_actions::DnsCacheActions;
use crate::dns_cache_actions::allocate_operation;
use crate::dns_cache_fixtures::{CachePortMode, IsolatedCaches};
use crate::dns_cache_projection::project_cache;
use futures_util::{FutureExt, pin_mut};
use infiltrator_contract::command::CommandIntent;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn missing_core_capability_refuses_before_controller_or_host_cache_side_effects() {
    let ports = Arc::new(IsolatedCaches::default());
    let owner = DnsCacheApplication::new(Some(ports.clone()), Some(ports.clone()));
    let application = crate::command_application::CommandApplication::new().with_dns_cache(owner);

    let failure = application
        .handle_output_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(
                crate::dns_cache_fixtures::test_core_scope(),
            )),
            CommandIntent::ClearDnsCache {
                operation: allocate_operation().unwrap(),
            },
        )
        .await
        .expect_err("missing engine capability denies the flush before dispatch");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        ports.contents(),
        (
            vec!["cached.example.test".into()],
            vec!["host.example.test".into()]
        )
    );
}

#[tokio::test]
async fn actual_targets_preserve_permission_identity_publish_partial_facts_retry_and_refuse_replayed_operations()
 {
    let ports = Arc::new(IsolatedCaches::default());
    let owner = DnsCacheApplication::new(Some(ports.clone()), Some(ports.clone()));
    assert_eq!(owner.snapshot().operation, DnsCacheOperation::Idle);
    let original = ports.contents();
    ports.set_system_mode(CachePortMode::Denied);
    let operation = allocate_operation().unwrap();
    let failure = owner
        .flush_with_id(crate::dns_cache_fixtures::test_core_scope(), operation)
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    assert!(!failure.retryable);
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.report_id, Some(operation));
    assert_eq!(snapshot.operation, DnsCacheOperation::Failed);
    assert_eq!(snapshot.report.fake_ip, DnsFlushOutcome::Flushed);
    assert_eq!(
        snapshot.report.os_cache,
        DnsFlushOutcome::Failed {
            failure: failure.clone()
        }
    );
    assert!(ports.contents().0.is_empty());
    assert_eq!(ports.contents().1, original.1);
    assert_eq!(
        owner
            .flush_with_id(crate::dns_cache_fixtures::test_core_scope(), operation)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 1);
    ports.set_system_mode(CachePortMode::Allowed);
    let retry = allocate_operation().unwrap();
    let report = owner
        .flush_with_id(crate::dns_cache_fixtures::test_core_scope(), retry)
        .await
        .unwrap();
    assert_eq!(report.fake_ip, DnsFlushOutcome::Flushed);
    assert_eq!(report.os_cache, DnsFlushOutcome::Flushed);
    assert!(ports.contents().0.is_empty() && ports.contents().1.is_empty());
    assert_eq!(owner.snapshot().operation, DnsCacheOperation::Completed);
    assert_eq!(owner.snapshot().report_id, Some(retry));
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn concurrent_or_dropped_flush_cannot_claim_completion_or_lose_previous_observations_and_retry_recovers()
 {
    let ports = Arc::new(IsolatedCaches::default());
    let owner = DnsCacheApplication::new(Some(ports.clone()), Some(ports.clone()));
    let previous = owner
        .flush_with_id(
            crate::dns_cache_fixtures::test_core_scope(),
            allocate_operation().unwrap(),
        )
        .await
        .unwrap();
    let previous_id = owner.snapshot().report_id;
    ports.set_fake_mode(CachePortMode::Pending);
    let operation = allocate_operation().unwrap();
    {
        let pending = owner.flush_with_id(crate::dns_cache_fixtures::test_core_scope(), operation);
        pin_mut!(pending);
        assert!(pending.as_mut().now_or_never().is_none());
        assert_eq!(owner.snapshot().operation, DnsCacheOperation::Running);
        assert_eq!(owner.snapshot().report, previous);
        assert_eq!(
            owner
                .flush_with_id(
                    crate::dns_cache_fixtures::test_core_scope(),
                    allocate_operation().unwrap(),
                )
                .await
                .unwrap_err()
                .code,
            ErrorCode::NotReady
        );
    }
    assert_eq!(owner.snapshot().operation, DnsCacheOperation::Failed);
    assert_eq!(owner.snapshot().failure.unwrap().code, ErrorCode::Canceled);
    assert_eq!(owner.snapshot().report, previous);
    assert_eq!(owner.snapshot().report_id, previous_id);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 1);
    ports.set_fake_mode(CachePortMode::Allowed);
    assert!(
        owner
            .flush_with_id(
                crate::dns_cache_fixtures::test_core_scope(),
                allocate_operation().unwrap(),
            )
            .await
            .unwrap()
            .fake_ip
            .is_flushed()
    );
    assert_eq!(owner.snapshot().operation, DnsCacheOperation::Completed);
}
#[test]
fn other_operation_or_stale_report_cannot_complete_confirmation_and_permission_copy_is_single_pass()
{
    let mut model = DnsCacheActions::default();
    model.show();
    let token = model.confirm().unwrap();
    let before = model.snapshot.clone();
    model.observe(&DnsCacheSnapshot {
        revision: 20,
        operation_id: Some(DnsCacheOperationId(token + 1)),
        report_id: Some(DnsCacheOperationId(token + 1)),
        operation: DnsCacheOperation::Completed,
        ..Default::default()
    });
    assert_eq!(model.snapshot, before);
    assert!(model.finish(token, Ok(())));
    assert!(
        project_cache(&model, "en-US")
            .status
            .contains("waiting for the shared report")
    );
    let snapshot = DnsCacheSnapshot {
        revision: 22,
        operation_id: Some(DnsCacheOperationId(token)),
        report_id: Some(DnsCacheOperationId(token)),
        operation: DnsCacheOperation::Completed,
        report: DnsCacheFlushReport {
            fake_ip: DnsFlushOutcome::Flushed,
            os_cache: DnsFlushOutcome::Flushed,
        },
        ..Default::default()
    };
    model.observe(&snapshot);
    assert!(
        project_cache(&model, "en-US")
            .status
            .contains("Clearing finished")
    );
    let other = DnsCacheSnapshot {
        revision: 24,
        operation: DnsCacheOperation::Failed,
        operation_id: Some(DnsCacheOperationId(token + 1)),
        failure: Some(Failure::new(
            ErrorCode::Canceled,
            "other operation canceled",
            true,
        )),
        ..snapshot.clone()
    };
    model.observe(&other);
    assert!(
        project_cache(&model, "en-US")
            .status
            .contains("Clearing finished")
    );
    model.show();
    let token = model.confirm().unwrap();
    model.finish(
        token,
        Err(Failure::new(
            ErrorCode::Permission,
            "allow {reason} verbatim",
            false,
        )),
    );
    let view = project_cache(&model, "zh-CN");
    assert!(view.retry);
    assert!(view.status.contains("allow {reason} verbatim"));
    assert!(model.cancel());
    assert!(!model.open);
}
