//! test-intent: support
use async_trait::async_trait;
use futures_util::StreamExt;
use futures_util::stream::{BoxStream, pending};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamPort, ScopedRuntimeStream, ScopedRuntimeStreamEvent,
};
use lurker_core_contract::identity::CoreScope;
use mem::take;
use std::collections::VecDeque;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::Notify;
use tokio::time::sleep;

pub struct TestRuntime;
impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future);
    }
    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}
#[derive(Clone, Default)]
pub struct LogGateway {
    pub streams: Arc<Mutex<Vec<BoxStream<'static, RuntimeStreamEvent<String>>>>>,
    pub event_scopes: Arc<Mutex<VecDeque<CoreScope>>>,
    pub calls: Arc<AtomicUsize>,
    pub opened: Arc<Notify>,
    pub current_scope: Option<CoreScope>,
    pub requested_scope: Arc<Mutex<Option<CoreScope>>>,
    pub opening_gate: Option<Arc<Notify>>,
    pub failure: Option<PortError>,
}
impl LogGateway {
    async fn open_stream(
        &self,
        level: Option<String>,
    ) -> Result<BoxStream<'static, RuntimeStreamEvent<String>>, PortError> {
        assert_eq!(level.as_deref(), Some("debug"));
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.opened.notify_one();
        if let Some(gate) = &self.opening_gate {
            gate.notified().await;
        }
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let mut streams = self.streams.lock().unwrap();
        if streams.is_empty() {
            Ok(Box::pin(pending()))
        } else {
            Ok(streams.remove(0))
        }
    }
}
#[async_trait]
impl RuntimeStreamPort for LogGateway {
    async fn logs(
        &self,
        scope: CoreScope,
        level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        if self
            .current_scope
            .as_ref()
            .is_some_and(|current| current != &scope)
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.opened.notify_one();
            return Err(PortError::Failed(
                "test gateway scope does not match the request".to_string(),
            ));
        }
        *self.requested_scope.lock().unwrap() = Some(scope.clone());
        let stream = self.open_stream(level).await?;
        let scopes = take(&mut *self.event_scopes.lock().unwrap());
        let mut event_scopes = scopes.into_iter();
        let scoped = stream.map(move |event| ScopedRuntimeStreamEvent {
            scope: event_scopes.next().unwrap_or_else(|| scope.clone()),
            event,
        });
        Ok(Box::pin(scoped))
    }
    async fn traffic(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<TrafficData>, PortError> {
        panic!("unexpected traffic stream")
    }
}
