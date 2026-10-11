//! Iced-local search draft plus typed outbound preference submission.

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::preferences::OutboundUiPreferenceIntent;

#[derive(Clone, Debug, Default)]
pub enum ProxySearchPhase {
    #[default]
    Uninitialized,
    Synchronized {
        query: String,
    },
    Dirty {
        query: String,
    },
    Pending {
        token: u64,
        query: String,
    },
    Failed {
        query: String,
        failure: Failure,
    },
}

#[derive(Default)]
pub struct ProxySearchState {
    pub phase: ProxySearchPhase,
}

impl ProxySearchState {
    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            ProxySearchPhase::Failed { failure, .. } => Some(failure),
            ProxySearchPhase::Uninitialized
            | ProxySearchPhase::Synchronized { .. }
            | ProxySearchPhase::Dirty { .. }
            | ProxySearchPhase::Pending { .. } => None,
        }
    }

    pub fn mark_pending(&mut self, token: u64, query: String) {
        self.phase = ProxySearchPhase::Pending { token, query };
    }

    pub fn preference_applied(&mut self, token: u64) {
        match self.phase.clone() {
            ProxySearchPhase::Pending {
                token: active_token,
                query,
            } if active_token == token => {
                self.phase = ProxySearchPhase::Synchronized { query };
            }
            ProxySearchPhase::Uninitialized
            | ProxySearchPhase::Synchronized { .. }
            | ProxySearchPhase::Dirty { .. }
            | ProxySearchPhase::Pending { .. }
            | ProxySearchPhase::Failed { .. } => {}
        }
    }

    pub fn preference_failed(&mut self, token: u64, failure: Failure) {
        match self.phase.clone() {
            ProxySearchPhase::Pending {
                token: active_token,
                query,
            } if active_token == token => {
                self.phase = ProxySearchPhase::Failed { query, failure };
            }
            ProxySearchPhase::Uninitialized
            | ProxySearchPhase::Synchronized { .. }
            | ProxySearchPhase::Dirty { .. }
            | ProxySearchPhase::Pending { .. }
            | ProxySearchPhase::Failed { .. } => {}
        }
    }

    pub fn preference_cancelled(&mut self, token: u64) {
        match self.phase.clone() {
            ProxySearchPhase::Pending {
                token: active_token,
                query,
            } if active_token == token => {
                self.phase = ProxySearchPhase::Dirty { query };
            }
            ProxySearchPhase::Uninitialized
            | ProxySearchPhase::Synchronized { .. }
            | ProxySearchPhase::Dirty { .. }
            | ProxySearchPhase::Pending { .. }
            | ProxySearchPhase::Failed { .. } => {}
        }
    }
}

impl AppState {
    pub(crate) fn update_proxy_search(&mut self, query: String) -> Task<Message> {
        self.runtime.proxy_filter = query.clone();
        self.runtime.proxy_search.phase = ProxySearchPhase::Dirty {
            query: query.clone(),
        };
        if self.is_product_command_ingress() {
            self.refresh_outbound_inventory_projection();
            if self.runtime.outbound_ui_preferences.data.is_none() {
                self.runtime.proxy_search.phase = ProxySearchPhase::Failed {
                    query,
                    failure: Failure::new(
                        ErrorCode::NotReady,
                        "outbound UI preferences are unavailable for this core scope",
                        true,
                    ),
                };
                return Task::none();
            }
            return self.submit_current_outbound_ui_preference(
                OutboundUiPreferenceIntent::SetSearchQuery(query),
            );
        }
        if self.shell.demo {
            self.recompute_filtered_groups();
            self.runtime.proxy_search.phase = ProxySearchPhase::Synchronized { query };
        } else {
            self.runtime.proxy_search.phase = ProxySearchPhase::Failed {
                query,
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "outbound search service is not composed",
                    true,
                ),
            };
        }
        Task::none()
    }
}
