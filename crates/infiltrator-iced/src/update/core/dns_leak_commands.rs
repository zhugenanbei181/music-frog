//! Composed DNS probes execute the product command owner and correlate actual terminal feedback.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
impl AppState {
    pub(crate) fn shared_dns_leak_command(&mut self) -> Task<Message> {
        let Some(application) = self.commands.clone() else {
            return Task::none();
        };
        let token = match self.diag.dns_leak_action.begin() {
            Ok(token) => token,
            Err(_) => return Task::none(),
        };
        self.diag.is_probing_dns_leak = true;
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
