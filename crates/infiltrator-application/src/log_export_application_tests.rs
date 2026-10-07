//! Source-bound prepare/confirm/cancel over the actual shared log buffer.
use super::LogExportApplication;
use crate::log_application::LogApplication;
use async_trait::async_trait;
use futures_util::{FutureExt, poll};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::session::SessionToken;
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::log_export::LogExportPort;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::future::pending;
use std::sync::{Arc, Mutex};
use std::task::Poll;

#[derive(Default)]
struct Writer {
    writes: Mutex<Vec<String>>,
    failure: Mutex<Option<Failure>>,
    corrupt: Mutex<bool>,
}
#[async_trait]
impl LogExportPort for Writer {
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        if let Some(failure) = self.failure.lock().unwrap().clone() {
            return Err(PortError::Rejected(failure));
        }
        self.writes.lock().unwrap().push(artifact.content.clone());
        Ok(LogExportReceipt {
            bytes_written: artifact.content.len() + usize::from(*self.corrupt.lock().unwrap()),
            path: "/actual-host/export.log".into(),
            summary: artifact.summary,
        })
    }
}
fn observed() -> (LogApplication, LogSession) {
    let logs = LogApplication::default();
    let session = LogSession {
        generation: 1,
        token: SessionToken::new(41),
    };
    logs.bind(Some(session));
    assert!(logs.ingest(session, RuntimeStreamEvent::Connected));
    assert!(logs.ingest(
        session,
        RuntimeStreamEvent::Item("WARN[1] token=credential".into())
    ));
    assert!(logs.ingest(
        session,
        RuntimeStreamEvent::Item("INFO[2] café healthy".into())
    ));
    (logs, session)
}
#[tokio::test]
async fn cancel_is_effect_free_and_confirmation_freezes_all_records_despite_filter_and_growth() {
    let (logs, session) = observed();
    logs.set_filter(Some("WARN".into())).unwrap();
    let writer = Arc::new(Writer::default());
    let application = LogExportApplication::new(logs.clone(), Some(writer.clone()), vec![]);
    let cancelled = application.prepare().await.unwrap();
    assert!(writer.writes.lock().unwrap().is_empty());
    application.cancel(&cancelled.identity).unwrap();
    assert_eq!(
        application
            .save(&cancelled.identity)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert!(writer.writes.lock().unwrap().is_empty());
    let summary = application.prepare().await.unwrap();
    assert_eq!(summary.records, 2);
    assert!(logs.ingest(
        session,
        RuntimeStreamEvent::Item("DEBUG[3] arrived after confirmation".into())
    ));
    let receipt = application.save(&summary.identity).await.unwrap();
    assert_eq!(receipt.summary, summary);
    assert_eq!(receipt.bytes_written, summary.bytes);
    assert_eq!(
        *writer.writes.lock().unwrap(),
        vec!["WARN[1] token=***\nINFO[2] café healthy\n"]
    );
    assert_eq!(application.save(&summary.identity).await.unwrap(), receipt);
    assert_eq!(writer.writes.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn failure_and_wrong_receipt_do_not_become_success_and_retry_keeps_the_prepared_source() {
    let (logs, _) = observed();
    let writer = Arc::new(Writer::default());
    let denied = Failure::new(ErrorCode::Permission, "export directory denied", false);
    *writer.failure.lock().unwrap() = Some(denied.clone());
    let application = LogExportApplication::new(logs, Some(writer.clone()), vec![]);
    let summary = application.prepare().await.unwrap();
    assert_eq!(application.save(&summary.identity).await, Err(denied));
    assert!(writer.writes.lock().unwrap().is_empty());
    *writer.failure.lock().unwrap() = None;
    *writer.corrupt.lock().unwrap() = true;
    assert_eq!(
        application.save(&summary.identity).await.unwrap_err().code,
        ErrorCode::InvalidState
    );
    *writer.corrupt.lock().unwrap() = false;
    assert_eq!(
        application.save(&summary.identity).await.unwrap().summary,
        summary
    );
}
#[tokio::test]
async fn superseded_confirmation_and_changed_controller_session_cannot_write() {
    let (logs, _) = observed();
    let writer = Arc::new(Writer::default());
    let application = LogExportApplication::new(logs.clone(), Some(writer.clone()), vec![]);
    let old = application.prepare().await.unwrap();
    let latest = application.prepare().await.unwrap();
    assert_eq!(
        application.save(&old.identity).await.unwrap_err().code,
        ErrorCode::InvalidState
    );
    logs.bind(Some(LogSession {
        generation: 2,
        token: SessionToken::new(42),
    }));
    assert!(application.save(&latest.identity).await.is_err());
    assert!(writer.writes.lock().unwrap().is_empty());
}
#[tokio::test]
async fn missing_host_is_typed_unsupported_and_observed_empty_differs_from_unobserved() {
    let logs = LogApplication::default();
    let application = LogExportApplication::new(logs.clone(), None, vec![]);
    assert_eq!(
        application.prepare().await.unwrap_err().code,
        ErrorCode::NotReady
    );
    let session = LogSession {
        generation: 1,
        token: SessionToken::new(41),
    };
    logs.bind(Some(session));
    assert!(logs.ingest(session, RuntimeStreamEvent::Connected));
    let summary = application.prepare().await.unwrap();
    assert_eq!(summary.records, 0);
    assert_eq!(summary.bytes, 0);
    assert_eq!(
        application.save(&summary.identity).await.unwrap_err().code,
        ErrorCode::Unsupported
    );
    application.cancel(&summary.identity).unwrap();
}
struct PendingWriter;
#[async_trait]
impl LogExportPort for PendingWriter {
    async fn save(&self, _: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        pending().await
    }
}
#[tokio::test]
async fn pending_save_rejects_cancel_prepare_and_duplicate_and_dropped_future_releases_the_guard() {
    let (logs, _) = observed();
    let application = LogExportApplication::new(logs, Some(Arc::new(PendingWriter)), vec![]);
    let summary = application.prepare().await.unwrap();
    let mut saving = application.save(&summary.identity).boxed();
    assert!(matches!(poll!(&mut saving), Poll::Pending));
    assert_eq!(
        application.cancel(&summary.identity).unwrap_err().code,
        ErrorCode::NotReady
    );
    assert_eq!(
        application.prepare().await.unwrap_err().code,
        ErrorCode::NotReady
    );
    assert_eq!(
        application.save(&summary.identity).await.unwrap_err().code,
        ErrorCode::NotReady
    );
    drop(saving);
    application.cancel(&summary.identity).unwrap();
    assert!(application.prepare().await.is_ok());
}

struct DynamicEndpoint(Mutex<Result<ControllerEndpoint, PortError>>);
#[async_trait]
impl EndpointSource for DynamicEndpoint {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        self.0.lock().unwrap().clone()
    }
}
#[tokio::test]
async fn preparation_resolves_current_controller_credentials_and_preserves_source_failures_without_writing()
 {
    let (logs, session) = observed();
    let source = Arc::new(DynamicEndpoint(Mutex::new(Ok(ControllerEndpoint {
        url: "http://localhost:9090".into(),
        secret: Some("old-bare-credential".into()),
    }))));
    let writer = Arc::new(Writer::default());
    let application = LogExportApplication::new(logs.clone(), Some(writer.clone()), vec![])
        .with_endpoint_source(source.clone());
    *source.0.lock().unwrap() = Ok(ControllerEndpoint {
        url: "http://localhost:9090".into(),
        secret: Some("rotated-bare-credential".into()),
    });
    assert!(logs.ingest(
        session,
        RuntimeStreamEvent::Item("DEBUG[3] rotated-bare-credential".into())
    ));
    let summary = application.prepare().await.unwrap();
    application.save(&summary.identity).await.unwrap();
    assert!(writer.writes.lock().unwrap()[0].contains("DEBUG[3] ***\n"));
    assert!(!writer.writes.lock().unwrap()[0].contains("rotated-bare-credential"));
    let denied = PortError::PermissionDenied("credential store unavailable".into());
    *source.0.lock().unwrap() = Err(denied.clone());
    assert_eq!(application.prepare().await, Err(Failure::from(denied)));
    assert_eq!(writer.writes.lock().unwrap().len(), 1);
}
