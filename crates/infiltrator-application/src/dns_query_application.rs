//! Serialize queries and publish correlated facts independently of DNS configuration reads.
use futures_util::lock;
use infiltrator_contract::dns_query::{
    DnsQueryOperation, DnsQueryOperationId, DnsQueryReport, DnsQueryRequest, DnsQuerySnapshot,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::dns_query::DnsQueryPort;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DnsQueryApplication {
    port: Option<Arc<dyn DnsQueryPort>>,
    state: Arc<Mutex<DnsQuerySnapshot>>,
    admission: Arc<lock::Mutex<()>>,
}
impl DnsQueryApplication {
    pub fn new(port: Option<Arc<dyn DnsQueryPort>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(if port.is_some() {
                DnsQuerySnapshot::default()
            } else {
                DnsQuerySnapshot::unavailable()
            })),
            port,
            admission: Arc::new(lock::Mutex::new(())),
        }
    }
    pub fn snapshot(&self) -> DnsQuerySnapshot {
        self.state.lock().expect("DNS query snapshot lock").clone()
    }
    pub async fn query(
        &self,
        operation: DnsQueryOperationId,
        request: DnsQueryRequest,
    ) -> Result<(), Failure> {
        request.validate()?;
        let _admission = self.admission.try_lock().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Another DNS query is already running",
                true,
            )
        })?;
        {
            let mut state = self.state.lock().expect("DNS query snapshot lock");
            if operation.0 == 0
                || state
                    .operation_id
                    .is_some_and(|previous| previous.0 >= operation.0)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "DNS query operation identity is stale",
                    false,
                ));
            }
            state.revision.checked_add(2).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS query revision exhausted",
                    false,
                )
            })?;
            state.revision += 1;
            state.operation_id = Some(operation);
            state.operation = DnsQueryOperation::Running;
            state.failure = None;
        }
        let mut run = QueryRun {
            state: self.state.clone(),
            finished: false,
        };
        let result = match &self.port {
            None => Err(Failure::unsupported(
                "The host has no controller DNS query adapter",
            )),
            Some(port) => match port.query(&request).await {
                Ok(response) => response
                    .validate_for(&request)
                    .map(|()| DnsQueryReport { request, response }),
                Err(error) => Err(Failure::from(error)),
            },
        };
        let mut state = self.state.lock().expect("DNS query snapshot lock");
        state.revision += 1;
        let terminal = match result {
            Ok(report) => {
                state.report = Some(report);
                state.report_id = Some(operation);
                state.operation = DnsQueryOperation::Completed;
                Ok(())
            }
            Err(failure) => {
                state.operation = if failure.code == ErrorCode::Unsupported {
                    DnsQueryOperation::Unsupported
                } else {
                    DnsQueryOperation::Failed
                };
                state.failure = Some(failure.clone());
                Err(failure)
            }
        };
        run.finished = true;
        terminal
    }
}
struct QueryRun {
    state: Arc<Mutex<DnsQuerySnapshot>>,
    finished: bool,
}
impl Drop for QueryRun {
    fn drop(&mut self) {
        if !self.finished {
            let mut state = self.state.lock().expect("DNS query snapshot lock");
            state.revision += 1;
            state.operation = DnsQueryOperation::Failed;
            state.failure = Some(Failure::new(
                ErrorCode::Canceled,
                "DNS query canceled before a response was observed",
                true,
            ));
        }
    }
}

#[cfg(test)]
#[path = "dns_query_application_test.rs"]
mod tests;
