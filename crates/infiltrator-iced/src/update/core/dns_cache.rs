//! TEA confirmation dispatches the shared application exactly once; failures stay in the modal.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::error::{ErrorCode, Failure};
impl AppState {
    pub(super) fn update_core_dns_cache(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DnsQuery(action) => self.update_dns_query(action),
            Message::FlushFakeIpCache => {
                self.diag.dns_cache_actions.show();
                Task::none()
            }
            Message::CancelDnsCacheFlush => {
                self.diag.dns_cache_actions.cancel();
                Task::none()
            }
            Message::ConfirmDnsCacheFlush | Message::RetryDnsCacheFlush => {
                let token = if matches!(message, Message::RetryDnsCacheFlush) {
                    self.diag.dns_cache_actions.retry()
                } else {
                    self.diag.dns_cache_actions.confirm()
                };
                let token = match token {
                    Ok(token) => token,
                    Err(_) => return Task::none(),
                };
                let Some(application) = self.commands.clone() else {
                    self.diag.dns_cache_actions.finish(
                        token,
                        Err(Failure::new(
                            ErrorCode::NotReady,
                            "DNS cache command service is unavailable",
                            true,
                        )),
                    );
                    return Task::none();
                };
                Task::perform(
                    async move {
                        match (application
                            .execute(CommandIntent::ClearDnsCache {
                                operation: DnsCacheOperationId(token),
                            })
                            .await)
                            .into_unit()
                        {
                            Ok(()) => Ok(()),
                            Err(failure) => Err(failure),
                        }
                    },
                    move |result| Message::DnsCacheCommandFinished { token, result },
                )
            }
            Message::DnsCacheCommandFinished { token, result } => {
                self.diag.dns_cache_actions.finish(token, result);
                Task::none()
            }
            other => self.update_core_dns_hosts(other),
        }
    }
}
