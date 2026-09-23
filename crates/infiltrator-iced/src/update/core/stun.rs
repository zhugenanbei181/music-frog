//! DUAL-14-09 (re-scoped): the shared STUN UDP-egress probe wiring.
//!
//! The Iced surface only triggers the shared application through the host port
//! and consumes the published report. It never authors a public address: a host
//! without a STUN prober answers a typed unsupported failure and the panel
//! renders exactly that. The observation is this host/process's own UDP egress
//! mapping as seen by a STUN server, not a browser WebRTC result.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;

impl AppState {
    /// Publish the shared STUN report into the render state. A probe in flight
    /// keeps the local optimistic report until the reader publishes the same
    /// fact.
    pub(crate) fn apply_stun_snapshot(
        &mut self,
        dns: &infiltrator_contract::surface_snapshot::DnsPageSnapshot,
    ) {
        if !self.diag.is_probing_stun || dns.stun.is_reported() {
            self.editor.dns_stun = dns.stun.clone();
        }
    }

    /// STUN egress probe messages. Unmatched messages fall through to the TUN
    /// config slice, which stays the next domain in the core chain.
    pub(super) fn update_core_stun(&mut self, message: Message) -> Task<Message> {
        match message {
            // The shared application owns the server and the comparison, so a
            // probe started here lands in the one report the reader publishes.
            Message::RunStunProbe => {
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return Task::none();
                };
                let Some(port) = runtime.stun_egress_probe_port() else {
                    return Task::done(Message::StunProbed(Err(
                        infiltrator_contract::error::Failure::unsupported(
                            infiltrator_application::stun_probe_application::NO_STUN_PORT_REASON,
                        ),
                    )));
                };
                self.diag.is_probing_stun = true;
                Task::perform(
                    async move {
                        port.probe().await.map_err(|error| {
                            infiltrator_contract::error::Failure::new(
                                error.error_code(),
                                error.to_string(),
                                false,
                            )
                        })
                    },
                    Message::StunProbed,
                )
            }
            Message::StunProbed(result) => {
                self.diag.is_probing_stun = false;
                match result {
                    Ok(report) => {
                        self.editor.dns_stun = report;
                        Task::none()
                    }
                    Err(failure) => {
                        self.set_error(&failure.message);
                        Task::done(Message::ShowToast(failure.message, ToastStatus::Error))
                    }
                }
            }
            other => self.update_core_tun_config(other),
        }
    }
}
