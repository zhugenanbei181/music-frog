use crate::state::AppState;
use crate::tray::tray_events_subscription;
use crate::types::{Message, Route, RuntimeStatus};
use iced::{Subscription, stream, window};
use std::time::Duration;

impl AppState {
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![];

        // 1. Tray Events (neutral channel; only subscribed when a tray is live)
        if let Some(rx) = &self.shell.tray_events {
            subs.push(tray_events_subscription(rx));
        }

        // 1b. Admin host commands (context -> app bridge for the Web UI)
        if let Some(rx) = &self.shell.admin_commands {
            subs.push(crate::admin_server::admin_commands_subscription(rx));
        }

        // 2. Scheduled subscription auto-update checks
        subs.push(Subscription::run(|| {
            stream::channel(
                100,
                |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let mut sub_interval = tokio::time::interval(Duration::from_secs(900));
                    loop {
                        sub_interval.tick().await;
                        let _ = output.try_send(Message::TickSubUpdate);
                    }
                },
            )
        }));

        // 3. Runtime-related background tasks
        if self.runtime.runtime.is_some() {
            subs.push(Subscription::run(|| {
                stream::channel(
                    100,
                    |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let mut sync_interval = tokio::time::interval(Duration::from_secs(3600));

                        loop {
                            sync_interval.tick().await;
                            let _ = output.try_send(Message::TickWebDavSync);
                        }
                    },
                )
            }));
        }
        if self.runtime.runtime.is_some()
            && self.shell.current_route == Route::Runtime
            && self.runtime.runtime_auto_refresh
            && matches!(self.runtime.status, RuntimeStatus::Running)
        {
            subs.push(Subscription::run(|| {
                stream::channel(
                    100,
                    |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let mut refresh_interval =
                            tokio::time::interval(Duration::from_millis(2000));
                        loop {
                            refresh_interval.tick().await;
                            let _ = output.try_send(Message::TickRuntimeRefresh);
                        }
                    },
                )
            }));
        }

        // 4. 高性能动画订阅：只有正在转场时才开启帧回调
        if self.shell.transition.start_time.is_some() {
            subs.push(window::frames().map(Message::TickFrame));
        }

        Subscription::batch(subs)
    }
}
