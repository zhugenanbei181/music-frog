//! Application-owned stream driver with an injected executor and weak lifecycle observation.
use crate::log_application::{
    LogApplication, LogBindingIntent, LogBindingOutcome, LogIngestOutcome,
};
use crate::runtime_stream_application::scope_checked;
use futures_util::future::{AbortHandle, Abortable};
use futures_util::{FutureExt, StreamExt, select};
use infiltrator_contract::logs::LogSession;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamPort, RuntimeStreamTerminal,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::core_scope::CoreScopeSource;
use std::sync::Arc;
use std::thread::{Builder, JoinHandle, current};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
struct LogDriver {
    logs: LogApplication,
    scope_source: Arc<dyn CoreScopeSource>,
    stream_port: Arc<dyn RuntimeStreamPort>,
    runtime: Arc<dyn ApplicationRuntime>,
    observe: Box<dyn Fn() -> Option<CoreLifecycleSnapshot> + Send + Sync>,
}
impl LogDriver {
    fn session(&self) -> Option<LogSession> {
        let core = (self.observe)()?;
        matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        )
        .then(|| core.scope())
        .flatten()
        .and_then(LogSession::from_core_scope)
    }
    async fn wait(&self, session: &LogSession, duration: Duration) {
        let delay = self.runtime.sleep(duration).fuse();
        futures_util::pin_mut!(delay);
        while self.session().as_ref() == Some(session) {
            let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
            futures_util::pin_mut!(tick);
            select! { _ = delay => return, _ = tick => {} }
        }
    }
    async fn run(self) {
        while (self.observe)().is_some() {
            let session = self.session();
            let binding = match session.as_ref() {
                Some(session) => LogBindingIntent::Bind(session.clone()),
                None => LogBindingIntent::Unbind,
            };
            match self.logs.bind(binding) {
                LogBindingOutcome::Bound | LogBindingOutcome::CoreOwnerConfirmed => {}
                LogBindingOutcome::RejectedByCoreOwner => continue,
            }
            let Some(session) = session else {
                self.runtime.sleep(POLL_INTERVAL).await;
                continue;
            };
            let Some(scope) = scope_from_session(&session) else {
                self.logs.fail(
                    session.clone(),
                    Failure::new(
                        ErrorCode::InvalidState,
                        "controller log stream requires a core-scoped session",
                        false,
                    ),
                );
                while self.session().as_ref() == Some(&session) {
                    self.runtime.sleep(POLL_INTERVAL).await;
                }
                continue;
            };
            let opening = self
                .stream_port
                .logs(scope.clone(), Some("debug".into()))
                .fuse();
            futures_util::pin_mut!(opening);
            let opened = loop {
                let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
                futures_util::pin_mut!(tick);
                select! { result = opening => break Some(result), _ = tick => {} }
                if self.session().as_ref() != Some(&session) {
                    break None;
                }
            };
            if self.session().as_ref() != Some(&session) {
                continue;
            }
            let Some(opened) = opened else { continue };
            let mut stream = match opened {
                Ok(stream) => scope_checked(self.scope_source.clone(), scope.clone(), stream),
                Err(error) => {
                    let failure = Failure::from(error);
                    let retryable = failure.retryable;
                    self.logs.fail(session.clone(), failure);
                    if retryable {
                        self.wait(&session, Duration::from_secs(1)).await;
                    } else {
                        while self.session().as_ref() == Some(&session) {
                            self.runtime.sleep(POLL_INTERVAL).await;
                        }
                    }
                    continue;
                }
            };
            let mut retry_disposition = LogRetryDisposition::Reconnect;
            loop {
                let event = stream.next().fuse();
                let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
                futures_util::pin_mut!(event, tick);
                let next = select! { item = event => Some(item), _ = tick => None };
                if self.session().as_ref() != Some(&session) {
                    break;
                }
                match next {
                    Some(Some(event)) => {
                        if event.scope != scope {
                            self.logs.fail(session.clone(), scope_mismatch_failure());
                            retry_disposition = LogRetryDisposition::WaitForSessionChange;
                            break;
                        }
                        retry_disposition = match &event.event {
                            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(
                                failure,
                            )) if !failure.retryable => LogRetryDisposition::WaitForSessionChange,
                            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Unsupported(_)) => {
                                LogRetryDisposition::WaitForSessionChange
                            }
                            _ => retry_disposition,
                        };
                        let terminal = matches!(&event.event, RuntimeStreamEvent::Terminal(_));
                        if self.logs.ingest(session.clone(), event.event)
                            == LogIngestOutcome::RejectedForeignSession
                        {
                            retry_disposition = LogRetryDisposition::WaitForSessionChange;
                            break;
                        }
                        if terminal {
                            break;
                        }
                    }
                    Some(None) => {
                        self.logs.fail(
                            session.clone(),
                            Failure::new(ErrorCode::Network, "controller log stream ended", true),
                        );
                        break;
                    }
                    None => {}
                }
            }
            drop(stream);
            match retry_disposition {
                LogRetryDisposition::Reconnect => {
                    self.wait(&session, Duration::from_millis(100)).await;
                }
                LogRetryDisposition::WaitForSessionChange => {
                    while self.session().as_ref() == Some(&session) {
                        self.runtime.sleep(POLL_INTERVAL).await;
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum LogRetryDisposition {
    Reconnect,
    WaitForSessionChange,
}

fn scope_from_session(session: &LogSession) -> Option<CoreScope> {
    Some(CoreScope {
        identity: session.identity()?.clone(),
        generation: session.generation,
        session_token: Some(session.token),
    })
}

fn scope_mismatch_failure() -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        "controller log stream returned an event from a different core scope",
        false,
    )
}

pub struct LogPump {
    abort: AbortHandle,
    worker: Option<JoinHandle<()>>,
}
impl LogPump {
    pub fn spawn(
        logs: LogApplication,
        scope_source: Arc<dyn CoreScopeSource>,
        stream_port: Arc<dyn RuntimeStreamPort>,
        runtime: Arc<dyn ApplicationRuntime>,
        observe: impl Fn() -> Option<CoreLifecycleSnapshot> + Send + Sync + 'static,
    ) -> Result<Self, Failure> {
        let (abort, registration) = AbortHandle::new_pair();
        let driver = LogDriver {
            logs,
            scope_source,
            stream_port,
            runtime: runtime.clone(),
            observe: Box::new(observe),
        };
        let worker = Builder::new()
            .name("musicfrog-logs".into())
            .spawn(move || {
                runtime.block_on(Box::pin(async move {
                    let _ = Abortable::new(driver.run(), registration).await;
                }));
            })
            .map_err(|error| Failure::new(ErrorCode::Internal, error.to_string(), true))?;
        Ok(Self {
            abort,
            worker: Some(worker),
        })
    }
}
impl Drop for LogPump {
    fn drop(&mut self) {
        self.abort.abort();
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != current().id()
        {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "log_stream_tests.rs"]
mod tests;
