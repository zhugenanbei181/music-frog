//! Explicit isolated response fixtures for actual command, reader and native UI paths.
use async_trait::async_trait;
use futures_util::future::{pending, poll_fn};
use futures_util::task::AtomicWaker;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::dns_query::{DnsQueryRequest, DnsQueryResponse, DnsQuestion, DnsRecord};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::dns_query::DnsQueryPort;
use infiltrator_ports::error::PortError;
use std::sync::Mutex;
use std::task::Poll;

#[derive(Clone, Copy, Debug, Default)]
pub enum QueryFixtureMode {
    #[default]
    Answer,
    ShortAnswer,
    Permission,
    Authentication,
    MismatchedQuestion,
    Unsupported,
    Pending,
    NegativeAnswer,
}
#[derive(Default)]
pub struct IsolatedQueries {
    requests: Mutex<Vec<DnsQueryRequest>>,
    mode: Mutex<QueryFixtureMode>,
    waker: AtomicWaker,
}
impl IsolatedQueries {
    pub fn requests(&self) -> Vec<DnsQueryRequest> {
        self.requests.lock().expect("isolated DNS requests").clone()
    }
    pub fn set_mode(&self, mode: QueryFixtureMode) {
        *self.mode.lock().expect("isolated DNS mode") = mode;
        self.waker.wake();
    }
}
pub fn response(request: &DnsQueryRequest) -> DnsQueryResponse {
    DnsQueryResponse {
        status: 0,
        questions: vec![DnsQuestion {
            name: format!("{}.", request.name.trim_end_matches('.')),
            record_type: request.record_type.number(),
            class: 1,
        }],
        truncated: false,
        recursion_desired: true,
        recursion_available: true,
        authenticated_data: false,
        checking_disabled: true,
        answers: (0..18)
            .map(|index| DnsRecord {
                name: format!("record{index}.test."),
                record_type: request.record_type.number(),
                ttl: 300 + index,
                data: format!("observed-{index} {{ttl}}"),
            })
            .collect(),
        authority: vec![DnsRecord {
            name: "test.".into(),
            record_type: 2,
            ttl: 600,
            data: "ns.test.".into(),
        }],
        additional: vec![DnsRecord {
            name: "ns.test.".into(),
            record_type: 1,
            ttl: 0,
            data: "192.0.2.53".into(),
        }],
    }
}
#[async_trait]
impl DnsQueryPort for IsolatedQueries {
    async fn query(&self, request: &DnsQueryRequest) -> Result<DnsQueryResponse, PortError> {
        self.requests
            .lock()
            .expect("isolated DNS requests")
            .push(request.clone());
        let mode = poll_fn(|context| {
            self.waker.register(context.waker());
            let mode = *self.mode.lock().expect("isolated DNS mode");
            if matches!(mode, QueryFixtureMode::Pending) {
                Poll::Pending
            } else {
                Poll::Ready(mode)
            }
        })
        .await;
        match mode {
            QueryFixtureMode::Answer => Ok(response(request)),
            QueryFixtureMode::ShortAnswer => {
                let mut response = response(request);
                response.answers.truncate(2);
                Ok(response)
            }
            QueryFixtureMode::NegativeAnswer => {
                let mut response = response(request);
                response.status = 3;
                response.answers.clear();
                Ok(response)
            }
            QueryFixtureMode::MismatchedQuestion => {
                let mut response = response(request);
                response.questions[0].name = "another.test.".into();
                Ok(response)
            }
            QueryFixtureMode::Permission => Err(PortError::PermissionDenied(
                "Allow controller DNS query access {reason}".into(),
            )),
            QueryFixtureMode::Authentication => Err(PortError::Rejected(Failure::new(
                ErrorCode::Authentication,
                "Configure controller authentication",
                false,
            ))),
            QueryFixtureMode::Unsupported => Err(PortError::unsupported(
                Capability::Dns,
                "No DNS query adapter",
            )),
            QueryFixtureMode::Pending => pending().await,
        }
    }
}
