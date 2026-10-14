//! Isolated echo host for behavior and pixel captures; never performs network requests.
use async_trait::async_trait;
use infiltrator_contract::dns_leak::{
    DnsLeakEchoReport, DnsLeakEchoRequest, DnsLeakObservation, DnsLeakObservationOutcome,
    DnsLeakProbeTransport,
};
use infiltrator_ports::dns_leak::DnsLeakEchoPort;
use infiltrator_ports::error::PortError;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Clone, Copy)]
pub enum EchoMode {
    Divergent,
    Consistent,
    Denied,
    AllSourcesFailed,
}
#[derive(Default)]
pub struct IsolatedEcho {
    mode: AtomicUsize,
    requests: Mutex<Vec<DnsLeakEchoRequest>>,
}
impl IsolatedEcho {
    pub fn set_mode(&self, mode: EchoMode) {
        self.mode.store(mode as usize, Ordering::SeqCst);
    }
    pub fn requests(&self) -> Vec<DnsLeakEchoRequest> {
        self.requests
            .lock()
            .expect("isolated probe requests")
            .clone()
    }
}
#[async_trait]
impl DnsLeakEchoPort for IsolatedEcho {
    async fn observe(&self, request: DnsLeakEchoRequest) -> Result<DnsLeakEchoReport, PortError> {
        self.requests
            .lock()
            .map_err(|_| PortError::Failed("isolated request log poisoned".into()))?
            .push(request.clone());
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == EchoMode::Denied as usize {
            return Err(PortError::PermissionDenied(
                "allow DNS echo access, then retry".into(),
            ));
        }
        let observations = request
            .probes
            .iter()
            .enumerate()
            .map(|(index, probe)| DnsLeakObservation {
                resolver: probe.resolver.clone(),
                authority: probe.authority.clone(),
                question: probe.question.clone(),
                transport: DnsLeakProbeTransport::System,
                outcome: if mode == EchoMode::AllSourcesFailed as usize {
                    DnsLeakObservationOutcome::TimedOut
                } else {
                    DnsLeakObservationOutcome::Observed {
                        identity: if mode == EchoMode::Consistent as usize || index == 0 {
                            "203.0.113.9"
                        } else {
                            "198.51.100.7"
                        }
                        .into(),
                    }
                },
            })
            .collect();
        Ok(DnsLeakEchoReport { observations })
    }
}
