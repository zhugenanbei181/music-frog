//! TEA search drafts submit the neutral preference intent and correlate terminal failures.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Default)]
pub struct ProxySearchState {
    pub failure: Option<Failure>,
    pub pending: Option<(u64, String)>,
    pub draft_dirty: bool,
    next_token: u64,
}

impl AppState {
    pub(crate) fn update_proxy_search(&mut self, query: String) -> Task<Message> {
        self.runtime.proxy_filter = query.clone();
        self.runtime.proxy_search.draft_dirty = true;
        self.runtime.proxy_search.failure = None;
        if self.runtime.proxy_search.pending.is_some() {
            return Task::none();
        }
        let Some(application) = self.commands.clone() else {
            if self.shell.demo {
                self.recompute_filtered_groups();
            } else {
                self.runtime.proxy_search.failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "proxy search service is not composed",
                    true,
                ));
            }
            return Task::none();
        };
        self.runtime.proxy_search.next_token = self.runtime.proxy_search.next_token.wrapping_add(1);
        let token = self.runtime.proxy_search.next_token;
        self.runtime.proxy_search.pending = Some((token, query.clone()));
        let submitted = query.clone();
        Task::perform(
            async move {
                match (application
                    .execute(CommandIntent::SetProxySearchQuery { query: submitted })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::ProxySearchFinished {
                token,
                query: query.clone(),
                result,
            },
        )
    }

    pub(crate) fn finish_proxy_search(
        &mut self,
        token: u64,
        query: String,
        result: Result<(), Failure>,
    ) -> Task<Message> {
        if !self
            .runtime
            .proxy_search
            .pending
            .as_ref()
            .is_some_and(|pending| pending.0 == token && pending.1 == query)
        {
            return Task::none();
        }
        self.runtime.proxy_search.pending = None;
        if self.runtime.proxy_filter != query {
            return self.update_proxy_search(self.runtime.proxy_filter.clone());
        }
        self.runtime.proxy_search.failure = result.err();
        Task::none()
    }
}
