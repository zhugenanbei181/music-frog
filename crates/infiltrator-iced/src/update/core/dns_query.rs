//! Execute one shared query from the native modal; acknowledgements are not results.
use crate::state::AppState;
use crate::types::dns_query::QueryAction;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::dns_query::DnsQueryOperationId;
use infiltrator_contract::error::{ErrorCode, Failure};
impl AppState {
    pub(super) fn update_dns_query(&mut self, action: QueryAction) -> Task<Message> {
        let model = &mut self.diag.dns_query;
        match action {
            QueryAction::Open => model.show(),
            QueryAction::Cancel => {
                model.cancel();
            }
            QueryAction::Name(name) => model.set_name(name),
            QueryAction::RecordType(kind) => model.set_type(kind),
            QueryAction::Section(section) => model.select(section),
            QueryAction::Previous => model.previous(),
            QueryAction::Next => model.next(),
            QueryAction::Finished { token, result } => {
                model.finish(token, result);
            }
            QueryAction::Run | QueryAction::Retry => {
                if matches!(action, QueryAction::Retry) && !model.can_retry() {
                    return Task::none();
                }
                let Ok((token, request)) = model.begin() else {
                    return Task::none();
                };
                let Some(application) = self.commands.clone() else {
                    model.finish(
                        token,
                        Err(Failure::new(
                            ErrorCode::NotReady,
                            "DNS query command service is unavailable",
                            true,
                        )),
                    );
                    return Task::none();
                };
                return Task::perform(
                    async move {
                        match (application
                            .execute(CommandIntent::QueryDns {
                                operation: DnsQueryOperationId(token),
                                request,
                            })
                            .await)
                            .into_unit()
                        {
                            Ok(()) => Ok(()),
                            Err(failure) => Err(failure),
                        }
                    },
                    move |result| Message::DnsQuery(QueryAction::Finished { token, result }),
                );
            }
        }
        Task::none()
    }
}
