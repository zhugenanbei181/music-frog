//! DUAL-14-08: the shared DNS leak cross-source probe wiring.
//!
//! The Iced surface only triggers the shared application through the host
//! port and consumes the published report; it never authors an identity, a
//! country or an ISP. A host without a configured echo source answers a typed
//! unsupported failure and the panel renders exactly that.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::update::core::dns_leak_commands::DnsLeakProbeAdmission;
use iced::Task;
use infiltrator_application::dns_leak_application::NO_ECHO_PORT_REASON;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_presentation::dns_leak_actions::{DnsLeakActionPhase, DnsLeakFinish};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;

impl AppState {
    /// Publish the shared leak report into the render state. A probe in
    /// flight keeps the local optimistic report until the reader publishes
    /// the same fact.
    pub(crate) fn apply_dns_leak_snapshot(&mut self, report: &DnsLeakReport) {
        if !matches!(
            self.diag.dns_leak_action.phase,
            DnsLeakActionPhase::Pending { .. }
        ) || report.is_probed()
        {
            self.editor.dns_leak = report.clone();
        }
    }

    /// DNS leak probe messages. Unmatched messages fall through to the TUN
    /// config slice, which stays the next domain in the core chain.
    pub(super) fn update_core_dns_leak(&mut self, message: Message) -> Task<Message> {
        match message {
            // The shared application generates the random subdomains and
            // compares the observed identities, so a probe started here lands
            // in the one report the reader publishes to both surfaces.
            Message::DnsLeakCommandFinished { token, result } => {
                match self.diag.dns_leak_action.finish(token, result) {
                    DnsLeakFinish::Stale => {}
                    DnsLeakFinish::Applied => {
                        if self.shell.demo
                            && let Some(application) = &self.diag.dns_leak_capture
                        {
                            let mut snapshot = self
                                .surface
                                .latest()
                                .expect("isolated DNS snapshot")
                                .clone();
                            snapshot.revision += 1;
                            snapshot.dns_leak = application.last_report();
                            self.apply_shared_surface_snapshot(snapshot);
                        }
                    }
                }
                Task::none()
            }
            Message::RetryDnsLeakProbe => {
                if self.commands.is_some() {
                    return self.shared_dns_leak_command(DnsLeakProbeAdmission::Retry);
                }
                self.local_dns_leak_probe(DnsLeakProbeAdmission::Retry)
            }
            Message::RunDnsLeakProbe => {
                if self.commands.is_some() {
                    return self.shared_dns_leak_command(DnsLeakProbeAdmission::Start);
                }
                self.local_dns_leak_probe(DnsLeakProbeAdmission::Start)
            }
            Message::DnsLeakProbed { token, result } => {
                let terminal = result.as_ref().map(|_| ()).map_err(Clone::clone);
                match self.diag.dns_leak_action.finish(token, terminal) {
                    DnsLeakFinish::Stale => Task::none(),
                    DnsLeakFinish::Applied => match result {
                        Ok(report) => {
                            self.editor.dns_leak = report;
                            Task::none()
                        }
                        Err(failure) => {
                            self.set_error(&failure.message);
                            Task::done(Message::ShowToast(failure.message, ToastStatus::Error))
                        }
                    },
                }
            }
            other => self.update_core_stun(other),
        }
    }

    fn local_dns_leak_probe(&mut self, admission: DnsLeakProbeAdmission) -> Task<Message> {
        let token = match self.begin_dns_leak_probe(admission) {
            Ok(token) => token,
            Err(_) => return Task::none(),
        };
        let Some(runtime) = self.runtime.runtime.clone() else {
            return Task::done(Message::DnsLeakProbed {
                token,
                result: Err(Failure::new(
                    ErrorCode::NotReady,
                    "DNS probe runtime is unavailable",
                    true,
                )),
            });
        };
        let Some(port) = runtime.dns_leak_probe_port() else {
            return Task::done(Message::DnsLeakProbed {
                token,
                result: Err(Failure::unsupported(NO_ECHO_PORT_REASON)),
            });
        };
        Task::perform(
            async move { port.probe().await.map_err(Failure::from) },
            move |result| Message::DnsLeakProbed { token, result },
        )
    }
}
