//! TEA adapter for the shared group-order draft and correlated command result.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::proxy_group_order_editor::GroupMove;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;

impl AppState {
    pub(crate) fn open_group_order(
        &mut self,
        movement: Option<(String, GroupMove)>,
    ) -> Task<Message> {
        if !self.runtime.group_order_open {
            let groups = self
                .runtime
                .proxy_groups
                .iter()
                .map(|group| group.name.clone())
                .collect();
            if let Err(failure) = self.runtime.group_order_editor.open(groups) {
                self.runtime.group_order_editor.failure = Some(failure);
                return Task::none();
            }
            self.runtime.group_order_open = true;
        }
        self.observe_group_order();
        if let Some((name, direction)) = movement {
            self.runtime.group_order_editor.move_group(&name, direction);
        }
        Task::none()
    }
    pub(crate) fn observe_group_order(&mut self) {
        if !self.runtime.group_order_open {
            return;
        }
        let groups = self
            .surface
            .latest()
            .map(|snapshot| match &snapshot.pages.proxies.status {
                PageStatus::Ready | PageStatus::Empty => snapshot
                    .pages
                    .proxies
                    .data
                    .as_ref()
                    .map(|page| page.groups.iter().map(|group| group.name.clone()).collect())
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::Internal,
                            "proxy page has no group identities",
                            true,
                        )
                    }),
                PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                    Err(failure.clone())
                }
                PageStatus::Loading => Err(Failure::new(
                    ErrorCode::NotReady,
                    "group observations are refreshing",
                    true,
                )),
            })
            .unwrap_or_else(|| {
                Ok(self
                    .runtime
                    .proxy_groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect())
            });
        self.runtime.group_order_editor.observe(groups);
    }
    pub(crate) fn apply_group_order(&mut self) -> Task<Message> {
        let Some(application) = self.commands.clone() else {
            self.runtime.group_order_editor.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "group order command service is not composed",
                true,
            ));
            return Task::none();
        };
        let pending = match self.runtime.group_order_editor.begin() {
            Ok(pending) => pending,
            Err(failure) => {
                self.runtime.group_order_editor.failure = Some(failure);
                return Task::none();
            }
        };
        Task::perform(
            async move {
                match (application
                    .execute(CommandIntent::ReorderProxyGroups {
                        group_names: pending.groups,
                    })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::ProxyGroupOrderApplied {
                token: pending.token,
                result,
            },
        )
    }
}
