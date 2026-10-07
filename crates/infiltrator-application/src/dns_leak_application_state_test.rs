//! test-intent: behavior
use super::*;
use async_trait::async_trait;
use futures_util::{future, poll};
use infiltrator_contract::dns_leak::{DnsLeakConclusion, DnsLeakEchoReport};
use std::sync::atomic::{AtomicBool, AtomicUsize};
#[derive(Default)]
struct Echo {
    deny: AtomicBool,
    blocked: AtomicBool,
    calls: AtomicUsize,
}
#[async_trait]
impl DnsLeakEchoPort for Echo {
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.blocked.load(Ordering::SeqCst) {
            future::pending::<()>().await;
        }
        if self.deny.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied("allow echo access".into()));
        }
        Ok(DnsLeakEchoReport {
            observations: request
                .probes
                .iter()
                .enumerate()
                .map(|(index, probe)| DnsLeakObservation {
                    resolver: probe.resolver.clone(),
                    authority: probe.authority.clone(),
                    question: probe.question.clone(),
                    transport: DnsLeakProbeTransport::System,
                    outcome: DnsLeakObservationOutcome::Observed {
                        identity: if index == 0 {
                            "203.0.113.9"
                        } else {
                            "198.51.100.7"
                        }
                        .into(),
                    },
                })
                .collect(),
        })
    }
}
#[tokio::test]
async fn failure_and_cancellation_keep_the_real_previous_facts_and_retry_publishes_fresh_results() {
    let echo = Arc::new(Echo::default());
    let application = DnsLeakApplication::new(Some(echo.clone()), default_echo_sources());
    let report = application.probe().await.unwrap();
    assert!(matches!(
        report.conclusion(),
        DnsLeakConclusion::Divergent { .. }
    ));
    echo.deny.store(true, Ordering::SeqCst);
    let failure = Failure::from(application.probe().await.unwrap_err());
    assert_eq!(failure.code, ErrorCode::Permission);
    let failed = application.last_report();
    assert_eq!(failed.observations, report.observations);
    assert_eq!(failed.operation, DnsLeakOperation::Failed { failure });
    echo.deny.store(false, Ordering::SeqCst);
    echo.blocked.store(true, Ordering::SeqCst);
    let mut probe = Box::pin(application.probe());
    assert!(poll!(probe.as_mut()).is_pending());
    assert_eq!(
        application.last_report().operation,
        DnsLeakOperation::Running
    );
    assert_eq!(application.last_report().observations, report.observations);
    drop(probe);
    let DnsLeakOperation::Failed { failure } = application.last_report().operation else {
        panic!("canceled probe")
    };
    assert_eq!(failure.code, ErrorCode::Canceled);
    echo.blocked.store(false, Ordering::SeqCst);
    let fresh = application.probe().await.unwrap();
    assert_eq!(fresh.operation, DnsLeakOperation::Completed);
    assert_eq!(fresh.observations.len(), 2);
    assert_eq!(echo.calls.load(Ordering::SeqCst), 4);
}
