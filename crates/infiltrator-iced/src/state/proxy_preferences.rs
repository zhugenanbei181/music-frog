//! Serialize native preference intents; only shared observations determine the rendered state.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::collections::VecDeque;

#[derive(Default)]
pub struct ProxyPreferenceState {
    queue: VecDeque<CommandIntent>,
    pub pending: Option<u64>,
    pub failure: Option<Failure>,
    next_token: u64,
}
impl AppState {
    pub(crate) fn submit_proxy_preference(&mut self, intent: CommandIntent) -> Task<Message> {
        self.runtime.proxy_preferences.queue.push_back(intent);
        self.start_next_proxy_preference()
    }
    fn start_next_proxy_preference(&mut self) -> Task<Message> {
        if self.runtime.proxy_preferences.pending.is_some() {
            return Task::none();
        }
        let Some(intent) = self.runtime.proxy_preferences.queue.pop_front() else {
            return Task::none();
        };
        let Some(application) = self.commands.clone() else {
            self.runtime.proxy_preferences.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "proxy preference service is not composed",
                true,
            ));
            self.runtime.proxy_preferences.queue.clear();
            return Task::none();
        };
        self.runtime.proxy_preferences.next_token =
            self.runtime.proxy_preferences.next_token.wrapping_add(1);
        let token = self.runtime.proxy_preferences.next_token;
        self.runtime.proxy_preferences.pending = Some(token);
        self.runtime.proxy_preferences.failure = None;
        Task::perform(
            async move {
                match (application.execute(intent).await).into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::ProxyPreferenceFinished { token, result },
        )
    }
    pub(crate) fn finish_proxy_preference(
        &mut self,
        token: u64,
        result: Result<(), Failure>,
    ) -> Task<Message> {
        if self.runtime.proxy_preferences.pending != Some(token) {
            return Task::none();
        }
        self.runtime.proxy_preferences.pending = None;
        self.runtime.proxy_preferences.failure = result.err();
        self.start_next_proxy_preference()
    }
}
