//! Composed DNS probes execute the product command owner and correlate actual terminal feedback.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::Failure;

#[derive(Clone, Copy)]
pub(crate) enum DnsLeakProbeAdmission {
    Start,
    Retry,
}

impl AppState {
    pub(crate) fn begin_dns_leak_probe(
        &mut self,
        admission: DnsLeakProbeAdmission,
    ) -> Result<u64, Failure> {
        match admission {
            DnsLeakProbeAdmission::Start => self.diag.dns_leak_action.begin(),
            DnsLeakProbeAdmission::Retry => self.diag.dns_leak_action.begin_retry(),
        }
    }

    pub(crate) fn shared_dns_leak_command(
        &mut self,
        admission: DnsLeakProbeAdmission,
    ) -> Task<Message> {
        let Some(application) = self.command_ingress() else {
            return Task::none();
        };
        let token = match self.begin_dns_leak_probe(admission) {
            Ok(token) => token,
            Err(_) => return Task::none(),
        };
        Task::perform(
            async move {
                match (application.execute(CommandIntent::TestDnsLeak).await).into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::DnsLeakCommandFinished { token, result },
        )
    }
}
